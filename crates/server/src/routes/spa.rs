//! SPA fallback: serves the embedded Vue 3 frontend.
//!
//! When the `embed-frontend` feature is enabled (default), the built
//! frontend from `frontend/dist` is embedded into the binary via
//! rust-embed.  When the feature is disabled (e.g. Docker builds
//! without the frontend), a simple stub message is returned instead.

use axum::{
    Router,
    body::Body,
    extract::Request,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

use crate::state::AppState;

// ── Embedded frontend (feature = "embed-frontend") ─────────────────

#[cfg(feature = "embed-frontend")]
mod embedded {
    use rust_embed::Embed;

    #[derive(Embed)]
    #[folder = "../../frontend/dist"]
    #[prefix = ""]
    pub struct FrontendAssets;
}

pub fn router() -> Router<AppState> {
    Router::new().fallback(get(spa_handler))
}

#[cfg(feature = "embed-frontend")]
async fn spa_handler(req: Request) -> impl IntoResponse {
    let path = req.uri().path().trim_start_matches('/');

    // Try to serve the exact file
    if let Some(content) = embedded::FrontendAssets::get(path) {
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
    match embedded::FrontendAssets::get("index.html") {
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

#[cfg(not(feature = "embed-frontend"))]
async fn spa_handler(_req: Request) -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(
            "<!DOCTYPE html><html><body>\
             <h1>llama-dashboard API server</h1>\
             <p>Frontend not embedded. Build with <code>--features embed-frontend</code> \
             or use the Vite dev server.</p>\
             </body></html>",
        ))
        .unwrap()
}
