use js_sys::{Date, Math};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wasm_bindgen::JsValue;
use worker::{
    console_error, console_log, event, Context, Env, Fetch, Headers, Method, Request, RequestInit,
    Response,
};

const MAX_BODY_BYTES: usize = 64 * 1024;
const RECENT_MAX_MESSAGES: usize = 6;
const RECENT_MESSAGE_MAX_CHARS: usize = 280;
const SUMMARY_MAX_CHARS: usize = 600;

#[event(fetch)]
async fn fetch(mut req: Request, env: Env, _ctx: Context) -> worker::Result<Response> {
    if req.method() == Method::Options {
        return cors_preflight();
    }

    let path = req.path();
    let method = req.method();

    let response = match (method, path.as_str()) {
        (Method::Get, "/health") => Ok(json_response(
            200,
            &HealthResponse {
                status: "ok",
                version: env!("CARGO_PKG_VERSION"),
            },
        )?),
        (Method::Get, "/v1/models") => handle_models(&env).await,
        (Method::Post, "/v1/chat/completions") => handle_chat_completions(&mut req, &env).await,
        _ => Err(ApiError::not_found("route not found")),
    };

    match response {
        Ok(resp) => Ok(add_cors(resp)?),
        Err(err) => Ok(add_cors(error_response(err))?),
    }
}

#[derive(Debug)]
struct ApiError {
    status: u16,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: 400,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: 401,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: 404,
            message: message.into(),
        }
    }

    fn too_many_requests(message: impl Into<String>) -> Self {
        Self {
            status: 429,
            message: message.into(),
        }
    }

    fn bad_gateway(message: impl Into<String>) -> Self {
        Self {
            status: 502,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: 500,
            message: message.into(),
        }
    }
}

type ApiResult<T> = std::result::Result<T, ApiError>;

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorDetails,
}

