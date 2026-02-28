//! Application configuration — persisted as JSON.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Global configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub model_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub api_key: Option<String>,
    /// Default context size (0 = model default).
    #[serde(default)]
    pub default_ctx_size: u32,
    /// Default GPU layers (-1 = all).
    #[serde(default = "default_gpu_layers")]
    pub default_n_gpu_layers: i32,
}

fn default_host() -> String {
    "127.0.0.1".into()
}
fn default_port() -> u16 {
    8080
}
fn default_gpu_layers() -> i32 {
    -1
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            model_dirs: Vec::new(),
            api_key: None,
            default_ctx_size: 0,
            default_n_gpu_layers: default_gpu_layers(),
        }
    }
}

impl AppConfig {
    /// Platform config directory: `~/.config/llama-dashboard/`
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("llama-dashboard")
    }

    fn config_file() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn db_path(&self) -> PathBuf {
        Self::config_dir().join("data.db")
    }

    /// Load from disk, or return defaults if the file doesn't exist.
    pub fn load_or_default() -> anyhow::Result<Self> {
        let path = Self::config_file();
        if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&data)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Persist to disk.
    pub fn save(&self) -> anyhow::Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(Self::config_file(), data)?;
        Ok(())
    }
}
