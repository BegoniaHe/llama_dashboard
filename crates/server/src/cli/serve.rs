use std::net::SocketAddr;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::cli::{GlobalArgs, ServeArgs};
use crate::config::AppConfig;
use crate::db::Database;
use crate::routes;
use crate::services::model_manager::ModelManager;
use crate::state::AppState;

pub async fn execute(global: GlobalArgs, serve_args: ServeArgs) -> anyhow::Result<()> {
    let _backend = llama_core::LlamaBackend::init();

    //  Config / DB
    let cfg = AppConfig::load_or_default()?;
    let db = Database::open(&cfg.db_path())?;

    //  Model manager
    let model_dirs: Vec<std::path::PathBuf> = if global.models_dirs.is_empty() {
        cfg.model_dirs.clone()
    } else {
        global.models_dirs.clone()
    };
    let model_manager = ModelManager::new(model_dirs);

    //  Pre-load model if specified
    if let Some(model_path) = &serve_args.model {
        let model_params = llama_core::ModelParams {
            n_gpu_layers: serve_args.n_gpu_layers,
            ..Default::default()
        };
        let ctx_params = llama_core::ContextParams {
            n_ctx: serve_args.ctx_size,
            ..Default::default()
        };
        model_manager
            .load(model_path, &model_params, &ctx_params)
            .map_err(|e| anyhow::anyhow!("Failed to pre-load model: {e}"))?;
    }

    //  Shared state
    let state = AppState::new(cfg.clone(), db, model_manager, global.api_key.clone());

    //  Router
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(routes::health::router())
        .merge(routes::openai::router())
        .merge(routes::native::router())
        .merge(routes::management::router())
        .merge(routes::ws::router())
        .merge(routes::spa::router())
        .layer(cors)
        .with_state(state);

    // Wrap with auth middleware if key is set.
    // (Phase 2: per-route middleware; for now auth is checked in handlers.)

    let addr: SocketAddr = format!("{}:{}", global.host, global.port).parse()?;
    info!(%addr, "Starting server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