#[derive(Serialize)]
struct ErrorDetails {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct ModelList {
    object: &'static str,
    data: Vec<ModelInfo>,
}

#[derive(Serialize)]
struct ModelInfo {
    id: String,
    object: &'static str,
    owned_by: &'static str,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ChatMessage {
    #[serde(default)]
    role: String,
    #[serde(default)]
    content: Value,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionRequest {
    model: Option<String>,
    #[serde(default)]
    messages: Vec<ChatMessage>,
    stream: Option<bool>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    top_p: Option<f32>,
    user: Option<String>,
}

#[derive(Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MemorySnapshot {
    summary: Option<String>,
    updated_at: u64,
    message_count: u64,
    #[serde(default)]
    last_messages: Vec<MemoryMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryMessage {
    role: String,
    content: String,
}

#[derive(Clone)]
struct Settings {
    openrouter_api_key: String,
    openrouter_api_url: String,
    openrouter_http_referer: Option<String>,
    openrouter_x_title: Option<String>,
    default_model: String,
    summary_model: String,
    memory_max_chars: usize,
    memory_summarize_every_turns: u64,
    allow_anon: bool,
    worker_api_token: Option<String>,
    rate_limit_max: u64,
    rate_limit_window_secs: u64,
    nostr_relays: Vec<String>,
    nostr_secret_key: Option<String>,
}

impl Settings {
    fn from_env(env: &Env) -> ApiResult<Self> {
        let openrouter_api_key = env_string(env, "OPENROUTER_API_KEY")
            .ok_or_else(|| ApiError::internal("OPENROUTER_API_KEY is not set"))?;
        let openrouter_api_url = env_string(env, "OPENROUTER_API_URL")
            .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
        let default_model = env_string(env, "DEFAULT_MODEL")
            .unwrap_or_else(|| "openai/gpt-4o-mini".to_string());
        let summary_model = env_string(env, "SUMMARY_MODEL")
            .unwrap_or_else(|| "mistral-small".to_string());
        let memory_max_chars = env_usize(env, "MEMORY_MAX_CHARS", 1200);
        let memory_summarize_every_turns = env_u64(env, "MEMORY_SUMMARIZE_EVERY_TURNS", 6);
        let allow_anon = env_bool(env, "ALLOW_ANON", true);
        let worker_api_token = env_string(env, "WORKER_API_TOKEN");
        let rate_limit_max = env_u64(env, "RATE_LIMIT_MAX", 60);
        let rate_limit_window_secs = env_u64(env, "RATE_LIMIT_WINDOW_SECS", 60);
        let nostr_relays = env_string(env, "NOSTR_RELAYS")
            .map(|value| {
                value
                    .split(',')
                    .map(|item| item.trim().to_string())
                    .filter(|item| !item.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let nostr_secret_key = env_string(env, "NOSTR_SECRET_KEY");

        Ok(Self {
            openrouter_api_key,
            openrouter_api_url,
            openrouter_http_referer: env_string(env, "OPENROUTER_HTTP_REFERER"),
            openrouter_x_title: env_string(env, "OPENROUTER_X_TITLE"),
            default_model,
            summary_model,
            memory_max_chars,
            memory_summarize_every_turns,
            allow_anon,
            worker_api_token,
            rate_limit_max,
            rate_limit_window_secs,
            nostr_relays,
            nostr_secret_key,
        })
    }
}

async fn handle_models(env: &Env) -> ApiResult<Response> {
    let settings = Settings::from_env(env)?;
    let response = ModelList {
        object: "list",
        data: vec![ModelInfo {
            id: settings.default_model,
            object: "model",
            owned_by: "openrouter",
        }],
    };

    json_response(200, &response).map_err(|err| ApiError::internal(err.to_string()))
}

async fn handle_chat_completions(req: &mut Request, env: &Env) -> ApiResult<Response> {
    let settings = Settings::from_env(env)?;
    let auth_header = header_value(req.headers(), "Authorization");
    let user_header = header_value(req.headers(), "X-Aman-User");

    if !settings.allow_anon {
        authorize(auth_header.as_deref(), &settings)?;
    }

    let body = req
        .bytes()
        .await
        .map_err(|err| ApiError::bad_request(format!("Failed to read body: {err}")))?;
    if body.len() > MAX_BODY_BYTES {
        return Err(ApiError::bad_request("Request body too large"));
    }

    let request: ChatCompletionRequest = serde_json::from_slice(&body)
        .map_err(|err| ApiError::bad_request(format!("Invalid JSON: {err}")))?;

    if request.stream.unwrap_or(false) {
        return Err(ApiError::bad_request("stream=true is not supported"));
    }

    if request.messages.is_empty() {
        return Err(ApiError::bad_request("messages array is required"));
    }

    let user_id = user_header
        .or_else(|| request.user.clone())
        .unwrap_or_else(|| "anon".to_string());
    let history_key = format!("user:{}", sanitize_identity(&user_id));

    let kv = env
        .kv("AMAN_MEMORY")
        .map_err(|_| ApiError::internal("KV binding AMAN_MEMORY is missing"))?;

    enforce_rate_limit(
        &kv,
        &history_key,
        settings.rate_limit_max,
        settings.rate_limit_window_secs,
    )
    .await?;

    let snapshot_key = format!("memory:{}", history_key);
    let mut snapshot = kv
        .get(&snapshot_key)
        .json::<MemorySnapshot>()
        .await
        .map_err(|err| ApiError::internal(format!("KV read failed: {err}")))?
        .unwrap_or_default();

    let memory_prompt = build_memory_prompt(&snapshot, settings.memory_max_chars);
    let messages = inject_memory(request.messages.clone(), memory_prompt);

    let model = request
        .model
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| settings.default_model.clone());

    let payload = OpenRouterRequest {
        model,
        messages,
        temperature: request.temperature,
        max_tokens: request.max_tokens,
        top_p: request.top_p,
        user: Some(history_key.clone()),
    };

    let response_json = call_openrouter(&settings, &payload).await?;

    let user_text = last_user_text(&request.messages);
    let assistant_text = extract_assistant_text(&response_json);

    let now = now_unix();
    update_snapshot(
        &mut snapshot,
        user_text.as_deref(),
        assistant_text.as_deref(),
        now,
    );

    if should_summarize(&snapshot, settings.memory_summarize_every_turns) {
        if let Some(summary) = summarize_memory(&settings, &snapshot).await? {
            snapshot.summary = Some(summary);
            if let Err(err) = publish_summary_event(&settings, &history_key, &snapshot).await {
                console_error!("Nostr publish failed: {}", err.message);
            }
        }
    }

    kv.put(&snapshot_key, serde_json::to_string(&snapshot).map_err(|err| {
        ApiError::internal(format!("Failed to serialize memory snapshot: {err}"))
    })?)
    .map_err(|err| ApiError::internal(format!("KV write failed: {err}")))?
    .execute()
    .await
    .map_err(|err| ApiError::internal(format!("KV write failed: {err}")))?;

    let resp = json_response(200, &response_json)
        .map_err(|err| ApiError::internal(format!("Response build failed: {err}")))?;

    Ok(resp)
}

fn authorize(auth_header: Option<&str>, settings: &Settings) -> ApiResult<()> {
    let expected = settings.worker_api_token.as_deref().ok_or_else(|| {
        ApiError::internal("WORKER_API_TOKEN is not configured and ALLOW_ANON=false")
    })?;
    let Some(auth) = auth_header else {
        return Err(ApiError::unauthorized("Missing Authorization header"));
    };
    let token = auth.strip_prefix("Bearer ").unwrap_or(auth);
    if token != expected {
        return Err(ApiError::unauthorized("Invalid token"));
    }
    Ok(())
}

async fn call_openrouter(
    settings: &Settings,
    payload: &OpenRouterRequest,
) -> ApiResult<Value> {
    let body = serde_json::to_string(payload)
        .map_err(|err| ApiError::internal(format!("Failed to encode payload: {err}")))?;

    let headers = Headers::new();
    headers
        .set("Authorization", &format!("Bearer {}", settings.openrouter_api_key))
        .map_err(|err| ApiError::internal(format!("Header error: {err}")))?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|err| ApiError::internal(format!("Header error: {err}")))?;
    if let Some(referrer) = settings.openrouter_http_referer.as_deref() {
        headers
            .set("HTTP-Referer", referrer)
            .map_err(|err| ApiError::internal(format!("Header error: {err}")))?;
    }
    if let Some(title) = settings.openrouter_x_title.as_deref() {
        headers
            .set("X-Title", title)
            .map_err(|err| ApiError::internal(format!("Header error: {err}")))?;
    }

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(JsValue::from_str(&body)));

