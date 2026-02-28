//! Management API routes: /api/models, /api/config, /api/system

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        // Model management
        .route("/api/models", get(list_models))
        .route("/api/models/scan", post(scan_models))
        .route("/api/models/{id}/details", get(model_details))
        .route("/api/models/{id}/load", post(load_model))
        .route("/api/models/{id}/unload", post(unload_model))
        .route("/api/models/{id}/favorite", put(toggle_favorite))
        // Config
        .route("/api/config", get(get_config).put(update_config))
        // System
        .route("/api/system/info", get(system_info))
}

//  Types

#[derive(Debug, Serialize)]
struct ModelEntry {
    id: String,
    filename: String,
    path: String,
    size: u64,
    architecture: Option<String>,
    parameters: Option<String>,
    context_length: Option<u64>,
    file_type: Option<String>,
    quantization: Option<String>,
    chat_template: Option<String>,
    status: &'static str,
    favorite: bool,
    alias: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoadModelRequest {
    #[serde(default = "default_ctx_size")]
    ctx_size: u32,
    #[serde(default = "default_gpu_layers")]
    n_gpu_layers: i32,
}

fn default_ctx_size() -> u32 {
    4096
}
fn default_gpu_layers() -> i32 {
    -1
}

#[derive(Debug, Serialize)]
struct ConfigResponse {
    model_dirs: Vec<String>,
    default_ctx_size: u32,
    default_n_gpu_layers: i32,
    default_temperature: f64,
    api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigUpdate {
    model_dirs: Option<Vec<String>>,
    default_ctx_size: Option<u32>,
    default_n_gpu_layers: Option<i32>,
    default_temperature: Option<f64>,
    api_key: Option<String>,
}

#[derive(Debug, Serialize)]
struct SystemInfoResponse {
    version: String,
    models_loaded: usize,
    models_available: usize,
}

//  Handlers

/// GET /api/models — list all discovered models with status
async fn list_models(State(state): State<AppState>) -> Json<Vec<ModelEntry>> {
    let available = state.model_manager().scan_available();
    let loaded_id = state.model_manager().loaded_model_id();

    let entries: Vec<ModelEntry> = available
        .into_iter()
        .map(|m| {
            let status = if loaded_id
                .as_ref()
                .map(|lid| lid.eq_ignore_ascii_case(&m.id))
                .unwrap_or(false)
            {
                "loaded"
            } else {
                "unloaded"
            };
            ModelEntry {
                id: m.id.clone(),
                filename: m.name.clone(),
                path: m.path.display().to_string(),
                size: m.file_size,
                architecture: m.architecture.clone(),
                parameters: None,
                context_length: m.context_length.map(|v| v as u64),
                file_type: m.quantization.clone(),
                quantization: m.quantization.clone(),
                chat_template: None,
                status,
                favorite: false,
                alias: None,
            }
        })
        .collect();

    Json(entries)
}

/// POST /api/models/scan — trigger directory rescan
async fn scan_models(State(state): State<AppState>) -> Json<serde_json::Value> {
    let entries = state.model_manager().scan_available();
    info!(count = entries.len(), "Model scan complete");
    Json(serde_json::json!({
        "scanned": entries.len()
    }))
}

/// GET /api/models/:id/details — get full model metadata
async fn model_details(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ModelEntry>, axum::http::StatusCode> {
    let available = state.model_manager().scan_available();
    let loaded_id = state.model_manager().loaded_model_id();

    let m = available
        .into_iter()
        .find(|m| m.id == id)
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    let status = if loaded_id
        .as_ref()
        .map(|lid| lid.eq_ignore_ascii_case(&m.id))
        .unwrap_or(false)
    {
        "loaded"
    } else {
        "unloaded"
    };

    Ok(Json(ModelEntry {
        id: m.id,
        filename: m.name,
        path: m.path.display().to_string(),
        size: m.file_size,
        architecture: m.architecture,
        parameters: None,
        context_length: m.context_length.map(|v| v as u64),
        file_type: m.quantization.clone(),
        quantization: m.quantization,
        chat_template: None,
        status,
        favorite: false,
        alias: None,
    }))
}

/// POST /api/models/:id/load — load a model by id
async fn load_model(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<LoadModelRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // Find model path
    let available = state.model_manager().scan_available();
    let entry = available.into_iter().find(|m| m.id == id).ok_or((
        axum::http::StatusCode::NOT_FOUND,
        format!("Model '{}' not found", id),
    ))?;

    let model_path = entry.path.clone();

    let model_params = llama_core::ModelParams {
        n_gpu_layers: req.n_gpu_layers,
        ..Default::default()
    };
    let ctx_params = llama_core::ContextParams {
        n_ctx: req.ctx_size,
        ..Default::default()
    };

    // Load in blocking task to avoid blocking the async runtime
    let mm = state.model_manager().clone();
    let load_result =
        tokio::task::spawn_blocking(move || mm.load(&model_path, &model_params, &ctx_params))
            .await
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match load_result {
        Ok(_) => {
            info!(id, "Model loaded via API");
            // Broadcast event
            state.broadcast_event("model.loaded", serde_json::json!({ "id": id }));
            Ok(Json(serde_json::json!({ "status": "loaded", "id": id })))
        }
        Err(e) => {
            error!(id, error = %e, "Failed to load model");
            Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// POST /api/models/:id/unload — unload a model
async fn unload_model(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    state.model_manager().unload();
    info!(id, "Model unloaded via API");
    state.broadcast_event("model.unloaded", serde_json::json!({ "id": id }));
    Json(serde_json::json!({ "status": "unloaded", "id": id }))
}

/// PUT /api/models/:id/favorite — toggle favorite
async fn toggle_favorite(Path(id): Path<String>) -> Json<serde_json::Value> {
    // TODO: persist favorite state in DB
    Json(serde_json::json!({ "id": id, "favorite": true }))
}

/// GET /api/config — get current configuration
async fn get_config(State(state): State<AppState>) -> Json<ConfigResponse> {
    let cfg = state.config();
    Json(ConfigResponse {
        model_dirs: cfg
            .model_dirs
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
        default_ctx_size: cfg.default_ctx_size,
        default_n_gpu_layers: cfg.default_n_gpu_layers,
        default_temperature: 0.7,
        api_key: cfg.api_key.clone(),
    })
}

/// PUT /api/config — update configuration
async fn update_config(
    State(state): State<AppState>,
    Json(update): Json<ConfigUpdate>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let mut cfg = state.config().clone();

    if let Some(dirs) = update.model_dirs {
        cfg.model_dirs = dirs.into_iter().map(std::path::PathBuf::from).collect();
    }
    if let Some(ctx) = update.default_ctx_size {
        cfg.default_ctx_size = ctx;
    }
    if let Some(ngl) = update.default_n_gpu_layers {
        cfg.default_n_gpu_layers = ngl;
    }
    if let Some(key) = update.api_key {
        cfg.api_key = if key.is_empty() { None } else { Some(key) };
    }

    cfg.save()
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

/// GET /api/system/info
async fn system_info(State(state): State<AppState>) -> Json<SystemInfoResponse> {
    let models_loaded = if state.model_manager().get_loaded().is_some() {
        1
    } else {
        0
    };
    let models_available = state.model_manager().scan_available().len();

    Json(SystemInfoResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        models_loaded,
        models_available,
    })
}
