use base64::Engine;
use js_sys::{Date, Math};
use futures_util::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wasm_bindgen::JsValue;
use xsalsa20poly1305::aead::{Aead, KeyInit};
use xsalsa20poly1305::{Key, Nonce, XSalsa20Poly1305};
use worker::{
    console_error, console_log, event, ByteStream, Context, D1Database, Env, Fetch, Headers,
    Method, Request, RequestInit, Response, ScheduleContext, ScheduledEvent,
};

mod nostr;

const MAX_BODY_BYTES: usize = 64 * 1024;
const RECENT_MAX_MESSAGES: usize = 6;
const RECENT_MESSAGE_MAX_CHARS: usize = 280;
const SUMMARY_MAX_CHARS: usize = 600;
const KB_QUERY_MAX_CHARS: usize = 500;
const KB_CONTEXT_PREFIX: &str = "[KNOWLEDGE BASE CONTEXT]";
const KB_CONTEXT_SUFFIX: &str = "[END KNOWLEDGE BASE CONTEXT]";
const SYNC_STATE_KEY: &str = "kb_checkpoint";
const SECRETBOX_TAG: &str = "secretbox-v1";
const NOSTR_RELAY_TIMEOUT_MS: u64 = 4500;
const KB_FALLBACK_CANDIDATES: usize = 200;
const DEFAULT_SYSTEM_PROMPT: &str = "You are Aman, a privacy-focused AI assistant built for high-risk contexts. Respond clearly and succinctly, prioritize user safety and privacy, and ask clarifying questions when needed. When [KNOWLEDGE BASE CONTEXT] is present, answer using only that context and cite document titles in brackets (e.g., [source: title]). If the context does not answer the question, say so.";

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
        (Method::Get, "/kb/status") => handle_kb_status(&env, req.headers()).await,
        (Method::Post, "/kb/search") => handle_kb_search(&mut req, &env).await,
        (Method::Post, "/kb/sync") => handle_kb_sync(&req, &env).await,
        _ => Err(ApiError::not_found("route not found")),
    };

    match response {
        Ok(resp) => Ok(add_cors(resp)?),
        Err(err) => Ok(add_cors(error_response(err))?),
    }
}

