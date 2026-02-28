use crate::cli::ConfigArgs;
use crate::config::AppConfig;

pub async fn execute(args: ConfigArgs) -> anyhow::Result<()> {
    match args.action {
        crate::cli::ConfigAction::Show => {
            let cfg = AppConfig::load_or_default()?;
            println!("{}", serde_json::to_string_pretty(&cfg)?);
        }
        crate::cli::ConfigAction::Set { key, value } => {
            let mut cfg = AppConfig::load_or_default()?;
            match key.as_str() {
                "port" => cfg.port = value.parse()?,
                "host" => cfg.host = value,
                _ => anyhow::bail!("Unknown config key: {key}"),
            }
            cfg.save()?;
            println!("Configuration updated.");
        }
    }
    Ok(())
}
