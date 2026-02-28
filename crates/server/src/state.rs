//! Shared application state injected into Axum handlers.

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::config::AppConfig;
use crate::db::Database;
use crate::services::model_manager::ModelManager;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    pub config: AppConfig,
    #[allow(dead_code)]
    pub db: Database,
    pub model_manager: ModelManager,
    #[allow(dead_code)]
    pub api_key: Option<String>,
    pub event_tx: broadcast::Sender<String>,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        db: Database,
        model_manager: ModelManager,
        api_key: Option<String>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            inner: Arc::new(Inner {
                config,
                db,
                model_manager,
                api_key,
                event_tx,
            }),
        }
    }

    pub fn config(&self) -> &AppConfig {
        &self.inner.config
    }
    #[allow(dead_code)]
    pub fn db(&self) -> &Database {
        &self.inner.db
    }
    pub fn model_manager(&self) -> &ModelManager {
        &self.inner.model_manager
    }
    #[allow(dead_code)]
    pub fn api_key(&self) -> Option<&str> {
        self.inner.api_key.as_deref()
    }

    /// Broadcast an event to all connected WebSocket clients.
    pub fn broadcast_event(&self, event_type: &str, data: serde_json::Value) {
        let event = serde_json::json!({
            "type": event_type,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "data": data,
        });
        // Ignore send errors (no subscribers)
        let _ = self.inner.event_tx.send(event.to_string());
    }

    /// Subscribe to the event broadcast channel.
    pub fn subscribe_events(&self) -> broadcast::Receiver<String> {
        self.inner.event_tx.subscribe()
    }

    /// Get a clone of the event broadcast sender (used by idle checker).
    pub fn event_tx(&self) -> broadcast::Sender<String> {
        self.inner.event_tx.clone()
    }
}