#[event(scheduled)]
async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    if let Err(err) = sync_kb(&env).await {
        console_error!("KB sync failed: {}", err.message);
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

#[derive(Serialize)]
struct KbStatusResponse {
    docs: u64,
    chunks: u64,
    last_checkpoint: Option<u64>,
    last_sync_at: Option<u64>,
    fts_enabled: bool,
}

#[derive(Debug, Deserialize)]
struct KbSearchRequest {
    query: String,
    limit: Option<u32>,
}

#[derive(Serialize)]
struct KbSearchResponse {
    hits: Vec<KbHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KbHit {
    chunk_id: String,
    doc_id: String,
    text: String,
    title: Option<String>,
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
    stream: Option<bool>,
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
struct SyncState {
    since: u64,
    updated_at: u64,
}

#[derive(Debug, Deserialize)]
struct DocManifestPayload {
    doc_id: String,
    title: String,
    lang: String,
    mime: String,
    updated_at: u64,
    content_hash: String,
    blob_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkRefPayload {
    chunk_id: String,
    doc_id: String,
    ord: u32,
    chunk_hash: String,
    #[serde(default)]
    blob_ref: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    created_at: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CountRow {
    count: i64,
}

#[derive(Debug, Deserialize)]
struct SyncStateRow {
    value: String,
}

#[derive(Debug, Deserialize)]
struct DbChunkRow {
    chunk_id: String,
    doc_id: String,
    text: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TitleRow {
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NameRow {
    name: String,
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
    system_prompt: String,
    memory_max_chars: usize,
    memory_summarize_every_turns: u64,
    allow_anon: bool,
    worker_api_token: Option<String>,
    rate_limit_max: u64,
    rate_limit_window_secs: u64,
    nostr_relays: Vec<String>,
    nostr_secret_key: Option<String>,
    nostr_kb_author: Option<String>,
    nostr_secretbox_key: Option<SecretBoxKey>,
    kb_sync_lookback_secs: u64,
    kb_max_snippet_chars: usize,
    kb_max_total_chars: usize,
    kb_max_hits: usize,
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
            .unwrap_or_else(|| "openai/gpt-5-nano".to_string());
        let system_prompt = env_string(env, "SYSTEM_PROMPT")
            .unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string());
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
        let nostr_kb_author = env_string(env, "NOSTR_KB_AUTHOR");
        let nostr_secretbox_key =
            env_string(env, "NOSTR_SECRETBOX_KEY").and_then(|value| match SecretBoxKey::from_str(&value) {
                Ok(key) => Some(key),
                Err(err) => {
                    console_error!("Invalid NOSTR_SECRETBOX_KEY: {err}");
                    None
                }
            });
        let kb_sync_lookback_secs = env_u64(env, "KB_SYNC_LOOKBACK_SECS", 86400);
        let kb_max_snippet_chars = env_usize(env, "KB_MAX_SNIPPET_CHARS", 600);
        let kb_max_total_chars = env_usize(env, "KB_MAX_TOTAL_CHARS", 1200);
        let kb_max_hits = env_usize(env, "KB_MAX_HITS", 3);

        Ok(Self {
            openrouter_api_key,
            openrouter_api_url,
            openrouter_http_referer: env_string(env, "OPENROUTER_HTTP_REFERER"),
            openrouter_x_title: env_string(env, "OPENROUTER_X_TITLE"),
            default_model,
            summary_model,
            system_prompt,
            memory_max_chars,
            memory_summarize_every_turns,
            allow_anon,
            worker_api_token,
            rate_limit_max,
            rate_limit_window_secs,
            nostr_relays,
            nostr_secret_key,
            nostr_kb_author,
            nostr_secretbox_key,
            kb_sync_lookback_secs,
            kb_max_snippet_chars,
            kb_max_total_chars,
            kb_max_hits,
        })
    }
}

#[derive(Clone)]
struct SecretBoxKey([u8; 32]);

impl SecretBoxKey {
    fn from_str(value: &str) -> Result<Self, String> {
        let bytes = decode_secretbox_key(value)?;
        Ok(Self(bytes))
    }
}

fn decode_secretbox_key(value: &str) -> Result<[u8; 32], String> {
    let trimmed = value.trim();
    let bytes = if let Some(hex_value) = trimmed.strip_prefix("hex:") {
        hex::decode(hex_value).map_err(|err| err.to_string())?
    } else if is_probably_hex(trimmed) {
        hex::decode(trimmed).map_err(|err| err.to_string())?
    } else {
        base64::engine::general_purpose::STANDARD
            .decode(trimmed)
            .map_err(|err| err.to_string())?
    };

    if bytes.len() != 32 {
        return Err(format!("expected 32 bytes, got {}", bytes.len()));
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

fn is_probably_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
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

    let mut kb_debug = header_value(req.headers(), "X-KB-Debug")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !kb_debug {
        if let Ok(url) = req.url() {
            for (key, value) in url.query_pairs() {
                if key.eq_ignore_ascii_case("kb_debug")
                    && (value == "1" || value.eq_ignore_ascii_case("true"))
                {
                    kb_debug = true;
                    break;
                }
            }
        }
    }

    let model = request
        .model
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| settings.default_model.clone());

    let messages = inject_system_prompt(request.messages.clone(), &settings.system_prompt);

    let user_text = last_user_text(&request.messages);
    let user_text_for_debug = user_text.clone();
    let kb_prompt = if let Some(query) = user_text.as_deref() {
        match env.d1("AMAN_KB") {
            Ok(db) => match build_kb_prompt(&db, query, &settings).await {
                Ok(prompt) => prompt,
                Err(err) => {
                    console_error!("KB retrieval failed: {}", err.message);
                    None
                }
            },
            Err(_) => None,
        }
    } else {
        None
    };
    let kb_prompt_for_debug = kb_prompt.clone();
    let memory_prompt = if kb_prompt.is_some() {
        None
    } else {
        build_memory_prompt(&snapshot, settings.memory_max_chars)
    };
    let messages = inject_memory(messages, memory_prompt);
    let messages = inject_knowledge(messages, kb_prompt);

    if request.stream.unwrap_or(false) {
        let payload = OpenRouterRequest {
            model,
            messages,
            stream: Some(true),
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            top_p: request.top_p,
            user: Some(history_key.clone()),
        };

        return stream_chat_completion(
            &settings,
            payload,
            kv,
            snapshot_key,
            snapshot,
            history_key,
            user_text,
        )
        .await;
    }

    let payload = OpenRouterRequest {
        model,
        messages,
        stream: None,
        temperature: request.temperature,
        max_tokens: request.max_tokens,
        top_p: request.top_p,
        user: Some(history_key.clone()),
    };

    let mut response_json = call_openrouter(&settings, &payload).await?;
    if kb_debug {
        if let Value::Object(obj) = &mut response_json {
            let context = kb_prompt_for_debug.unwrap_or_default();
            let tokens = user_text_for_debug
                .as_deref()
                .map(tokenize_query)
                .unwrap_or_default();
            let token_values = tokens.into_iter().map(Value::String).collect::<Vec<_>>();
            let mut debug = serde_json::Map::new();
            debug.insert(
                "query".to_string(),
                user_text_for_debug
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            debug.insert("tokens".to_string(), Value::Array(token_values));
            debug.insert(
                "context".to_string(),
                if context.is_empty() {
                    Value::Null
                } else {
                    Value::String(context)
                },
            );
            obj.insert("kb_debug".to_string(), Value::Object(debug));
        }
    }

    let assistant_text = extract_assistant_text(&response_json);
    update_snapshot(
        &mut snapshot,
        user_text.as_deref(),
        assistant_text.as_deref(),
        now_unix(),
    );

    finalize_snapshot(&settings, &history_key, &mut snapshot).await?;
    save_snapshot(&kv, &snapshot_key, &snapshot).await?;

    let resp = json_response(200, &response_json)
        .map_err(|err| ApiError::internal(format!("Response build failed: {err}")))?;

    Ok(resp)
}

async fn handle_kb_status(env: &Env, headers: &Headers) -> ApiResult<Response> {
    let settings = Settings::from_env(env)?;
    let auth_header = header_value(headers, "Authorization");
    if !settings.allow_anon {
        authorize(auth_header.as_deref(), &settings)?;
    }

    let db = env
        .d1("AMAN_KB")
        .map_err(|_| ApiError::internal("D1 binding AMAN_KB is missing"))?;

    let docs = count_table(&db, "docs").await?;
    let chunks = count_table(&db, "chunks").await?;
    let sync_state = load_sync_state(&db).await?;
    let fts_enabled = fts_available(&db).await.unwrap_or(false);

    let response = KbStatusResponse {
        docs,
        chunks,
        last_checkpoint: sync_state.as_ref().map(|state| state.since),
        last_sync_at: sync_state.as_ref().map(|state| state.updated_at),
        fts_enabled,
    };

    json_response(200, &response).map_err(|err| ApiError::internal(err.to_string()))
}

async fn handle_kb_search(req: &mut Request, env: &Env) -> ApiResult<Response> {
    let settings = Settings::from_env(env)?;
    let auth_header = header_value(req.headers(), "Authorization");
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

    let request: KbSearchRequest = serde_json::from_slice(&body)
        .map_err(|err| ApiError::bad_request(format!("Invalid JSON: {err}")))?;

    let limit = request
        .limit
        .map(|value| value as usize)
        .unwrap_or(settings.kb_max_hits)
        .min(settings.kb_max_hits)
        .max(1);

    let db = env
        .d1("AMAN_KB")
        .map_err(|_| ApiError::internal("D1 binding AMAN_KB is missing"))?;

    let hits = search_kb(&db, &request.query, &settings, Some(limit)).await?;
    let response = KbSearchResponse { hits };

    json_response(200, &response).map_err(|err| ApiError::internal(err.to_string()))
}

async fn handle_kb_sync(req: &Request, env: &Env) -> ApiResult<Response> {
    let settings = Settings::from_env(env)?;
    let auth_header = header_value(req.headers(), "Authorization");
    if !settings.allow_anon {
        authorize(auth_header.as_deref(), &settings)?;
    }

    let mut override_since = None;
    if let Ok(url) = req.url() {
        for (key, value) in url.query_pairs() {
            if key.eq_ignore_ascii_case("full") || key.eq_ignore_ascii_case("reset") {
                if value == "1" || value.eq_ignore_ascii_case("true") {
                    override_since =
                        Some(now_unix().saturating_sub(settings.kb_sync_lookback_secs));
                }
            }
        }
    }

    sync_kb_with_since(env, override_since).await?;
    handle_kb_status(env, req.headers()).await
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

async fn call_openrouter_stream(
    settings: &Settings,
    payload: &OpenRouterRequest,
) -> ApiResult<Response> {
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
    headers
        .set("Accept", "text/event-stream")
        .map_err(|err| ApiError::internal(format!("Header error: {err}")))?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(JsValue::from_str(&body)));

    let req = Request::new_with_init(
        &format!("{}/chat/completions", settings.openrouter_api_url.trim_end_matches('/')),
        &init,
    )
    .map_err(|err| ApiError::internal(format!("Failed to build OpenRouter request: {err}")))?;

    Fetch::Request(req)
        .send()
        .await
        .map_err(|err| ApiError::bad_gateway(format!("OpenRouter request failed: {err}")))
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
        stream: None,
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

async fn build_kb_prompt(
    db: &D1Database,
    query: &str,
    settings: &Settings,
) -> ApiResult<Option<String>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if looks_sensitive_query(trimmed) {
        return Ok(None);
    }

    let capped = truncate_text(trimmed, KB_QUERY_MAX_CHARS);
    let hits = search_kb(db, &capped, settings, None).await?;
    if hits.is_empty() {
        return Ok(None);
    }

    Ok(format_kb_context(
        &hits,
        settings.kb_max_snippet_chars,
        settings.kb_max_total_chars,
    ))
}

fn format_kb_context(
    hits: &[KbHit],
    max_snippet_chars: usize,
    max_total_chars: usize,
) -> Option<String> {
    if hits.is_empty() || max_total_chars == 0 {
        return None;
    }

    let mut lines = Vec::new();
    let instruction = "Answer using only these sources. Cite with [source: <title>]. If they do not answer the question, say so.";
    let mut used = KB_CONTEXT_PREFIX.len() + KB_CONTEXT_SUFFIX.len() + 2 + instruction.len() + 1;

    for hit in hits {
        let snippet = truncate_text(&normalize_line(&hit.text), max_snippet_chars);
        if snippet.is_empty() {
            continue;
        }
        let mut label = format!("doc_id={}, chunk_id={}", hit.doc_id, hit.chunk_id);
        if let Some(title) = hit.title.as_ref() {
            let title = truncate_text(&normalize_line(title), 120);
            if !title.is_empty() {
                label = title;
            }
        }
        let line = format!("- [{}] {}", label, snippet);
        if used + line.len() + 1 > max_total_chars {
            break;
        }
        used += line.len() + 1;
        lines.push(line);
    }

    if lines.is_empty() {
        return None;
    }

    let mut out = String::new();
    out.push_str(KB_CONTEXT_PREFIX);
    out.push('\n');
    out.push_str(instruction);
    out.push('\n');
    out.push_str(&lines.join("\n"));
    out.push('\n');
    out.push_str(KB_CONTEXT_SUFFIX);

    let trimmed = truncate_text(out.trim(), max_total_chars);
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

async fn stream_chat_completion(
    settings: &Settings,
    payload: OpenRouterRequest,
    kv: worker::KvStore,
    snapshot_key: String,
    snapshot: MemorySnapshot,
    history_key: String,
    user_text: Option<String>,
) -> ApiResult<Response> {
    let mut upstream = call_openrouter_stream(settings, &payload).await?;
    let status = upstream.status_code();
    if status >= 400 {
        let text = upstream
            .text()
            .await
            .map_err(|err| ApiError::bad_gateway(format!("OpenRouter response failed: {err}")))?;
        return Err(ApiError::bad_gateway(format!(
            "OpenRouter error ({status}): {}",
            truncate_text(&text, 500)
        )));
    }

    let upstream_stream = upstream
        .stream()
        .map_err(|err| ApiError::bad_gateway(format!("OpenRouter stream failed: {err}")))?;

    let state = StreamState {
        upstream: upstream_stream,
        buffer: String::new(),
        assistant_text: String::new(),
        snapshot,
        snapshot_key,
        history_key,
        user_text,
        settings: settings.clone(),
        kv,
    };

    let stream = stream::unfold(state, |mut state| async move {
        let next = state.upstream.next().await;
        match next {
            Some(Ok(chunk)) => {
                absorb_sse_chunk(&mut state, &chunk);
                Some((Ok(chunk), state))
            }
            Some(Err(err)) => Some((Err(err), state)),
            None => {
                finalize_stream_state(&mut state).await;
                None
            }
        }
    });

    let mut resp = Response::from_stream(stream)
        .map_err(|err| ApiError::bad_gateway(format!("Streaming response failed: {err}")))?;
    let headers = resp.headers_mut();
    headers
        .set("Content-Type", "text/event-stream")
        .map_err(|err| ApiError::internal(format!("Header error: {err}")))?;
    headers
        .set("Cache-Control", "no-cache")
        .map_err(|err| ApiError::internal(format!("Header error: {err}")))?;
    Ok(resp)
}

struct StreamState {
    upstream: ByteStream,
    buffer: String,
    assistant_text: String,
    snapshot: MemorySnapshot,
    snapshot_key: String,
    history_key: String,
    user_text: Option<String>,
    settings: Settings,
    kv: worker::KvStore,
}

fn absorb_sse_chunk(state: &mut StreamState, chunk: &[u8]) {
    let text = String::from_utf8_lossy(chunk);
    state.buffer.push_str(&text);

    while let Some(idx) = state.buffer.find('\n') {
        let line = state.buffer[..idx].trim_end_matches('\r').to_string();
        state.buffer = state.buffer[idx + 1..].to_string();
        process_sse_line(state, &line);
    }
}

fn process_sse_line(state: &mut StreamState, line: &str) {
    let line = line.trim();
    if !line.starts_with("data:") {
        return;
    }
    let data = line.trim_start_matches("data:").trim();
    if data.is_empty() || data == "[DONE]" {
        return;
    }
    let Ok(value) = serde_json::from_str::<Value>(data) else {
        return;
    };
    if let Some(content) = value
        .pointer("/choices/0/delta/content")
        .and_then(|val| val.as_str())
    {
        state.assistant_text.push_str(content);
    } else if let Some(content) = value
        .pointer("/choices/0/message/content")
        .and_then(|val| val.as_str())
    {
        state.assistant_text.push_str(content);
    }
}

async fn finalize_stream_state(state: &mut StreamState) {
    update_snapshot(
        &mut state.snapshot,
        state.user_text.as_deref(),
        Some(state.assistant_text.as_str()),
        now_unix(),
    );

    if let Err(err) =
        finalize_snapshot(&state.settings, &state.history_key, &mut state.snapshot).await
    {
        console_error!("Stream finalize failed: {}", err.message);
    }

    if let Err(err) = save_snapshot(&state.kv, &state.snapshot_key, &state.snapshot).await {
        console_error!("KV write failed: {}", err.message);
    }
}

fn inject_system_prompt(mut messages: Vec<ChatMessage>, prompt: &str) -> Vec<ChatMessage> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return messages;
    }

    if let Some(first) = messages.first() {
        if first.role == "system" {
            if let Value::String(content) = &first.content {
                if content.trim() == trimmed {
                    return messages;
                }
            }
        }
    }

    messages.insert(
        0,
        ChatMessage {
            role: "system".to_string(),
            content: Value::String(trimmed.to_string()),
        },
    );
    messages
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

fn inject_knowledge(mut messages: Vec<ChatMessage>, knowledge_prompt: Option<String>) -> Vec<ChatMessage> {
    let Some(knowledge) = knowledge_prompt else {
        return messages;
    };

    if let Some(index) = messages.iter().position(|msg| msg.role == "system") {
        if let Value::String(content) = &messages[index].content {
            let mut combined = content.trim_end().to_string();
            combined.push('\n');
            combined.push('\n');
            combined.push_str(&knowledge);
            messages[index].content = Value::String(combined);
            return messages;
        }
    }

    messages.insert(
        0,
        ChatMessage {
            role: "system".to_string(),
            content: Value::String(knowledge),
        },
    );
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

async fn finalize_snapshot(
    settings: &Settings,
    history_key: &str,
    snapshot: &mut MemorySnapshot,
) -> ApiResult<()> {
    if should_summarize(snapshot, settings.memory_summarize_every_turns) {
        if let Some(summary) = summarize_memory(settings, snapshot).await? {
            snapshot.summary = Some(summary);
            if let Err(err) = publish_summary_event(settings, history_key, snapshot).await {
                console_error!("Nostr publish failed: {}", err.message);
            }
        }
    }
    Ok(())
}

async fn save_snapshot(
    kv: &worker::KvStore,
    snapshot_key: &str,
    snapshot: &MemorySnapshot,
) -> ApiResult<()> {
    kv.put(
        snapshot_key,
        serde_json::to_string(snapshot)
            .map_err(|err| ApiError::internal(format!("Failed to serialize memory snapshot: {err}")))?,
    )
    .map_err(|err| ApiError::internal(format!("KV write failed: {err}")))?
    .execute()
    .await
    .map_err(|err| ApiError::internal(format!("KV write failed: {err}")))?;
    Ok(())
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

async fn search_kb(
    db: &D1Database,
    query: &str,
    settings: &Settings,
    limit_override: Option<usize>,
) -> ApiResult<Vec<KbHit>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if looks_sensitive_query(trimmed) {
        return Ok(Vec::new());
    }

    let capped = truncate_text(trimmed, KB_QUERY_MAX_CHARS);
    let tokens = tokenize_query(&capped);
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    if settings.kb_max_hits == 0 {
        return Ok(Vec::new());
    }

    let mut limit = limit_override.unwrap_or(settings.kb_max_hits).max(1);
    limit = limit.min(settings.kb_max_hits).max(1);

    let mut hits = Vec::new();
    if fts_available(db).await.unwrap_or(false) {
        match search_kb_fts(db, &tokens, limit).await {
            Ok(found) => hits = found,
            Err(err) => console_error!("KB FTS search failed: {}", err.message),
        }
    }

    if hits.is_empty() {
        hits = search_kb_fallback(db, &tokens, limit).await?;
    }

    for hit in hits.iter_mut() {
        hit.text = truncate_text(hit.text.trim(), settings.kb_max_snippet_chars);
    }
    hits.retain(|hit| !hit.text.is_empty());

    Ok(hits)
}

async fn search_kb_fts(
    db: &D1Database,
    tokens: &[String],
    limit: usize,
) -> ApiResult<Vec<KbHit>> {
    let Some(query) = build_fts_query(tokens) else {
        return Ok(Vec::new());
    };

    let stmt = db.prepare(
        "SELECT chunk_id, doc_id, text, title \
         FROM chunks_fts \
         WHERE chunks_fts MATCH ?1 \
         ORDER BY bm25(chunks_fts) \
         LIMIT ?2",
    );
    let result = stmt
        .bind(&[
            JsValue::from_str(&query),
            JsValue::from_f64(limit as f64),
        ])
        .map_err(|err| ApiError::internal(format!("D1 bind failed: {err}")))?
        .all()
        .await
        .map_err(|err| ApiError::internal(format!("D1 query failed: {err}")))?;

    let rows: Vec<DbChunkRow> = result
        .results()
        .map_err(|err| ApiError::internal(format!("D1 parse failed: {err}")))?;

    let hits = rows
        .into_iter()
        .filter_map(|row| {
            row.text.map(|text| KbHit {
                chunk_id: row.chunk_id,
                doc_id: row.doc_id,
                text,
                title: row.title,
            })
        })
        .collect();

    Ok(hits)
}

async fn search_kb_fallback(
    db: &D1Database,
    tokens: &[String],
    limit: usize,
) -> ApiResult<Vec<KbHit>> {
    if tokens.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let stmt = db.prepare(
        "SELECT chunks.chunk_id as chunk_id, chunks.doc_id as doc_id, chunks.text as text, \
         docs.title as title \
         FROM chunks \
         LEFT JOIN docs ON docs.doc_id = chunks.doc_id \
         WHERE chunks.text IS NOT NULL \
         ORDER BY chunks.created_at DESC \
         LIMIT ?1",
    );
    let result = stmt
        .bind(&[JsValue::from_f64(KB_FALLBACK_CANDIDATES as f64)])
        .map_err(|err| ApiError::internal(format!("D1 bind failed: {err}")))?
        .all()
        .await
        .map_err(|err| ApiError::internal(format!("D1 query failed: {err}")))?;

    let rows: Vec<DbChunkRow> = result
        .results()
        .map_err(|err| ApiError::internal(format!("D1 parse failed: {err}")))?;

    let mut scored = Vec::new();
    for row in rows {
        let text = row.text.unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        let haystack = text.to_lowercase();
        let mut score = 0usize;
        for token in tokens {
            if haystack.contains(token) {
                score += 1;
            }
        }
        if score > 0 {
            scored.push((
                score,
                KbHit {
                    chunk_id: row.chunk_id,
                    doc_id: row.doc_id,
                    text,
                    title: row.title,
                },
            ));
        }
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.truncate(limit);
    Ok(scored.into_iter().map(|(_, hit)| hit).collect())
}

fn build_fts_query(tokens: &[String]) -> Option<String> {
    if tokens.is_empty() {
        return None;
    }
    let mut unique = Vec::new();
    for token in tokens {
        if !unique.contains(token) {
            unique.push(token.clone());
        }
    }
    if unique.is_empty() {
        None
    } else {
        Some(unique.join(" OR "))
    }
}

fn tokenize_query(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .filter_map(|token| {
            let cleaned: String = token
                .chars()
                .filter(|ch| ch.is_ascii_alphanumeric())
                .collect();
            let cleaned = cleaned.to_lowercase();
            if cleaned.len() < 3 {
                return None;
            }
            if is_stopword(&cleaned) {
                return None;
            }
            Some(cleaned)
        })
        .take(12)
        .collect()
}

fn is_stopword(token: &str) -> bool {
    matches!(
        token,
        "a"
            | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "been"
            | "but"
            | "by"
            | "can"
            | "could"
            | "did"
            | "do"
            | "does"
            | "for"
            | "from"
            | "had"
            | "has"
            | "have"
            | "how"
            | "if"
            | "in"
            | "is"
            | "it"
            | "its"
            | "me"
            | "of"
            | "on"
            | "or"
            | "our"
            | "please"
            | "should"
            | "tell"
            | "that"
            | "the"
            | "their"
            | "them"
            | "then"
            | "there"
            | "these"
            | "they"
            | "this"
            | "to"
            | "was"
            | "we"
            | "were"
            | "what"
            | "when"
            | "where"
            | "which"
            | "who"
            | "why"
            | "with"
            | "would"
            | "you"
            | "your"
            | "about"
    )
}

fn looks_sensitive_query(query: &str) -> bool {
    let lower = query.to_lowercase();
    if lower.contains('@') && lower.contains('.') {
        return true;
    }

    let digits = query.chars().filter(|ch| ch.is_ascii_digit()).count();
    if digits >= 7 {
        return true;
    }

    let address_markers = [
        "street", "st.", "road", "rd.", "avenue", "ave", "blvd", "boulevard", "drive", "dr.",
        "lane", "ln.", "address", "postal", "postcode", "zip",
    ];
    address_markers
        .iter()
        .any(|marker| lower.contains(marker))
}

async fn fts_available(db: &D1Database) -> ApiResult<bool> {
    let stmt =
        db.prepare("SELECT name FROM sqlite_master WHERE name = 'chunks_fts' LIMIT 1");
    let result = stmt
        .all()
        .await
        .map_err(|err| ApiError::internal(format!("D1 query failed: {err}")))?;
    let rows: Vec<NameRow> = result
        .results()
        .map_err(|err| ApiError::internal(format!("D1 parse failed: {err}")))?;
    Ok(!rows.is_empty())
}

async fn count_table(db: &D1Database, table: &str) -> ApiResult<u64> {
    if !matches!(table, "docs" | "chunks") {
        return Err(ApiError::internal("Invalid table name"));
    }
    let query = format!("SELECT COUNT(*) as count FROM {table}");
    let result = db
        .prepare(&query)
        .all()
        .await
        .map_err(|err| ApiError::internal(format!("D1 query failed: {err}")))?;
    let rows: Vec<CountRow> = result
        .results()
        .map_err(|err| ApiError::internal(format!("D1 parse failed: {err}")))?;
    Ok(rows.first().map(|row| row.count.max(0) as u64).unwrap_or(0))
}

async fn load_sync_state(db: &D1Database) -> ApiResult<Option<SyncState>> {
    let stmt = db.prepare("SELECT value FROM sync_state WHERE key = ?1");
    let result = stmt
        .bind(&[JsValue::from_str(SYNC_STATE_KEY)])
        .map_err(|err| ApiError::internal(format!("D1 bind failed: {err}")))?
        .all()
        .await
        .map_err(|err| ApiError::internal(format!("D1 query failed: {err}")))?;
    let rows: Vec<SyncStateRow> = result
        .results()
        .map_err(|err| ApiError::internal(format!("D1 parse failed: {err}")))?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };

    let state = serde_json::from_str::<SyncState>(&row.value)
        .map_err(|err| ApiError::internal(format!("Sync state decode failed: {err}")))?;
    Ok(Some(state))
}

async fn save_sync_state(db: &D1Database, state: &SyncState) -> ApiResult<()> {
    let payload = serde_json::to_string(state)
        .map_err(|err| ApiError::internal(format!("Sync state encode failed: {err}")))?;
    let stmt = db.prepare(
        "INSERT INTO sync_state (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    );
    stmt.bind(&[JsValue::from_str(SYNC_STATE_KEY), JsValue::from_str(&payload)])
        .map_err(|err| ApiError::internal(format!("D1 bind failed: {err}")))?
        .run()
        .await
        .map_err(|err| ApiError::internal(format!("D1 write failed: {err}")))?;
    Ok(())
}

async fn sync_kb(env: &Env) -> ApiResult<()> {
    sync_kb_with_since(env, None).await
}

async fn sync_kb_with_since(env: &Env, override_since: Option<u64>) -> ApiResult<()> {
    let settings = Settings::from_env(env)?;
    if settings.nostr_relays.is_empty() {
        return Ok(());
    }

    let db = env
        .d1("AMAN_KB")
        .map_err(|_| ApiError::internal("D1 binding AMAN_KB is missing"))?;

    let now = now_unix();
    let mut state = load_sync_state(&db).await?.unwrap_or(SyncState {
        since: now.saturating_sub(settings.kb_sync_lookback_secs),
        updated_at: 0,
    });
    if let Some(since) = override_since {
        state.since = since;
    }

    let since = state.since.saturating_sub(1);
    let fts_enabled = fts_available(&db).await.unwrap_or(false);
    let mut max_created_at = state.since;

    for relay in &settings.nostr_relays {
        let filter = nostr::NostrFilter {
            kinds: Some(vec![nostr::KIND_DOC_MANIFEST, nostr::KIND_CHUNK_REF]),
            since: Some(since),
            authors: settings
                .nostr_kb_author
                .clone()
                .map(|author| vec![author]),
            limit: None,
        };

        match nostr::fetch_relay_events(relay, &filter, NOSTR_RELAY_TIMEOUT_MS).await {
            Ok(events) => {
                for raw in events {
                    if let Some(author) = settings.nostr_kb_author.as_ref() {
                        if raw.event.pubkey != *author {
                            continue;
                        }
                    }
                    max_created_at = max_created_at.max(raw.event.created_at);
                    if let Err(err) = handle_nostr_event(&db, &raw, &settings, fts_enabled).await
                    {
                        console_error!("KB ingest failed: {}", err.message);
                    }
                }
            }
            Err(err) => {
                console_error!("Relay sync failed for {}: {}", relay, err.message);
            }
        }
    }

    state.updated_at = now;
    if max_created_at > state.since {
        state.since = max_created_at;
    }
    save_sync_state(&db, &state).await?;

    if let Ok(meta) = env.kv("AMAN_META") {
        match meta.put("kb:last_sync_at", state.updated_at.to_string()) {
            Ok(builder) => {
                if let Err(err) = builder.execute().await {
                    console_error!("KV write failed: {err}");
                }
            }
            Err(err) => {
                console_error!("KV write failed: {err}");
            }
        }
        match meta.put("kb:last_checkpoint", state.since.to_string()) {
            Ok(builder) => {
                if let Err(err) = builder.execute().await {
                    console_error!("KV write failed: {err}");
                }
            }
            Err(err) => {
                console_error!("KV write failed: {err}");
            }
        }
    }

    Ok(())
}

async fn handle_nostr_event(
    db: &D1Database,
    raw: &nostr::NostrRawEvent,
    settings: &Settings,
    fts_enabled: bool,
) -> ApiResult<()> {
    upsert_nostr_event(db, raw).await?;

    let content = match decode_event_content(&raw.event, settings.nostr_secretbox_key.as_ref()) {
        Ok(content) => content,
        Err(err) => {
            console_error!("Failed to decode event {}: {}", raw.event.id, err.message);
            return Ok(());
        }
    };

    match raw.event.kind {
        nostr::KIND_DOC_MANIFEST => {
            let manifest: DocManifestPayload = match serde_json::from_str(&content) {
                Ok(manifest) => manifest,
                Err(err) => {
                    console_error!("Doc manifest parse failed: {err}");
                    return Ok(());
                }
            };
            upsert_doc_manifest(db, &raw.event, &manifest).await?;
        }
        nostr::KIND_CHUNK_REF => {
            let chunk: ChunkRefPayload = match serde_json::from_str(&content) {
                Ok(chunk) => chunk,
                Err(err) => {
                    console_error!("Chunk ref parse failed: {err}");
                    return Ok(());
                }
            };
            upsert_chunk_ref(db, &raw.event, &chunk, fts_enabled).await?;
        }
        _ => {}
    }

    Ok(())
}

fn decode_event_content(
    event: &nostr::NostrEvent,
    secretbox_key: Option<&SecretBoxKey>,
) -> ApiResult<String> {
    if let Some(enc) = event.tag_value("enc") {
        if enc != SECRETBOX_TAG {
            return Err(ApiError::internal(format!(
                "Unsupported encryption tag: {enc}"
            )));
        }
        let key = secretbox_key
            .ok_or_else(|| ApiError::internal("NOSTR_SECRETBOX_KEY is missing"))?;
        return decrypt_secretbox_payload(key, &event.content);
    }

    Ok(event.content.clone())
}

fn decrypt_secretbox_payload(key: &SecretBoxKey, content: &str) -> ApiResult<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content.trim())
        .map_err(|err| ApiError::internal(format!("Secretbox base64 failed: {err}")))?;
    if bytes.len() < 24 {
        return Err(ApiError::internal("Secretbox payload too short"));
    }
    let (nonce_bytes, ciphertext) = bytes.split_at(24);
    let cipher = XSalsa20Poly1305::new(Key::from_slice(&key.0));
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| ApiError::internal("Secretbox decrypt failed"))?;
    String::from_utf8(plaintext)
        .map_err(|err| ApiError::internal(format!("Secretbox utf8 failed: {err}")))
}

async fn upsert_nostr_event(db: &D1Database, raw: &nostr::NostrRawEvent) -> ApiResult<()> {
    let seen_at = now_unix();
    let d_tag = raw.event.tag_value("d");
    let stmt = db.prepare(
        "INSERT INTO nostr_events (event_id, kind, pubkey, created_at, d_tag, raw_json, seen_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
         ON CONFLICT(event_id) DO UPDATE SET seen_at = excluded.seen_at",
    );
    stmt.bind(&[
        JsValue::from_str(&raw.event.id),
        JsValue::from_f64(raw.event.kind as f64),
        JsValue::from_str(&raw.event.pubkey),
        JsValue::from_f64(raw.event.created_at as f64),
        js_value_opt_str(d_tag),
        JsValue::from_str(&raw.raw_json),
        JsValue::from_f64(seen_at as f64),
    ])
    .map_err(|err| ApiError::internal(format!("D1 bind failed: {err}")))?
    .run()
    .await
    .map_err(|err| ApiError::internal(format!("D1 write failed: {err}")))?;
    Ok(())
}

async fn upsert_doc_manifest(
    db: &D1Database,
    event: &nostr::NostrEvent,
    manifest: &DocManifestPayload,
) -> ApiResult<()> {
    let stmt = db.prepare(
        "INSERT INTO docs (doc_id, title, lang, mime, updated_at, manifest_event_id, content_hash, blob_ref) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(doc_id) DO UPDATE SET \
           title = excluded.title, \
           lang = excluded.lang, \
           mime = excluded.mime, \
           updated_at = excluded.updated_at, \
           manifest_event_id = excluded.manifest_event_id, \
           content_hash = excluded.content_hash, \
           blob_ref = excluded.blob_ref \
         WHERE excluded.updated_at >= IFNULL(docs.updated_at, 0)",
    );
    stmt.bind(&[
        JsValue::from_str(&manifest.doc_id),
        JsValue::from_str(&manifest.title),
        JsValue::from_str(&manifest.lang),
        JsValue::from_str(&manifest.mime),
        JsValue::from_f64(manifest.updated_at as f64),
        JsValue::from_str(&event.id),
        JsValue::from_str(&manifest.content_hash),
        js_value_opt_str(manifest.blob_ref.as_deref()),
    ])
    .map_err(|err| ApiError::internal(format!("D1 bind failed: {err}")))?
    .run()
    .await
    .map_err(|err| ApiError::internal(format!("D1 write failed: {err}")))?;
    Ok(())
}

async fn upsert_chunk_ref(
    db: &D1Database,
    event: &nostr::NostrEvent,
    chunk: &ChunkRefPayload,
    fts_enabled: bool,
) -> ApiResult<()> {
    let created_at = chunk.created_at.unwrap_or(event.created_at);
    let text = chunk.text.as_ref().map(|value| value.trim()).filter(|v| !v.is_empty());

    let stmt = db.prepare(
        "INSERT INTO chunks (chunk_id, doc_id, ord, chunk_hash, blob_ref, text, created_at, event_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(chunk_id) DO UPDATE SET \
           doc_id = excluded.doc_id, \
           ord = excluded.ord, \
           chunk_hash = excluded.chunk_hash, \
           blob_ref = excluded.blob_ref, \
           text = excluded.text, \
           created_at = excluded.created_at, \
           event_id = excluded.event_id \
         WHERE excluded.created_at >= IFNULL(chunks.created_at, 0)",
    );
    stmt.bind(&[
        JsValue::from_str(&chunk.chunk_id),
        JsValue::from_str(&chunk.doc_id),
        JsValue::from_f64(chunk.ord as f64),
        JsValue::from_str(&chunk.chunk_hash),
        js_value_opt_str(chunk.blob_ref.as_deref()),
        js_value_opt_str(text),
        JsValue::from_f64(created_at as f64),
        JsValue::from_str(&event.id),
    ])
    .map_err(|err| ApiError::internal(format!("D1 bind failed: {err}")))?
    .run()
    .await
    .map_err(|err| ApiError::internal(format!("D1 write failed: {err}")))?;

    if fts_enabled {
        if let Some(text) = text {
            if let Ok(title) = fetch_doc_title(db, &chunk.doc_id).await {
                if let Err(err) = update_fts_row(db, &chunk.chunk_id, &chunk.doc_id, text, title).await
                {
                    console_error!("FTS update failed: {}", err.message);
                }
            }
        }
    }

    Ok(())
}

async fn fetch_doc_title(db: &D1Database, doc_id: &str) -> ApiResult<Option<String>> {
    let stmt = db.prepare("SELECT title FROM docs WHERE doc_id = ?1 LIMIT 1");
    let result = stmt
        .bind(&[JsValue::from_str(doc_id)])
        .map_err(|err| ApiError::internal(format!("D1 bind failed: {err}")))?
        .all()
        .await
        .map_err(|err| ApiError::internal(format!("D1 query failed: {err}")))?;
    let rows: Vec<TitleRow> = result
        .results()
        .map_err(|err| ApiError::internal(format!("D1 parse failed: {err}")))?;
    Ok(rows.first().and_then(|row| row.title.clone()))
}

async fn update_fts_row(
    db: &D1Database,
    chunk_id: &str,
    doc_id: &str,
    text: &str,
    title: Option<String>,
) -> ApiResult<()> {
    db.prepare("DELETE FROM chunks_fts WHERE chunk_id = ?1")
        .bind(&[JsValue::from_str(chunk_id)])
        .map_err(|err| ApiError::internal(format!("D1 bind failed: {err}")))?
        .run()
        .await
        .map_err(|err| ApiError::internal(format!("D1 write failed: {err}")))?;

    db.prepare("INSERT INTO chunks_fts (text, doc_id, chunk_id, title) VALUES (?1, ?2, ?3, ?4)")
        .bind(&[
            JsValue::from_str(text),
            JsValue::from_str(doc_id),
            JsValue::from_str(chunk_id),
            js_value_opt_str(title.as_deref()),
        ])
        .map_err(|err| ApiError::internal(format!("D1 bind failed: {err}")))?
        .run()
        .await
        .map_err(|err| ApiError::internal(format!("D1 write failed: {err}")))?;
    Ok(())
}

fn js_value_opt_str(value: Option<&str>) -> JsValue {
    value.map(JsValue::from_str).unwrap_or_else(JsValue::null)
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
