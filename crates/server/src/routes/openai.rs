//! OpenAI-compatible API routes:
//!   GET    /v1/models
//!   GET    /v1/models/{model}
//!   DELETE /v1/models/{model}
//!   POST   /v1/chat/completions
//!   POST   /v1/completions
//!   POST   /v1/embeddings

use std::convert::Infallible;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tracing::error;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/models", get(list_models))
        .route(
            "/v1/models/{model}",
            get(retrieve_model).delete(delete_model),
        )
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/completions", post(completions))
        .route("/v1/embeddings", post(embeddings))
}

//  Error response (OpenAI format)

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    message: String,
    r#type: String,
    param: Option<String>,
    code: Option<String>,
}

fn api_error(status: StatusCode, message: impl Into<String>, error_type: &str) -> Response {
    (
        status,
        Json(ErrorBody {
            error: ErrorDetail {
                message: message.into(),
                r#type: error_type.to_string(),
                param: None,
                code: None,
            },
        }),
    )
        .into_response()
}

//  Shared types

#[derive(Serialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

//  /v1/models

#[derive(Serialize)]
struct ModelObject {
    id: String,
    object: &'static str,
    created: i64,
    owned_by: &'static str,
}

#[derive(Serialize)]
struct ModelsListResponse {
    object: &'static str,
    data: Vec<ModelObject>,
}

/// GET /v1/models — List available models.
async fn list_models(State(state): State<AppState>) -> Json<ModelsListResponse> {
    let mut data = Vec::new();

    // Include all loaded models
    let loaded_ids = state.model_manager().loaded_model_ids();
    for id in &loaded_ids {
        data.push(ModelObject {
            id: id.clone(),
            object: "model",
            created: 0,
            owned_by: "local",
        });
    }

    // Also include scanned but not loaded models
    let available = state.model_manager().scan_available();
    for m in available {
        if loaded_ids.iter().any(|lid| lid.eq_ignore_ascii_case(&m.id)) {
            continue; // already listed
        }
        data.push(ModelObject {
            id: m.id,
            object: "model",
            created: 0,
            owned_by: "local",
        });
    }

    Json(ModelsListResponse {
        object: "list",
        data,
    })
}

/// GET /v1/models/{model} — Retrieve a single model.
async fn retrieve_model(State(state): State<AppState>, Path(model_id): Path<String>) -> Response {
    // Check loaded models
    if let Some(loaded) = state.model_manager().get_loaded(&model_id) {
        return Json(ModelObject {
            id: loaded.id.clone(),
            object: "model",
            created: 0,
            owned_by: "local",
        })
        .into_response();
    }

    // Check scanned models
    let available = state.model_manager().scan_available();
    if let Some(m) = available
        .into_iter()
        .find(|m| m.id.eq_ignore_ascii_case(&model_id))
    {
        return Json(ModelObject {
            id: m.id,
            object: "model",
            created: 0,
            owned_by: "local",
        })
        .into_response();
    }

    api_error(
        StatusCode::NOT_FOUND,
        format!("The model '{}' does not exist", model_id),
        "invalid_request_error",
    )
}

/// DELETE /v1/models/{model} — Unload a model.
async fn delete_model(State(state): State<AppState>, Path(model_id): Path<String>) -> Response {
    if state.model_manager().is_loaded(&model_id) {
        state.model_manager().unload(&model_id);
        state.broadcast_event("model.unloaded", serde_json::json!({ "id": model_id }));

        #[derive(Serialize)]
        struct DeleteResponse {
            id: String,
            object: &'static str,
            deleted: bool,
        }
        return Json(DeleteResponse {
            id: model_id,
            object: "model",
            deleted: true,
        })
        .into_response();
    }

    api_error(
        StatusCode::NOT_FOUND,
        format!("The model '{}' does not exist or is not loaded", model_id),
        "invalid_request_error",
    )
}

