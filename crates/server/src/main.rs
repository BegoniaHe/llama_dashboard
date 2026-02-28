mod cli;
mod config;
mod db;
mod middleware;
mod routes;
mod services;
mod state;

use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    //  Logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,llama_dashboard=debug")),
        )
        .init();

    let args = cli::Cli::parse();

    match args.command {
        Some(cli::Commands::Run(run_args)) => cli::run::execute(run_args).await,
        Some(cli::Commands::Models(m)) => cli::models::execute(m).await,
        Some(cli::Commands::Config(c)) => cli::config_cmd::execute(c).await,
        // Default: start HTTP server
        Some(cli::Commands::Serve(serve_args)) => {
            cli::serve::execute(args.global, serve_args).await
        }
        None => {
            cli::serve::execute(
                args.global,
                cli::ServeArgs {
                    model: None,
                    ctx_size: 4096,
                    n_gpu_layers: -1,
                    max_models: 4,
                    idle_timeout: 0,
                },
            )
            .await
        }
    }
}
