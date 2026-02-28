//! Native llama.cpp API routes: /tokenize, /detokenize

use axum::{Json, Router, extract::State, routing::post};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tokenize", post(tokenize))
        .route("/detokenize", post(detokenize))
}

#[derive(Deserialize)]
struct TokenizeRequest {
    content: String,
    #[serde(default = "default_true")]
    add_special: bool,
    #[serde(default)]
    parse_special: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize)]
struct TokenizeResponse {
    tokens: Vec<i32>,
}

#[derive(Deserialize)]
struct DetokenizeRequest {
    tokens: Vec<i32>,
}

#[derive(Serialize)]
struct DetokenizeResponse {
    content: String,
}

async fn tokenize(
    State(state): State<AppState>,
    Json(req): Json<TokenizeRequest>,
) -> Result<Json<TokenizeResponse>, axum::http::StatusCode> {
    let loaded = state
        .model_manager()
        .get_loaded()
        .ok_or(axum::http::StatusCode::SERVICE_UNAVAILABLE)?;

    let vocab = loaded.model.vocab();
    let tokens = llama_core::tokenize(vocab, &req.content, req.add_special, req.parse_special)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    Ok(Json(TokenizeResponse { tokens }))
}

async fn detokenize(
    State(state): State<AppState>,
    Json(req): Json<DetokenizeRequest>,
) -> Result<Json<DetokenizeResponse>, axum::http::StatusCode> {
    let loaded = state
        .model_manager()
        .get_loaded()
        .ok_or(axum::http::StatusCode::SERVICE_UNAVAILABLE)?;

    let vocab = loaded.model.vocab();
    let content = llama_core::detokenize(vocab, &req.tokens)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    Ok(Json(DetokenizeResponse { content }))
}
