//! WebSocket events: /ws/events
//!
//! Broadcasts real-time events (model state changes, system events) to
//! connected WebSocket clients.

use axum::{
    Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use tracing::{info, warn};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/ws/events", get(ws_handler))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.subscribe_events();

    info!("WebSocket client connected");

    // Forward broadcast events to WebSocket
    let send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if sender.send(Message::Text(event.into())).await.is_err() {
                break;
            }
        }
    });

    // Read messages from client (pings, close frames)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Close(_) = msg {
                break;
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    warn!("WebSocket client disconnected");
}
