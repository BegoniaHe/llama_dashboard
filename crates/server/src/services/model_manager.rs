//! Single-model manager (Phase 1).
//!
//! Tracks one loaded model + context.  Phase 2 will extend this to
//! multi-model with LRU eviction.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use tracing::{info, warn};

/// Handle to a loaded model + inference context.
pub struct LoadedModel {
    pub id: String,
    pub path: PathBuf,
    pub model: Arc<llama_core::LlamaModel>,
    pub context: Mutex<llama_core::LlamaContext>,
}

#[derive(Clone)]
pub struct ModelManager {
    loaded: Arc<RwLock<Option<Arc<LoadedModel>>>>,
    model_dirs: Arc<RwLock<Vec<PathBuf>>>,
}

impl ModelManager {
    pub fn new(model_dirs: Vec<PathBuf>) -> Self {
        Self {
            loaded: Arc::new(RwLock::new(None)),
            model_dirs: Arc::new(RwLock::new(model_dirs)),
        }
    }

    /// Add a directory to the scan list.
    pub fn add_model_dir(&self, dir: PathBuf) {
        let mut dirs = self.model_dirs.write().unwrap();
        if !dirs.contains(&dir) {
            info!(dir = %dir.display(), "Added model directory");
            dirs.push(dir);
        }
    }

    /// Load a model from `path`.  Unloads any previously loaded model.
    pub fn load(
        &self,
        path: &Path,
        model_params: &llama_core::ModelParams,
        ctx_params: &llama_core::ContextParams,
    ) -> Result<Arc<LoadedModel>, llama_core::LlamaError> {
        let model = Arc::new(llama_core::LlamaModel::load_from_file(path, model_params)?);
        let ctx = llama_core::LlamaContext::new(model.clone(), ctx_params)?;

        let id = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let loaded = Arc::new(LoadedModel {
            id: id.clone(),
            path: path.to_path_buf(),
            model,
            context: Mutex::new(ctx),
        });

        let mut slot = self.loaded.write().unwrap();
        if slot.is_some() {
            info!("Unloading previous model");
        }
        *slot = Some(loaded.clone());

        // Auto-register the model's parent directory for scanning
        if let Some(parent) = path.parent() {
            let canonical = std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
            self.add_model_dir(canonical);
        }

        info!(id, "Model loaded and ready");
        Ok(loaded)
    }

    /// Unload the current model.
    pub fn unload(&self) {
        let mut slot = self.loaded.write().unwrap();
        if slot.take().is_some() {
            info!("Model unloaded");
        }
    }

    /// Get a reference to the loaded model
    pub fn get_loaded(&self) -> Option<Arc<LoadedModel>> {
        self.loaded.read().unwrap().clone()
    }

    /// Return the model id of the loaded model.
    pub fn loaded_model_id(&self) -> Option<String> {
        self.loaded.read().unwrap().as_ref().map(|l| l.id.clone())
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
}
