use std::env;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Json, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    api_token: Option<String>,
    default_model: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionRequest {
    #[serde(default)]
    model: String,
    #[serde(default)]
    messages: Vec<ChatMessage>,
    #[serde(default)]
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    role: String,
    #[serde(default)]
    content: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ChatCompletionResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<ChatChoice>,
    usage: Usage,
}

#[derive(Debug, Serialize)]
struct ChatChoice {
    index: u32,
    message: ChatMessageResponse,
    finish_reason: String,
}

#[derive(Debug, Serialize)]
struct ChatMessageResponse {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Serialize)]
struct ChatCompletionChunk {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<ChatChoiceChunk>,
}

#[derive(Debug, Serialize)]
struct ChatChoiceChunk {
    index: u32,
    delta: serde_json::Value,
    finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct ModelList {
    object: String,
    data: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
struct ModelInfo {
    id: String,
    object: String,
    owned_by: String,
}

#[derive(Debug, Serialize)]
struct Health {
    status: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let addr = env::var("AMAN_API_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let api_token = env::var("AMAN_API_TOKEN").ok();
    let default_model = env::var("AMAN_API_MODEL").unwrap_or_else(|_| "aman-chat".to_string());

    let state = AppState {
        api_token,
        default_model,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .with_state(state);

    let addr: SocketAddr = addr.parse().expect("Invalid AMAN_API_ADDR");
    info!(%addr, "Aman API listening");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok".to_string(),
    })
}

async fn list_models(State(state): State<AppState>) -> Json<ModelList> {
    Json(ModelList {
        object: "list".to_string(),
        data: vec![
            ModelInfo {
                id: state.default_model.clone(),
                object: "model".to_string(),
                owned_by: "aman".to_string(),
            },
            ModelInfo {
                id: "aman-rag".to_string(),
                object: "model".to_string(),
                owned_by: "aman".to_string(),
            },
        ],
    })
}

async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ChatCompletionRequest>,
) -> Result<Response, ApiError> {
    authorize(&state, &headers)?;

    let model = if payload.model.is_empty() {
        state.default_model.clone()
    } else {
        payload.model.clone()
    };

    let user_text = last_user_text(&payload.messages);
    let response_text = format!("Echo: {}", user_text.unwrap_or_else(|| "(no user message)".to_string()));

    if payload.stream {
        let stream = stream_chat_completion(model, response_text);
        return Ok(Sse::new(stream).into_response());
    }

    let response = ChatCompletionResponse {
        id: format!("chatcmpl-{}", Uuid::new_v4()),
        object: "chat.completion".to_string(),
        created: unix_timestamp(),
        model,
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessageResponse {
                role: "assistant".to_string(),
                content: response_text,
            },
            finish_reason: "stop".to_string(),
        }],
        usage: Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
    };

    Ok(Json(response).into_response())
}

fn authorize(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(expected) = state.api_token.as_deref() else {
        return Ok(());
    };

    let Some(value) = headers.get(axum::http::header::AUTHORIZATION) else {
        return Err(ApiError::Unauthorized);
    };

    let Ok(value) = value.to_str() else {
        return Err(ApiError::Unauthorized);
    };

    let token = value.strip_prefix("Bearer ").unwrap_or(value);
    if token != expected {
        return Err(ApiError::Unauthorized);
    }

    Ok(())
}

fn last_user_text(messages: &[ChatMessage]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|msg| msg.role == "user")
        .and_then(|msg| extract_text(&msg.content))
}

fn extract_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    parts.push(text);
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(""))
            }
        }
        _ => None,
    }
}

fn stream_chat_completion(
    model: String,
    content: String,
) -> impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>> {
    let id = format!("chatcmpl-{}", Uuid::new_v4());
    let created = unix_timestamp();

    let first = ChatCompletionChunk {
        id: id.clone(),
        object: "chat.completion.chunk".to_string(),
        created,
        model: model.clone(),
        choices: vec![ChatChoiceChunk {
            index: 0,
            delta: serde_json::json!({
                "role": "assistant",
                "content": content,
            }),
            finish_reason: None,
        }],
    };

    let done = ChatCompletionChunk {
        id,
        object: "chat.completion.chunk".to_string(),
        created,
        model,
        choices: vec![ChatChoiceChunk {
            index: 0,
            delta: serde_json::json!({}),
            finish_reason: Some("stop".to_string()),
        }],
    };

    let events = vec![
        Event::default().data(serde_json::to_string(&first).unwrap()),
        Event::default().data(serde_json::to_string(&done).unwrap()),
        Event::default().data("[DONE]"),
    ];

    tokio_stream::iter(events.into_iter().map(Ok))
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug)]
enum ApiError {
    Unauthorized,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::Unauthorized => {
                warn!("Unauthorized request");
                let body = serde_json::json!({
                    "error": {
                        "message": "Unauthorized",
                        "type": "auth_error"
                    }
                });
                (StatusCode::UNAUTHORIZED, Json(body)).into_response()
            }
        }
    }
}
