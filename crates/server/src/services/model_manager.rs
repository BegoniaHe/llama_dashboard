//! Multi-model manager (Phase 2 — M2).
//!
//! Manages multiple concurrently-loaded models with:
//! - Per-model slots with state machine (Unloaded → Loading → Ready → Unloading)
//! - Serialised loading via `Mutex` (one model loads at a time)
//! - LRU eviction based on `last_used` timestamps
//! - `Arc::strong_count` reference counting to prevent eviction during inference
//! - Idle timeout with background sweeper
//!
//! Design references:
//! - llama.cpp Router Mode (server-models.h / server-context.cpp)
//! - Ollama scheduler (sched.go)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use tracing::{info, warn};

//  Types

/// Status of a model slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    Loading,
    Ready,
    Unloading,
}

/// A loaded model together with its inference context.
pub struct LoadedModel {
    pub id: String,
    pub path: PathBuf,
    pub model: Arc<llama_core::LlamaModel>,
    pub context: Mutex<llama_core::LlamaContext>,
}

/// Metadata for one model slot visible from the outside.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SlotInfo {
    pub id: String,
    pub path: String,
    pub status: ModelStatus,
    pub last_used: u64, // millis since manager creation
}

/// Internal slot tracked by the manager.
struct ModelSlot {
    status: ModelStatus,
    loaded: Option<Arc<LoadedModel>>,
    last_used: Instant,
}

/// Configuration for the model manager.
#[derive(Debug, Clone)]
pub struct ModelManagerConfig {
    /// Maximum number of concurrently loaded models (0 = unlimited).
    pub max_models: usize,
    /// Idle timeout in seconds (0 = disabled).
    #[allow(dead_code)]
    pub idle_timeout_secs: u64,
    /// Default model params for auto-loading.
    #[allow(dead_code)]
    pub default_n_gpu_layers: i32,
    /// Default context size for auto-loading.
    #[allow(dead_code)]
    pub default_ctx_size: u32,
}

impl Default for ModelManagerConfig {
    fn default() -> Self {
        Self {
            max_models: 4,
            idle_timeout_secs: 0,
            default_n_gpu_layers: -1,
            default_ctx_size: 4096,
        }
    }
}

//  ModelManager

#[derive(Clone)]
pub struct ModelManager {
    /// id → slot
    slots: Arc<RwLock<HashMap<String, ModelSlot>>>,
    /// Serialises loading (only one model loads at a time).
    load_lock: Arc<Mutex<()>>,
    model_dirs: Arc<RwLock<Vec<PathBuf>>>,
    config: Arc<ModelManagerConfig>,
    epoch: Instant,
}

impl ModelManager {
    pub fn new(model_dirs: Vec<PathBuf>, config: ModelManagerConfig) -> Self {
        Self {
            slots: Arc::new(RwLock::new(HashMap::new())),
            load_lock: Arc::new(Mutex::new(())),
            model_dirs: Arc::new(RwLock::new(model_dirs)),
            config: Arc::new(config),
            epoch: Instant::now(),
        }
    }

    //  Directory management

    /// Add a directory to the scan list.
    pub fn add_model_dir(&self, dir: PathBuf) {
        let mut dirs = self.model_dirs.write().unwrap();
        if !dirs.contains(&dir) {
            info!(dir = %dir.display(), "Added model directory");
            dirs.push(dir);
        }
    }

    /// Scan configured directories for available models.
    pub fn scan_available(&self) -> Vec<gguf_parser::ModelEntry> {
        let dirs = self.model_dirs.read().unwrap();
        let mut all = Vec::new();
        for dir in dirs.iter() {
            match gguf_parser::scan_directory(dir) {
                Ok(entries) => all.extend(entries),
                Err(e) => warn!(dir = %dir.display(), "Scan failed: {e}"),
            }
        }
        all
    }

    //  Loading / Unloading

