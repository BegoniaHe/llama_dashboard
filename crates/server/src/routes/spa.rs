//! SPA fallback: serves the embedded Vue 3 frontend.
//!
//! In development, the Vite dev server handles this.
//! In production, the built frontend is embedded via rust-embed.

use axum::{
    Router,
    body::Body,
    extract::Request,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::Embed;

use crate::state::AppState;

#[derive(Embed)]
#[folder = "../../frontend/dist"]
#[prefix = ""]
struct FrontendAssets;

pub fn router() -> Router<AppState> {
    Router::new().fallback(get(spa_handler))
}

async fn spa_handler(req: Request) -> impl IntoResponse {
    let path = req.uri().path().trim_start_matches('/');

    // Try to serve the exact file
    if let Some(content) = FrontendAssets::get(path) {
        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
            .body(Body::from(content.data.to_vec()))
            .unwrap();
    }

    // SPA fallback: serve index.html for all unmatched routes
    match FrontendAssets::get("index.html") {
        Some(content) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(content.data.to_vec()))
            .unwrap(),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(
                "Frontend not found. Run `npm run build` in frontend/ first.",
            ))
            .unwrap(),
    }
}
