use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Json, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Clone)]
struct AppState {
    api_token: Option<String>,
    default_model: String,
    kb: Option<Arc<KnowledgeBase>>,
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
    let kb_path = env::var("AMAN_KB_PATH").ok();
    let nostr_db_path = env::var("NOSTR_DB_PATH").ok();

    let kb = match nostr_db_path {
        Some(path) if !path.trim().is_empty() => match KnowledgeBase::from_nostr_db(PathBuf::from(path)) {
            Ok(kb) => {
                info!(entries = kb.entries.len(), "Loaded knowledge base from Nostr DB");
                Some(Arc::new(kb))
            }
            Err(err) => {
                warn!(error = %err, "Failed to load knowledge base from Nostr DB");
                None
            }
        },
        _ => match kb_path {
            Some(path) if !path.trim().is_empty() => match KnowledgeBase::load(PathBuf::from(path)) {
                Ok(kb) => {
                    info!(entries = kb.entries.len(), "Loaded knowledge base");
                    Some(Arc::new(kb))
                }
                Err(err) => {
                    warn!(error = %err, "Failed to load knowledge base");
                    None
                }
            },
            _ => None,
        },
    };

    let state = AppState {
        api_token,
        default_model,
        kb,
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
    let response_text = match (user_text, &state.kb) {
        (Some(text), Some(kb)) => match kb.search(&text) {
            Some(hit) => format!("KB hit ({})\n\n{}", hit.source, hit.snippet),
            None => format!("Echo: {}", text),
        },
        (Some(text), None) => format!("Echo: {}", text),
        (None, _) => "Echo: (no user message)".to_string(),
    };

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

struct KnowledgeBase {
    entries: Vec<KbEntry>,
}

struct KbEntry {
    source: String,
    text: String,
    text_lower: String,
}

struct KbMatch {
    source: String,
    snippet: String,
}

impl KnowledgeBase {
    fn load(path: PathBuf) -> Result<Self, std::io::Error> {
        if path.is_file() {
            let entry = load_file(&path)?;
            return Ok(Self {
                entries: entry.into_iter().collect(),
            });
        }

        if !path.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Knowledge base path not found: {}", path.display()),
            ));
        }

        let mut entries = Vec::new();
        for entry in WalkDir::new(&path).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_file() || !is_supported_path(path) {
                continue;
            }
            if let Ok(Some(kb_entry)) = load_file(path) {
                entries.push(kb_entry);
            }
        }

        Ok(Self { entries })
    }

    fn from_nostr_db(path: PathBuf) -> Result<Self, std::io::Error> {
        let conn = Connection::open(&path).map_err(to_io_error)?;
        let mut stmt = conn
            .prepare("SELECT doc_id, chunk_id, blob_ref FROM chunks")
            .map_err(to_io_error)?;

        let mut entries = Vec::new();
        let rows = stmt
            .query_map([], |row| {
                let doc_id: String = row.get(0)?;
                let chunk_id: String = row.get(1)?;
                let blob_ref: Option<String> = row.get(2)?;
                Ok((doc_id, chunk_id, blob_ref))
            })
            .map_err(to_io_error)?;

        for row in rows {
            let (doc_id, chunk_id, blob_ref) = row.map_err(to_io_error)?;
            let Some(blob_ref) = blob_ref else { continue };
            let Some(path) = blob_ref_to_path(&blob_ref) else { continue };
            let source = format!("{}:{}", doc_id, chunk_id);
            if let Ok(Some(entry)) = load_chunk_from_path(&path, source) {
                entries.push(entry);
            }
        }

        Ok(Self { entries })
    }

    fn search(&self, query: &str) -> Option<KbMatch> {
        let tokens = tokenize(query);
        if tokens.is_empty() {
            return None;
        }

        let mut best: Option<(&KbEntry, usize)> = None;
        for entry in &self.entries {
            let score = tokens
                .iter()
                .map(|token| entry.text_lower.matches(token).count())
                .sum::<usize>();

            if score > 0 {
                match best {
                    Some((_, best_score)) if best_score >= score => {}
                    _ => best = Some((entry, score)),
                }
            }
        }

        let (entry, _) = best?;
        let snippet = build_snippet(entry, &tokens);
        Some(KbMatch {
            source: entry.source.clone(),
            snippet,
        })
    }
}

fn load_file(path: &Path) -> Result<Option<KbEntry>, std::io::Error> {
    const MAX_BYTES: u64 = 512 * 1024;
    const MAX_CHARS: usize = 8000;

    let metadata = fs::metadata(path)?;
    if metadata.len() == 0 || metadata.len() > MAX_BYTES {
        return Ok(None);
    }

    let text = fs::read_to_string(path)?;
    let trimmed: String = text.chars().take(MAX_CHARS).collect();
    if trimmed.trim().is_empty() {
        return Ok(None);
    }

    let source = path.display().to_string();
    let text_lower = trimmed.to_ascii_lowercase();
    Ok(Some(KbEntry {
        source,
        text: trimmed,
        text_lower,
    }))
}

fn is_supported_path(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => matches!(ext.to_ascii_lowercase().as_str(), "txt" | "md" | "markdown" | "jsonl"),
        None => false,
    }
}

fn tokenize(query: &str) -> Vec<String> {
    query
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| token.len() > 2)
        .take(8)
        .map(|token| token.to_string())
        .collect()
}

fn build_snippet(entry: &KbEntry, tokens: &[String]) -> String {
    let lower = &entry.text_lower;
    let mut first_hit = None;
    for token in tokens {
        if let Some(idx) = lower.find(token) {
            first_hit = Some(idx);
            break;
        }
    }

    match first_hit {
        Some(idx) => {
            let prefix = entry.text.get(..idx).unwrap_or(&entry.text);
            let start_chars = prefix.chars().count().saturating_sub(160);
            let total_chars = entry.text.chars().count();
            let end_chars = (start_chars + 400).min(total_chars);
            entry
                .text
                .chars()
                .skip(start_chars)
                .take(end_chars.saturating_sub(start_chars))
                .collect::<String>()
                .trim()
                .to_string()
        }
        None => entry.text.chars().take(400).collect(),
    }
}

fn load_chunk_from_path(path: &Path, source: String) -> Result<Option<KbEntry>, std::io::Error> {
    const MAX_BYTES: u64 = 512 * 1024;
    const MAX_CHARS: usize = 8000;

    let metadata = fs::metadata(path)?;
    if metadata.len() == 0 || metadata.len() > MAX_BYTES {
        return Ok(None);
    }

    let text = fs::read_to_string(path)?;
    let trimmed: String = text.chars().take(MAX_CHARS).collect();
    if trimmed.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(KbEntry {
        source,
        text_lower: trimmed.to_ascii_lowercase(),
        text: trimmed,
    }))
}

fn blob_ref_to_path(blob_ref: &str) -> Option<PathBuf> {
    if let Some(path) = blob_ref.strip_prefix("file://") {
        return Some(PathBuf::from(path));
    }

    let candidate = PathBuf::from(blob_ref);
    if candidate.is_absolute() || candidate.exists() {
        return Some(candidate);
    }

    None
}

fn to_io_error(err: rusqlite::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, err.to_string())
}
