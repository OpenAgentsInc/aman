use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{Json, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use reqwest::Client;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;
use walkdir::WalkDir;

use orchestrator::{InboundMessage, NoOpSender, Orchestrator};

#[derive(Clone)]
struct AppState {
    api_token: Option<String>,
    default_model: String,
    kb: Option<Arc<KnowledgeBase>>,
    mode: ApiMode,
    orchestrator: Option<Arc<Orchestrator<NoOpSender>>>,
    openrouter: Option<OpenRouterConfig>,
    http_client: Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApiMode {
    Echo,
    Orchestrator,
    OpenRouter,
}

impl ApiMode {
    fn from_env(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "orchestrator" | "brain" | "aman" => Self::Orchestrator,
            "openrouter" | "open-router" | "router" => Self::OpenRouter,
            _ => Self::Echo,
        }
    }
}

#[derive(Clone, Debug)]
struct OpenRouterConfig {
    api_key: String,
    api_url: String,
    model: Option<String>,
    http_referer: Option<String>,
    title: Option<String>,
}

impl OpenRouterConfig {
    fn from_env() -> Result<Self, String> {
        let api_key = env::var("OPENROUTER_API_KEY")
            .map_err(|_| "OPENROUTER_API_KEY not set".to_string())?;
        let api_url = env::var("OPENROUTER_API_URL")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());
        let model = env::var("OPENROUTER_MODEL").ok().filter(|value| !value.trim().is_empty());
        let http_referer = env::var("OPENROUTER_HTTP_REFERER").ok().filter(|value| !value.trim().is_empty());
        let title = env::var("OPENROUTER_X_TITLE").ok().filter(|value| !value.trim().is_empty());

        Ok(Self {
            api_key,
            api_url,
            model,
            http_referer,
            title,
        })
    }
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
    let _ = dotenvy::dotenv();

    let addr = env::var("AMAN_API_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let api_token = env::var("AMAN_API_TOKEN").ok();
    let default_model = env::var("AMAN_API_MODEL").unwrap_or_else(|_| "aman-chat".to_string());
    let kb_path = env::var("AMAN_KB_PATH").ok();
    let nostr_db_path = env::var("NOSTR_DB_PATH").ok();
    let mode = ApiMode::from_env(&env::var("AMAN_API_MODE").unwrap_or_else(|_| "echo".to_string()));

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

    let orchestrator = if mode == ApiMode::Orchestrator {
        info!("Initializing orchestrator-backed API");
        let orchestrator = Orchestrator::from_env(NoOpSender)
            .await
            .unwrap_or_else(|err| {
                panic!("Failed to initialize orchestrator: {}", err);
            });
        Some(Arc::new(orchestrator))
    } else {
        None
    };

    let openrouter = if mode == ApiMode::OpenRouter {
        match OpenRouterConfig::from_env() {
            Ok(config) => {
                info!("Initializing OpenRouter-backed API");
                Some(config)
            }
            Err(err) => panic!("Failed to initialize OpenRouter: {}", err),
        }
    } else {
        None
    };

    let http_client = Client::builder()
        .build()
        .expect("Failed to initialize HTTP client");

    let state = AppState {
        api_token,
        default_model,
        kb,
        mode,
        orchestrator,
        openrouter,
        http_client,
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
    Json(payload): Json<serde_json::Value>,
) -> Result<Response, ApiError> {
    authorize(&state, &headers)?;

    let parsed: ChatCompletionRequest = serde_json::from_value(payload.clone()).map_err(|err| {
        ApiError::BadRequest(format!("Invalid request body: {}", err))
    })?;

    let model = if parsed.model.is_empty() {
        state.default_model.clone()
    } else {
        parsed.model.clone()
    };

    let user_text = last_user_text(&parsed.messages);
    if state.mode == ApiMode::OpenRouter {
        return openrouter_infer(&state, &headers, payload, user_text.as_deref()).await;
    }
    let response_text = match state.mode {
        ApiMode::Orchestrator => {
            let text = user_text.ok_or_else(|| ApiError::BadRequest("Missing user message".to_string()))?;
            let sender = header_string(&headers, "x-aman-user").unwrap_or_else(|| "api-user".to_string());
            let group_id = header_string(&headers, "x-aman-group");
            let inbound = build_inbound_message(sender, group_id, text);
            let orchestrator = state
                .orchestrator
                .clone()
                .ok_or_else(|| ApiError::Upstream("Orchestrator not configured".to_string()))?;
            let response = orchestrator
                .process(inbound)
                .await
                .map_err(|err| ApiError::Upstream(format!("Orchestrator error: {}", err)))?;
            response.text
        }
        ApiMode::Echo => match (user_text, &state.kb) {
            (Some(text), Some(kb)) => match kb.search(&text) {
                Some(hit) => format!("KB hit ({})\n\n{}", hit.source, hit.snippet),
                None => format!("Echo: {}", text),
            },
            (Some(text), None) => format!("Echo: {}", text),
            (None, _) => "Echo: (no user message)".to_string(),
        },
        ApiMode::OpenRouter => unreachable!("handled earlier"),
    };

    if parsed.stream {
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

fn header_string(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn build_inbound_message(sender: String, group_id: Option<String>, text: String) -> InboundMessage {
    let timestamp = unix_timestamp_millis();
    if let Some(group_id) = group_id {
        InboundMessage::group(sender, text, timestamp, group_id)
    } else {
        InboundMessage::direct(sender, text, timestamp)
    }
}

async fn openrouter_infer(
    state: &AppState,
    headers: &HeaderMap,
    payload: serde_json::Value,
    user_text: Option<&str>,
) -> Result<Response, ApiError> {
    let Some(config) = state.openrouter.as_ref() else {
        return Err(ApiError::Upstream("OpenRouter not configured".to_string()));
    };

    let mut body = match payload {
        serde_json::Value::Object(map) => map,
        _ => return Err(ApiError::BadRequest("Request body must be a JSON object".to_string())),
    };

    if let (Some(kb), Some(text)) = (state.kb.as_ref(), user_text) {
        if let Some(hit) = kb.search(text) {
            if let Some(serde_json::Value::Array(messages)) = body.get_mut("messages") {
                let context = format!(
                    "Context from local knowledge base (use only if relevant; cite the source in plain text if used):\nSource: {}\n\n{}",
                    hit.source, hit.snippet
                );
                let context_message = serde_json::json!({
                    "role": "system",
                    "content": context,
                });
                let insert_at = find_system_tail(messages);
                messages.insert(insert_at, context_message);
            } else {
                warn!("OpenRouter request missing messages array; skipping KB injection");
            }
        }
    }

    if !body.contains_key("model") {
        if let Some(model) = &config.model {
            body.insert("model".to_string(), serde_json::Value::String(model.clone()));
        }
    }

    if !body.contains_key("user") {
        if let Some(user) = header_string(headers, "x-aman-user") {
            body.insert("user".to_string(), serde_json::Value::String(user));
        }
    }

    let url = format!("{}/chat/completions", config.api_url.trim_end_matches('/'));
    let mut request = state
        .http_client
        .post(url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json");

    if let Some(referer) = &config.http_referer {
        request = request.header("HTTP-Referer", referer);
    }

    if let Some(title) = &config.title {
        request = request.header("X-Title", title);
    }

    let response = request
        .json(&serde_json::Value::Object(body))
        .send()
        .await
        .map_err(|err| ApiError::Upstream(format!("OpenRouter request failed: {}", err)))?;

    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let bytes = response
        .bytes()
        .await
        .map_err(|err| ApiError::Upstream(format!("OpenRouter response failed: {}", err)))?;

    let mut outgoing = Response::new(Body::from(bytes));
    *outgoing.status_mut() = status;
    if let Ok(value) = HeaderValue::from_str(&content_type) {
        outgoing.headers_mut().insert(CONTENT_TYPE, value);
    }

    Ok(outgoing)
}

fn find_system_tail(messages: &[serde_json::Value]) -> usize {
    let mut index = 0;
    while index < messages.len() {
        let role = messages[index]
            .get("role")
            .and_then(|value| value.as_str());
        if role == Some("system") {
            index += 1;
        } else {
            break;
        }
    }
    index
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

fn unix_timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Debug)]
enum ApiError {
    Unauthorized,
    BadRequest(String),
    Upstream(String),
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
            ApiError::BadRequest(message) => {
                let body = serde_json::json!({
                    "error": {
                        "message": message,
                        "type": "invalid_request_error"
                    }
                });
                (StatusCode::BAD_REQUEST, Json(body)).into_response()
            }
            ApiError::Upstream(message) => {
                let body = serde_json::json!({
                    "error": {
                        "message": message,
                        "type": "server_error"
                    }
                });
                (StatusCode::BAD_GATEWAY, Json(body)).into_response()
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