    /// Load a model from `path`, returns an `Arc<LoadedModel>`.
    ///
    /// If `max_models` would be exceeded, the least-recently-used model
    /// (with no active references) is evicted first.
    pub fn load(
        &self,
        path: &Path,
        model_params: &llama_core::ModelParams,
        ctx_params: &llama_core::ContextParams,
    ) -> Result<Arc<LoadedModel>, llama_core::LlamaError> {
        let id = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Serialise loading
        let _guard = self.load_lock.lock().unwrap();

        // If already loaded, just touch + return
        {
            let slots = self.slots.read().unwrap();
            if let Some(slot) = slots.get(&id)
                && slot.status == ModelStatus::Ready
                && slot.loaded.is_some()
            {
                let loaded = slot.loaded.clone().unwrap();
                drop(slots);
                self.touch(&id);
                info!(id, "Model already loaded, returning existing");
                return Ok(loaded);
            }
        }

        // Evict if needed
        self.maybe_evict();

        // Mark loading
        {
            let mut slots = self.slots.write().unwrap();
            slots.insert(
                id.clone(),
                ModelSlot {
                    status: ModelStatus::Loading,
                    loaded: None,
                    last_used: Instant::now(),
                },
            );
        }

        // Actually load
        let result = (|| {
            let model = Arc::new(llama_core::LlamaModel::load_from_file(path, model_params)?);
            let ctx = llama_core::LlamaContext::new(model.clone(), ctx_params)?;
            Ok(Arc::new(LoadedModel {
                id: id.clone(),
                path: path.to_path_buf(),
                model,
                context: Mutex::new(ctx),
            }))
        })();

        match result {
            Ok(loaded) => {
                let mut slots = self.slots.write().unwrap();
                slots.insert(
                    id.clone(),
                    ModelSlot {
                        status: ModelStatus::Ready,
                        loaded: Some(loaded.clone()),
                        last_used: Instant::now(),
                    },
                );

                // Auto-register the model's parent directory
                if let Some(parent) = path.parent() {
                    let canonical =
                        std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
                    drop(slots);
                    self.add_model_dir(canonical);
                }

                info!(id, "Model loaded and ready");
                Ok(loaded)
            }
            Err(e) => {
                // Remove the Loading slot
                let mut slots = self.slots.write().unwrap();
                slots.remove(&id);
                Err(e)
            }
        }
    }

    /// Unload a specific model by id.
    pub fn unload(&self, id: &str) -> bool {
        let mut slots = self.slots.write().unwrap();
        if let Some(slot) = slots.get_mut(id) {
            slot.status = ModelStatus::Unloading;
            slot.loaded.take();
            slots.remove(id);
            info!(id, "Model unloaded");
            true
        } else {
            false
        }
    }

    /// Unload all models.
    #[allow(dead_code)]
    pub fn unload_all(&self) {
        let mut slots = self.slots.write().unwrap();
        let ids: Vec<String> = slots.keys().cloned().collect();
        for id in &ids {
            info!(id, "Unloading model");
        }
        slots.clear();
    }

    //  Queries

    /// Get a reference to a loaded model by id.
    pub fn get_loaded(&self, id: &str) -> Option<Arc<LoadedModel>> {
        let slots = self.slots.read().unwrap();
        slots
            .get(id)
            .filter(|s| s.status == ModelStatus::Ready)
            .and_then(|s| s.loaded.clone())
    }

    /// Get any one loaded model (for backwards compatibility / default model).
    pub fn get_any_loaded(&self) -> Option<Arc<LoadedModel>> {
        let slots = self.slots.read().unwrap();
        slots
            .values()
            .filter(|s| s.status == ModelStatus::Ready)
            .max_by_key(|s| s.last_used)
            .and_then(|s| s.loaded.clone())
    }