    let req = Request::new_with_init(
        &format!("{}/chat/completions", settings.openrouter_api_url.trim_end_matches('/')),
        &init,
    )
    .map_err(|err| ApiError::internal(format!("Failed to build OpenRouter request: {err}")))?;

    let mut resp = Fetch::Request(req)
        .send()
        .await
        .map_err(|err| ApiError::bad_gateway(format!("OpenRouter request failed: {err}")))?;

    let status = resp.status_code();
    let text = resp
        .text()
        .await
        .map_err(|err| ApiError::bad_gateway(format!("OpenRouter response failed: {err}")))?;

    if status >= 400 {
        return Err(ApiError::bad_gateway(format!(
            "OpenRouter error ({status}): {}",
            truncate_text(&text, 500)
        )));
    }

    serde_json::from_str(&text)
        .map_err(|err| ApiError::bad_gateway(format!("Invalid OpenRouter JSON: {err}")))
}

async fn summarize_memory(
    settings: &Settings,
    snapshot: &MemorySnapshot,
) -> ApiResult<Option<String>> {
    if snapshot.last_messages.is_empty() {
        return Ok(snapshot.summary.clone());
    }

    let mut lines = Vec::new();
    if let Some(summary) = snapshot.summary.as_ref().filter(|s| !s.trim().is_empty()) {
        lines.push(format!("Existing summary: {}", summary.trim()));
    } else {
        lines.push("Existing summary: (none)".to_string());
    }
    lines.push("Recent messages:".to_string());
    for msg in &snapshot.last_messages {
        let content = normalize_line(&msg.content);
        lines.push(format!("- {}: {}", msg.role, content));
    }

    let prompt = lines.join("\n");

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: Value::String(
                "Summarize the conversation memory in 1-3 short bullet points. Keep it under 600 characters. Avoid sensitive details or PII."
                    .to_string(),
            ),
        },
        ChatMessage {
            role: "user".to_string(),
            content: Value::String(prompt),
        },
    ];