//  /v1/chat/completions

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ChatCompletionRequest {
    #[serde(default)]
    model: Option<String>,
    messages: Vec<ChatMessageReq>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    max_completion_tokens: Option<u32>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    n: Option<u32>,
    #[serde(default)]
    stream: Option<bool>,
    #[serde(default)]
    stop: Option<StopSequence>,
    #[serde(default)]
    frequency_penalty: Option<f32>,
    #[serde(default)]
    presence_penalty: Option<f32>,
    #[serde(default)]
    logprobs: Option<bool>,
    #[serde(default)]
    top_logprobs: Option<u32>,
    #[serde(default)]
    seed: Option<u32>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    response_format: Option<ResponseFormat>,
}

/// OpenAI `stop` can be a string or an array of strings.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StopSequence {
    Single(String),
    Multiple(Vec<String>),
}

impl StopSequence {
    fn into_vec(self) -> Vec<String> {
        match self {
            StopSequence::Single(s) => vec![s],
            StopSequence::Multiple(v) => v,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ResponseFormat {
    r#type: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ChatMessageReq {
    role: String,
    #[serde(default)]
    content: Option<ChatContent>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tool_calls: Option<serde_json::Value>,
    #[serde(default)]
    tool_call_id: Option<String>,
}

/// Content can be a string or array of content parts (text/image_url).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ChatContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl ChatContent {
    fn as_text(&self) -> String {
        match self {
            ChatContent::Text(s) => s.clone(),
            ChatContent::Parts(parts) => parts
                .iter()
                .filter_map(|p| {
                    if p.r#type == "text" {
                        p.text.clone()
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(""),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ContentPart {
    r#type: String,
    #[serde(default)]
    text: Option<String>,
}

// Chat completion response

#[derive(Serialize)]
struct ChatCompletionResponse {
    id: String,
    object: &'static str,
    created: i64,
    model: String,
    choices: Vec<ChatChoice>,
    usage: Usage,
    system_fingerprint: Option<String>,
}

#[derive(Serialize)]
struct ChatChoice {
    index: u32,
    message: ChatMessageResp,
    finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logprobs: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct ChatMessageResp {
    role: &'static str,
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<serde_json::Value>,
}

// Streaming chunk

#[derive(Serialize)]
struct ChatCompletionChunk {
    id: String,
    object: &'static str,
    created: i64,
    model: String,
    choices: Vec<ChatChunkChoice>,
    system_fingerprint: Option<String>,
}

#[derive(Serialize)]
struct ChatChunkChoice {
    index: u32,
    delta: ChatDelta,
    finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logprobs: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct ChatDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
}

/// Resolve the model for a request: try by name, fall back to any loaded.
#[allow(clippy::result_large_err)]
fn resolve_model(
    state: &AppState,
    model_name: Option<&str>,
) -> Result<std::sync::Arc<crate::services::model_manager::LoadedModel>, Response> {
    let mm = state.model_manager();
    match mm.resolve(model_name) {
        Some(loaded) => {
            mm.touch(&loaded.id);
            Ok(loaded)
        }
        None => Err(api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            model_name
                .map(|n| format!("Model '{}' is not loaded", n))
                .unwrap_or_else(|| "No model loaded".to_string()),
            "server_error",
        )),
    }
}

/// POST /v1/chat/completions — Chat completion (stream + non-stream).
async fn chat_completions(
    State(state): State<AppState>,
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    let stream = req.stream.unwrap_or(false);

    let loaded = match resolve_model(&state, req.model.as_deref()) {
        Ok(l) => l,
        Err(e) => return e,
    };

    let model_id = loaded.id.clone();
    let model = loaded.model.clone();

    // Build chat messages
    let messages: Vec<llama_core::ChatMessage> = req
        .messages
        .iter()
        .map(|m| llama_core::ChatMessage {
            role: m.role.clone(),
            content: m.content.as_ref().map(|c| c.as_text()).unwrap_or_default(),
        })
        .collect();

    let template = model.chat_template();
    let prompt =
        llama_core::apply_template(template.as_deref(), &messages, true).unwrap_or_else(|| {
            messages
                .iter()
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n")
                + "\nassistant:"
        });

    let tokens = match llama_core::tokenize(model.vocab(), &prompt, true, true) {
        Ok(t) => t,
        Err(e) => {
            return api_error(
                StatusCode::BAD_REQUEST,
                format!("Tokenization failed: {e}"),
                "invalid_request_error",
            );
        }
    };

    let max_tokens = req.max_completion_tokens.or(req.max_tokens).unwrap_or(2048);

    let sampling = llama_core::SamplingParams {
        temperature: req.temperature.unwrap_or(0.8),
        top_p: req.top_p.unwrap_or(0.95),
        frequency_penalty: req.frequency_penalty.unwrap_or(0.0),
        presence_penalty: req.presence_penalty.unwrap_or(0.0),
        seed: req.seed,
        ..Default::default()
    };

    let gen_req = llama_core::GenerateRequest {
        tokens,
        max_tokens,
        stop_words: req.stop.map(|s| s.into_vec()).unwrap_or_default(),
        sampling_params: sampling,
    };

    let request_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = chrono::Utc::now().timestamp();
    let fingerprint = format!("fp_{}", &model_id[..model_id.len().min(8)]);

    if stream {
        chat_stream(loaded, gen_req, request_id, created, model_id, fingerprint).into_response()
    } else {
        chat_non_stream(loaded, gen_req, request_id, created, model_id, fingerprint)
            .await
            .into_response()
    }
}

fn chat_stream(
    loaded: std::sync::Arc<crate::services::model_manager::LoadedModel>,
    gen_req: llama_core::GenerateRequest,
    request_id: String,
    created: i64,
    model_id: String,
    fingerprint: String,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel(64);

    tokio::task::spawn_blocking(move || {
        let mut ctx = loaded.context.lock().unwrap();
        ctx.kv_cache_clear();
        llama_core::generate::generate_blocking(&mut ctx, &gen_req, tx);
    });

    let rid = request_id.clone();
    let mid = model_id.clone();
    let fp = fingerprint.clone();
    let mut sent_role = false;

    let stream = ReceiverStream::new(rx).map(move |event| {
        let chunk = match event {
            llama_core::GenerateEvent::Token(piece) => {
                let role = if !sent_role {
                    sent_role = true;
                    Some("assistant".to_string())
                } else {
                    None
                };
                ChatCompletionChunk {
                    id: rid.clone(),
                    object: "chat.completion.chunk",
                    created,
                    model: mid.clone(),
                    choices: vec![ChatChunkChoice {
                        index: 0,
                        delta: ChatDelta {
                            role,
                            content: Some(piece),
                        },
                        finish_reason: None,
                        logprobs: None,
                    }],
                    system_fingerprint: Some(fp.clone()),
                }
            }
            llama_core::GenerateEvent::Done { finish_reason, .. } => {
                let reason = match finish_reason {
                    llama_core::FinishReason::Stop => "stop",
                    llama_core::FinishReason::Length => "length",
                    llama_core::FinishReason::StopWord(_) => "stop",
                };
                ChatCompletionChunk {
                    id: rid.clone(),
                    object: "chat.completion.chunk",
                    created,
                    model: mid.clone(),
                    choices: vec![ChatChunkChoice {
                        index: 0,
                        delta: ChatDelta {
                            role: None,
                            content: None,
                        },
                        finish_reason: Some(reason.to_string()),
                        logprobs: None,
                    }],
                    system_fingerprint: Some(fp.clone()),
                }
            }
            llama_core::GenerateEvent::Error(e) => {
                error!("Generation error: {e}");
                ChatCompletionChunk {
                    id: rid.clone(),
                    object: "chat.completion.chunk",
                    created,
                    model: mid.clone(),
                    choices: vec![],
                    system_fingerprint: Some(fp.clone()),
                }
            }
        };
        Ok(Event::default().data(serde_json::to_string(&chunk).unwrap_or_default()))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn chat_non_stream(
    loaded: std::sync::Arc<crate::services::model_manager::LoadedModel>,
    gen_req: llama_core::GenerateRequest,
    request_id: String,
    created: i64,
    model_id: String,
    fingerprint: String,
) -> Json<ChatCompletionResponse> {
    let (tx, mut rx) = mpsc::channel(64);

    tokio::task::spawn_blocking(move || {
        let mut ctx = loaded.context.lock().unwrap();
        ctx.kv_cache_clear();
        llama_core::generate::generate_blocking(&mut ctx, &gen_req, tx);
    });

    let mut content = String::new();
    let mut finish_reason = None;
    let mut prompt_tokens = 0u32;
    let mut completion_tokens = 0u32;

    while let Some(event) = rx.recv().await {
        match event {
            llama_core::GenerateEvent::Token(piece) => content.push_str(&piece),
            llama_core::GenerateEvent::Done {
                finish_reason: fr,
                prompt_tokens: pt,
                completion_tokens: ct,
            } => {
                finish_reason = Some(match fr {
                    llama_core::FinishReason::Stop => "stop".to_string(),
                    llama_core::FinishReason::Length => "length".to_string(),
                    llama_core::FinishReason::StopWord(_) => "stop".to_string(),
                });
                prompt_tokens = pt;
                completion_tokens = ct;
            }
            llama_core::GenerateEvent::Error(e) => {
                error!("Generation error: {e}");
                break;
            }
        }
    }

    Json(ChatCompletionResponse {
        id: request_id,
        object: "chat.completion",
        created,
        model: model_id,
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessageResp {
                role: "assistant",
                content: Some(content),
                tool_calls: None,
            },
            finish_reason,
            logprobs: None,
        }],
        usage: Usage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        },
        system_fingerprint: Some(fingerprint),
    })
}

//  /v1/completions (legacy text completions)

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CompletionRequest {
    #[serde(default)]
    model: Option<String>,
    prompt: PromptInput,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    n: Option<u32>,
    #[serde(default)]
    stream: Option<bool>,
    #[serde(default)]
    stop: Option<StopSequence>,
    #[serde(default)]
    frequency_penalty: Option<f32>,
    #[serde(default)]
    presence_penalty: Option<f32>,
    #[serde(default)]
    logprobs: Option<u32>,
    #[serde(default)]
    echo: Option<bool>,
    #[serde(default)]
    suffix: Option<String>,
    #[serde(default)]
    seed: Option<u32>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    best_of: Option<u32>,
}

/// OpenAI `prompt` can be a string, array of strings, or token array.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PromptInput {
    Single(String),
    Multiple(Vec<String>),
    Tokens(Vec<i32>),
}

impl PromptInput {
    fn as_text(&self) -> String {
        match self {
            PromptInput::Single(s) => s.clone(),
            PromptInput::Multiple(v) => v.join(""),
            PromptInput::Tokens(_) => String::new(), // handled separately
        }
    }

    fn as_tokens(&self) -> Option<&[i32]> {
        match self {
            PromptInput::Tokens(t) => Some(t),
            _ => None,
        }
    }
}

#[derive(Serialize)]
struct CompletionResponse {
    id: String,
    object: &'static str,
    created: i64,
    model: String,
    choices: Vec<CompletionChoice>,
    usage: Usage,
    system_fingerprint: Option<String>,
}

#[derive(Serialize)]
struct CompletionChoice {
    index: u32,
    text: String,
    finish_reason: Option<String>,
    logprobs: Option<serde_json::Value>,
}

// Streaming completion chunk

#[derive(Serialize)]
struct CompletionChunk {
    id: String,
    object: &'static str,
    created: i64,
    model: String,
    choices: Vec<CompletionChunkChoice>,
    system_fingerprint: Option<String>,
}

#[derive(Serialize)]
struct CompletionChunkChoice {
    index: u32,
    text: String,
    finish_reason: Option<String>,
    logprobs: Option<serde_json::Value>,
}

/// POST /v1/completions — Text completion (legacy).
async fn completions(
    State(state): State<AppState>,
    Json(req): Json<CompletionRequest>,
) -> Response {
    let stream = req.stream.unwrap_or(false);
    let echo = req.echo.unwrap_or(false);

    let loaded = match resolve_model(&state, req.model.as_deref()) {
        Ok(l) => l,
        Err(e) => return e,
    };

    let model_id = loaded.id.clone();
    let model = loaded.model.clone();

    // Tokenize prompt
    let tokens = if let Some(tok) = req.prompt.as_tokens() {
        tok.to_vec()
    } else {
        let prompt_text = req.prompt.as_text();
        match llama_core::tokenize(model.vocab(), &prompt_text, true, true) {
            Ok(t) => t,
            Err(e) => {
                return api_error(
                    StatusCode::BAD_REQUEST,
                    format!("Tokenization failed: {e}"),
                    "invalid_request_error",
                );
            }
        }
    };

    let prompt_text = if echo {
        req.prompt.as_text()
    } else {
        String::new()
    };

    let sampling = llama_core::SamplingParams {
        temperature: req.temperature.unwrap_or(1.0),
        top_p: req.top_p.unwrap_or(1.0),
        frequency_penalty: req.frequency_penalty.unwrap_or(0.0),
        presence_penalty: req.presence_penalty.unwrap_or(0.0),
        seed: req.seed,
        ..Default::default()
    };

    let gen_req = llama_core::GenerateRequest {
        tokens,
        max_tokens: req.max_tokens.unwrap_or(16),
        stop_words: req.stop.map(|s| s.into_vec()).unwrap_or_default(),
        sampling_params: sampling,
    };

    let request_id = format!("cmpl-{}", uuid::Uuid::new_v4());
    let created = chrono::Utc::now().timestamp();
    let fingerprint = format!("fp_{}", &model_id[..model_id.len().min(8)]);

    if stream {
        completion_stream(
            loaded,
            gen_req,
            request_id,
            created,
            model_id,
            fingerprint,
            prompt_text,
        )
        .into_response()
    } else {
        completion_non_stream(
            loaded,
            gen_req,
            request_id,
            created,
            model_id,
            fingerprint,
            prompt_text,
        )
        .await
        .into_response()
    }
}

fn completion_stream(
    loaded: std::sync::Arc<crate::services::model_manager::LoadedModel>,
    gen_req: llama_core::GenerateRequest,
    request_id: String,
    created: i64,
    model_id: String,
    fingerprint: String,
    echo_prefix: String,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel(64);

    tokio::task::spawn_blocking(move || {
        let mut ctx = loaded.context.lock().unwrap();
        ctx.kv_cache_clear();
        llama_core::generate::generate_blocking(&mut ctx, &gen_req, tx);
    });

    let rid = request_id.clone();
    let mid = model_id.clone();
    let fp = fingerprint.clone();
    let mut sent_echo = echo_prefix.is_empty();

    let stream = ReceiverStream::new(rx).map(move |event| {
        // If echo, first emit the prompt as a chunk
        let prefix_text = if !sent_echo {
            sent_echo = true;
            echo_prefix.clone()
        } else {
            String::new()
        };

        let chunk = match event {
            llama_core::GenerateEvent::Token(piece) => CompletionChunk {
                id: rid.clone(),
                object: "text_completion",
                created,
                model: mid.clone(),
                choices: vec![CompletionChunkChoice {
                    index: 0,
                    text: format!("{}{}", prefix_text, piece),
                    finish_reason: None,
                    logprobs: None,
                }],
                system_fingerprint: Some(fp.clone()),
            },
            llama_core::GenerateEvent::Done { finish_reason, .. } => {
                let reason = match finish_reason {
                    llama_core::FinishReason::Stop => "stop",
                    llama_core::FinishReason::Length => "length",
                    llama_core::FinishReason::StopWord(_) => "stop",
                };
                CompletionChunk {
                    id: rid.clone(),
                    object: "text_completion",
                    created,
                    model: mid.clone(),
                    choices: vec![CompletionChunkChoice {
                        index: 0,
                        text: String::new(),
                        finish_reason: Some(reason.to_string()),
                        logprobs: None,
                    }],
                    system_fingerprint: Some(fp.clone()),
                }
            }
            llama_core::GenerateEvent::Error(e) => {
                error!("Generation error: {e}");
                CompletionChunk {
                    id: rid.clone(),
                    object: "text_completion",
                    created,
                    model: mid.clone(),
                    choices: vec![],
                    system_fingerprint: Some(fp.clone()),
                }
            }
        };
        Ok(Event::default().data(serde_json::to_string(&chunk).unwrap_or_default()))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn completion_non_stream(
    loaded: std::sync::Arc<crate::services::model_manager::LoadedModel>,
    gen_req: llama_core::GenerateRequest,
    request_id: String,
    created: i64,
    model_id: String,
    fingerprint: String,
    echo_prefix: String,
) -> Json<CompletionResponse> {
    let (tx, mut rx) = mpsc::channel(64);

    tokio::task::spawn_blocking(move || {
        let mut ctx = loaded.context.lock().unwrap();
        ctx.kv_cache_clear();
        llama_core::generate::generate_blocking(&mut ctx, &gen_req, tx);
    });

    let mut content = echo_prefix;
    let mut finish_reason = None;
    let mut prompt_tokens = 0u32;
    let mut completion_tokens = 0u32;

    while let Some(event) = rx.recv().await {
        match event {
            llama_core::GenerateEvent::Token(piece) => content.push_str(&piece),
            llama_core::GenerateEvent::Done {
                finish_reason: fr,
                prompt_tokens: pt,
                completion_tokens: ct,
            } => {
                finish_reason = Some(match fr {
                    llama_core::FinishReason::Stop => "stop".to_string(),
                    llama_core::FinishReason::Length => "length".to_string(),
                    llama_core::FinishReason::StopWord(_) => "stop".to_string(),
                });
                prompt_tokens = pt;
                completion_tokens = ct;
            }
            llama_core::GenerateEvent::Error(e) => {
                error!("Generation error: {e}");
                break;
            }
        }
    }

    Json(CompletionResponse {
        id: request_id,
        object: "text_completion",
        created,
        model: model_id,
        choices: vec![CompletionChoice {
            index: 0,
            text: content,
            finish_reason,
            logprobs: None,
        }],
        usage: Usage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        },
        system_fingerprint: Some(fingerprint),
    })
}

//  /v1/embeddings

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct EmbeddingRequest {
    #[serde(default)]
    model: Option<String>,
    input: EmbeddingInput,
    #[serde(default)]
    encoding_format: Option<String>,
    #[serde(default)]
    user: Option<String>,
}

/// Input can be a string, array of strings, or token array(s).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[allow(dead_code)]
enum EmbeddingInput {
    Single(String),
    Multiple(Vec<String>),
    Tokens(Vec<Vec<i32>>),
    SingleTokens(Vec<i32>),
}

impl EmbeddingInput {
    fn into_texts(self) -> Vec<String> {
        match self {
            EmbeddingInput::Single(s) => vec![s],
            EmbeddingInput::Multiple(v) => v,
            EmbeddingInput::Tokens(_) | EmbeddingInput::SingleTokens(_) => vec![],
        }
    }
}

#[derive(Serialize)]
struct EmbeddingResponse {
    object: &'static str,
    data: Vec<EmbeddingData>,
    model: String,
    usage: EmbeddingUsage,
}

#[derive(Serialize)]
struct EmbeddingData {
    object: &'static str,
    index: usize,
    embedding: Vec<f32>,
}

#[derive(Serialize)]
struct EmbeddingUsage {
    prompt_tokens: u32,
    total_tokens: u32,
}

/// POST /v1/embeddings — Get embeddings for text.
///
/// Note: embedding support depends on the model. Standard chat models
/// may not produce meaningful embeddings. A dedicated embedding model
/// (e.g. nomic-embed) is recommended.
async fn embeddings(State(state): State<AppState>, Json(req): Json<EmbeddingRequest>) -> Response {
    let loaded = match resolve_model(&state, req.model.as_deref()) {
        Ok(l) => l,
        Err(e) => return e,
    };

    let model_id = loaded.id.clone();
    let model = loaded.model.clone();
    let texts = req.input.into_texts();

    if texts.is_empty() {
        return api_error(
            StatusCode::BAD_REQUEST,
            "Input must be a non-empty string or array of strings",
            "invalid_request_error",
        );
    }

    let n_embd = model.n_embd() as usize;

    // Create a temporary context with embeddings enabled.
    // This is separate from the inference context so that normal chat/completions
    // are not affected by the embeddings flag.
    let model_clone = model.clone();
    let results = tokio::task::spawn_blocking(move || {
        let emb_ctx_params = llama_core::ContextParams {
            n_ctx: 4096,
            embeddings: true,
            ..Default::default()
        };
        let mut emb_ctx = llama_core::LlamaContext::new(model_clone.clone(), &emb_ctx_params)
            .map_err(|e| format!("Failed to create embeddings context: {e}"))?;

        let vocab = model_clone.vocab();
        let mut all_embeddings: Vec<(Vec<f32>, u32)> = Vec::new();

        for text in &texts {
            emb_ctx.kv_cache_clear();

            let tokens = match llama_core::tokenize(vocab, text, true, true) {
                Ok(t) => t,
                Err(e) => return Err(format!("Tokenization failed: {e}")),
            };
            let n_tokens = tokens.len() as u32;

            // Create a batch with the tokens
            let mut batch = llama_core::LlamaBatch::new(tokens.len() as i32, 0, 1);
            for (i, &token) in tokens.iter().enumerate() {
                batch.add(token, i as i32, &[0], i == tokens.len() - 1);
            }

            // Decode
            if let Err(e) = emb_ctx.decode(&mut batch) {
                return Err(format!("Decode failed: {e}"));
            }

            // Get embeddings
            match emb_ctx.get_embeddings() {
                Some(emb) => {
                    let embedding = emb[..n_embd].to_vec();
                    // L2 normalize
                    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                    let normalized = if norm > 0.0 {
                        embedding.iter().map(|x| x / norm).collect()
                    } else {
                        embedding
                    };
                    all_embeddings.push((normalized, n_tokens));
                }
                None => {
                    return Err("Model does not support embeddings extraction.".to_string());
                }
            }
        }

        Ok(all_embeddings)
    })
    .await;

    match results {
        Ok(Ok(embeddings_data)) => {
            let mut total_tokens = 0u32;
            let data: Vec<EmbeddingData> = embeddings_data
                .into_iter()
                .enumerate()
                .map(|(i, (emb, n_tok))| {
                    total_tokens += n_tok;
                    EmbeddingData {
                        object: "embedding",
                        index: i,
                        embedding: emb,
                    }
                })
                .collect();

            Json(EmbeddingResponse {
                object: "list",
                data,
                model: model_id,
                usage: EmbeddingUsage {
                    prompt_tokens: total_tokens,
                    total_tokens,
                },
            })
            .into_response()
        }
        Ok(Err(e)) => api_error(StatusCode::BAD_REQUEST, e, "invalid_request_error"),
        Err(e) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
            "server_error",
        ),
    }
}