    /// Return all loaded model IDs.
    pub fn loaded_model_ids(&self) -> Vec<String> {
        let slots = self.slots.read().unwrap();
        slots
            .iter()
            .filter(|(_, s)| s.status == ModelStatus::Ready)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Check if a model is currently loaded.
    pub fn is_loaded(&self, id: &str) -> bool {
        let slots = self.slots.read().unwrap();
        slots
            .get(id)
            .map(|s| s.status == ModelStatus::Ready)
            .unwrap_or(false)
    }

    /// Number of currently loaded models.
    pub fn loaded_count(&self) -> usize {
        let slots = self.slots.read().unwrap();
        slots
            .values()
            .filter(|s| s.status == ModelStatus::Ready)
            .count()
    }

    /// Get information about all slots.
    pub fn slot_info(&self) -> Vec<SlotInfo> {
        let slots = self.slots.read().unwrap();
        slots
            .iter()
            .map(|(id, s)| SlotInfo {
                id: id.clone(),
                path: s
                    .loaded
                    .as_ref()
                    .map(|l| l.path.display().to_string())
                    .unwrap_or_default(),
                status: s.status,
                last_used: s.last_used.duration_since(self.epoch).as_millis() as u64,
            })
            .collect()
    }

    /// Update LRU timestamp for a model.
    pub fn touch(&self, id: &str) {
        let mut slots = self.slots.write().unwrap();
        if let Some(slot) = slots.get_mut(id) {
            slot.last_used = Instant::now();
        }
    }

    /// Find a model path by scanning directories for a matching model id.
    pub fn find_model_path(&self, model_id: &str) -> Option<PathBuf> {
        let available = self.scan_available();
        available
            .into_iter()
            .find(|m| m.id.eq_ignore_ascii_case(model_id))
            .map(|m| m.path)
    }

    //  Resolve (for route handlers)

    /// Resolve a model: if a model name is given, try to get it from
    /// loaded models. Otherwise return the most recently used model.
    pub fn resolve(&self, model_name: Option<&str>) -> Option<Arc<LoadedModel>> {
        match model_name {
            Some(name) => self.get_loaded(name),
            None => self.get_any_loaded(),
        }
    }

    /// Same as `resolve` but also tries auto-loading if the model is
    /// not currently loaded.  This is a **blocking** call.
    #[allow(dead_code)]
    pub fn ensure_loaded(
        &self,
        model_name: Option<&str>,
    ) -> Result<Arc<LoadedModel>, llama_core::LlamaError> {
        // Try loaded first
        if let Some(loaded) = self.resolve(model_name) {
            self.touch(&loaded.id);
            return Ok(loaded);
        }

        // Auto-load if model name is provided
        if let Some(name) = model_name
            && let Some(path) = self.find_model_path(name)
        {
            let model_params = llama_core::ModelParams {
                n_gpu_layers: self.config.default_n_gpu_layers,
                ..Default::default()
            };
            let ctx_params = llama_core::ContextParams {
                n_ctx: self.config.default_ctx_size,
                ..Default::default()
            };
            return self.load(&path, &model_params, &ctx_params);
        }

        Err(llama_core::LlamaError::ContextCreationFailed(
            "No model loaded and no model name specified".into(),
        ))
    }

    //  LRU eviction

    /// Evict the least-recently-used model if we're at capacity.
    fn maybe_evict(&self) {
        let max = self.config.max_models;
        if max == 0 {
            return; // unlimited
        }

        let mut slots = self.slots.write().unwrap();
        while slots
            .values()
            .filter(|s| s.status == ModelStatus::Ready || s.status == ModelStatus::Loading)
            .count()
            >= max
        {
            // Find the LRU model with no active external refs
            let victim = slots
                .iter()
                .filter(|(_, s)| s.status == ModelStatus::Ready)
                .filter(|(_, s)| {
                    // Only evict if nobody else holds a reference.
                    // The slot itself holds 1 ref; if strong_count > 1
                    // someone is actively using it.
                    s.loaded
                        .as_ref()
                        .map(|l| Arc::strong_count(l) <= 1)
                        .unwrap_or(true)
                })
                .min_by_key(|(_, s)| s.last_used)
                .map(|(id, _)| id.clone());

            match victim {
                Some(id) => {
                    info!(id, "Evicting LRU model to make room");
                    slots.remove(&id);
                }
                None => {
                    warn!("Cannot evict: all loaded models have active references");
                    break;
                }
            }
        }
    }

    /// Sweep idle models (called from background task).
    pub fn sweep_idle(&self, timeout_secs: u64) {
        if timeout_secs == 0 {
            return;
        }
        let cutoff = Instant::now() - std::time::Duration::from_secs(timeout_secs);
        let mut slots = self.slots.write().unwrap();
        let idle: Vec<String> = slots
            .iter()
            .filter(|(_, s)| s.status == ModelStatus::Ready)
            .filter(|(_, s)| s.last_used < cutoff)
            .filter(|(_, s)| {
                s.loaded
                    .as_ref()
                    .map(|l| Arc::strong_count(l) <= 1)
                    .unwrap_or(true)
            })
            .map(|(id, _)| id.clone())
            .collect();

        for id in idle {
            info!(id, "Unloading idle model (timeout={}s)", timeout_secs);
            slots.remove(&id);
        }
    }
}

/// Spawn a background task that periodically sweeps idle models.
pub fn spawn_idle_checker(
    manager: ModelManager,
    idle_timeout_secs: u64,
    mut shutdown: tokio::sync::broadcast::Receiver<String>,
) {
    if idle_timeout_secs == 0 {
        return;
    }

    let interval = std::time::Duration::from_secs(idle_timeout_secs.max(30).min(idle_timeout_secs));
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.tick().await; // skip first immediate tick
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    manager.sweep_idle(idle_timeout_secs);
                }
                _ = shutdown.recv() => {
                    break;
                }
            }
        }
    });
}