    let payload = OpenRouterRequest {
        model: settings.summary_model.clone(),
        messages,
        temperature: Some(0.2),
        max_tokens: Some(200),
        top_p: Some(0.9),
        user: None,
    };

    let response = call_openrouter(settings, &payload).await?;
    let summary = extract_assistant_text(&response)
        .unwrap_or_default();
    let summary = truncate_text(summary.trim(), SUMMARY_MAX_CHARS);

    if summary.is_empty() {
        Ok(snapshot.summary.clone())
    } else {
        Ok(Some(summary))
    }
}

async fn publish_summary_event(
    settings: &Settings,
    history_key: &str,
    snapshot: &MemorySnapshot,
) -> ApiResult<()> {
    if settings.nostr_relays.is_empty() || settings.nostr_secret_key.is_none() {
        return Ok(());
    }

    console_log!(
        "Nostr publish requested for {history_key} (relays: {}). Not yet implemented in worker.",
        settings.nostr_relays.join(",")
    );

    let _ = snapshot;
    Ok(())
}

async fn enforce_rate_limit(
    kv: &worker::KvStore,
    history_key: &str,
    max: u64,
    window_secs: u64,
) -> ApiResult<()> {
    if max == 0 || window_secs == 0 {
        return Ok(());
    }

    let now = now_unix();
    let window = now / window_secs;
    let key = format!("rate:{history_key}:{window}");

    let current = kv
        .get(&key)
        .text()
        .await
        .map_err(|err| ApiError::internal(format!("KV read failed: {err}")))?
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);

    if current >= max {
        return Err(ApiError::too_many_requests("Rate limit exceeded"));
    }

    let next = current + 1;
    kv.put(&key, next.to_string())
        .map_err(|err| ApiError::internal(format!("KV write failed: {err}")))?
        .expiration_ttl(window_secs + 5)
        .execute()
        .await
        .map_err(|err| ApiError::internal(format!("KV write failed: {err}")))?;

    Ok(())
}

fn build_memory_prompt(snapshot: &MemorySnapshot, max_chars: usize) -> Option<String> {
    if max_chars == 0 {
        return None;
    }

    let mut lines = Vec::new();

    if let Some(summary) = snapshot.summary.as_ref().filter(|s| !s.trim().is_empty()) {
        lines.push(format!("- Summary: {}", normalize_line(summary)));
    }

    if !snapshot.last_messages.is_empty() {
        lines.push("- Recent:".to_string());
        for msg in &snapshot.last_messages {
            let content = normalize_line(&msg.content);
            lines.push(format!("  - {}: {}", msg.role, content));
        }
    }

    if lines.is_empty() {
        return None;
    }

    let mut prompt = String::new();
    prompt.push_str("[MEMORY]\n");
    prompt.push_str(&lines.join("\n"));
    prompt.push_str("\n[/MEMORY]");

    let trimmed = truncate_text(prompt.trim(), max_chars);
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn inject_memory(mut messages: Vec<ChatMessage>, memory_prompt: Option<String>) -> Vec<ChatMessage> {
    let Some(memory) = memory_prompt else {
        return messages;
    };

    let memory_message = ChatMessage {
        role: "system".to_string(),
        content: Value::String(memory),
    };

    let insert_at = messages
        .iter()
        .position(|msg| msg.role != "system")
        .unwrap_or(messages.len());
    messages.insert(insert_at, memory_message);
    messages
}

fn update_snapshot(
    snapshot: &mut MemorySnapshot,
    user_text: Option<&str>,
    assistant_text: Option<&str>,
    now: u64,
) {
    if let Some(text) = user_text {
        push_recent(snapshot, "user", text);
        snapshot.message_count = snapshot.message_count.saturating_add(1);
    }

    if let Some(text) = assistant_text {
        push_recent(snapshot, "assistant", text);
    }

    snapshot.updated_at = now;
}

fn push_recent(snapshot: &mut MemorySnapshot, role: &str, content: &str) {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return;
    }
    let entry = MemoryMessage {
        role: role.to_string(),
        content: truncate_text(trimmed, RECENT_MESSAGE_MAX_CHARS),
    };
    snapshot.last_messages.push(entry);
    while snapshot.last_messages.len() > RECENT_MAX_MESSAGES {
        snapshot.last_messages.remove(0);
    }
}

