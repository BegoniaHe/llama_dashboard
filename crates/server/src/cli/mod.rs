pub mod config_cmd;
pub mod models;
pub mod run;
pub mod serve;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "llama-dashboard",
    version,
    about = "Local LLM management platform powered by llama.cpp"
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, clap::Args, Clone)]
pub struct GlobalArgs {
    /// Listen address.
    #[arg(long, default_value = "127.0.0.1", env = "LLAMA_HOST")]
    pub host: String,

    /// Listen port.
    #[arg(short, long, default_value_t = 8080, env = "LLAMA_PORT")]
    pub port: u16,

    /// Model search directories (can be repeated).
    #[arg(long = "models-dir", env = "LLAMA_MODELS_DIR")]
    pub models_dirs: Vec<std::path::PathBuf>,

    /// Optional API key for bearer-token auth.
    #[arg(long, env = "LLAMA_API_KEY")]
    pub api_key: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the HTTP API server (default).
    Serve(ServeArgs),

    /// Load a model and start an interactive chat.
    Run(RunArgs),

    /// Manage discovered models.
    Models(ModelsArgs),

    /// View / edit configuration.
    Config(ConfigArgs),
}

//  Subcommand argument structs

#[derive(Debug, clap::Args, Clone)]
pub struct ServeArgs {
    /// Pre-load this model on startup.
    #[arg(long)]
    pub model: Option<std::path::PathBuf>,

    /// Context size (default: 4096, 0 = model default).
    #[arg(long, default_value_t = 4096)]
    pub ctx_size: u32,

    /// GPU layers (-1 = all, 0 = CPU only).
    #[arg(long, default_value_t = -1)]
    pub n_gpu_layers: i32,

    /// Maximum number of concurrently loaded models (0 = unlimited).
    #[arg(long = "models-max", default_value_t = 4, env = "LLAMA_MODELS_MAX")]
    pub max_models: usize,

    /// Idle timeout in seconds; unload models after this period (0 = disabled).
    #[arg(long = "idle-timeout", default_value_t = 0, env = "LLAMA_IDLE_TIMEOUT")]
    pub idle_timeout: u64,
}

#[derive(Debug, clap::Args, Clone)]
pub struct RunArgs {
    /// Path to a GGUF model file.
    pub model: std::path::PathBuf,

    /// Context size (0 = model default).
    #[arg(long, default_value_t = 0)]
    pub ctx_size: u32,

    /// GPU layers (-1 = all).
    #[arg(long, default_value_t = -1)]
    pub n_gpu_layers: i32,

    /// Threads.
    #[arg(long)]
    pub threads: Option<i32>,

    /// Temperature.
    #[arg(long, default_value_t = 0.8)]
    pub temp: f32,

    /// System prompt.
    #[arg(long)]
    pub system: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct ModelsArgs {
    #[command(subcommand)]
    pub action: ModelsAction,
}

#[derive(Debug, Subcommand)]
pub enum ModelsAction {
    /// List available models.
    List {
        /// Directory to scan (overrides config).
        #[arg(long)]
        dir: Option<std::path::PathBuf>,
    },
    /// Show detailed info about a GGUF file.
    Info {
        /// Path to the GGUF file.
        path: std::path::PathBuf,
    },
}

#[derive(Debug, clap::Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Display the current configuration.
    Show,
    /// Set a configuration value.
    Set { key: String, value: String },
}