fn should_summarize(snapshot: &MemorySnapshot, every_turns: u64) -> bool {
    every_turns > 0 && snapshot.message_count > 0 && snapshot.message_count % every_turns == 0
}

fn last_user_text(messages: &[ChatMessage]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|msg| msg.role == "user")
        .and_then(|msg| extract_text(&msg.content))
}

fn extract_assistant_text(response: &Value) -> Option<String> {
    response
        .pointer("/choices/0/message/content")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn extract_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => {
            let mut out = String::new();
            for part in parts {
                if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(text);
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(out)
            }
        }
        _ => None,
    }
}

fn normalize_line(input: &str) -> String {
    input
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_text(input: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut count = 0usize;
    let mut out = String::new();
    for ch in input.chars() {
        if count >= max_chars {
            break;
        }
        out.push(ch);
        count += 1;
    }
    out
}

fn sanitize_identity(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        if out.len() >= 64 {
            break;
        }
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch);
        } else if ch.is_ascii_whitespace() {
            out.push('_');
        }
    }
    if out.is_empty() {
        "anon".to_string()
    } else {
        out
    }
}

fn env_string(env: &Env, key: &str) -> Option<String> {
    env.var(key)
        .ok()
        .map(|value| value.to_string())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_bool(env: &Env, key: &str, default: bool) -> bool {
    env_string(env, key)
        .map(|value| matches!(value.to_lowercase().as_str(), "1" | "true" | "yes" | "y"))
        .unwrap_or(default)
}

fn env_usize(env: &Env, key: &str, default: usize) -> usize {
    env_string(env, key)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
}

fn env_u64(env: &Env, key: &str, default: u64) -> u64 {
    env_string(env, key)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn header_value(headers: &Headers, name: &str) -> Option<String> {
    headers
        .get(name)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn now_unix() -> u64 {
    (Date::now() / 1000.0) as u64
}

fn json_response<T: Serialize>(status: u16, value: &T) -> worker::Result<Response> {
    let mut resp = Response::from_json(value)?;
    resp = resp.with_status(status);
    Ok(resp)
}

fn error_response(err: ApiError) -> Response {
    let envelope = ErrorEnvelope {
        error: ErrorDetails {
            message: err.message,
            error_type: "invalid_request_error".to_string(),
        },
    };
    let mut resp = Response::from_json(&envelope).unwrap_or_else(|_| {
        Response::error("Internal error", 500).unwrap_or_else(|_| Response::empty().unwrap())
    });
    resp = resp.with_status(err.status);
    resp
}

fn add_cors(mut resp: Response) -> worker::Result<Response> {
    let headers = resp.headers_mut();
    headers.set("Access-Control-Allow-Origin", "*")?;
    headers.set(
        "Access-Control-Allow-Headers",
        "Authorization, Content-Type, X-Aman-User",
    )?;
    headers.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")?;
    Ok(resp)
}

fn cors_preflight() -> worker::Result<Response> {
    let mut resp = Response::empty()?;
    resp = resp.with_status(204);
    add_cors(resp)
}

fn _random_id(prefix: &str) -> String {
    let ts = now_unix();
    let rand = (Math::random() * 1_000_000.0) as u64;
    format!("{prefix}-{ts}-{rand}")
}
