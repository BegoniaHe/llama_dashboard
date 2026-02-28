use crate::cli::ModelsArgs;

pub async fn execute(args: ModelsArgs) -> anyhow::Result<()> {
    match args.action {
        crate::cli::ModelsAction::List { dir } => {
            let search_dir = dir.unwrap_or_else(|| std::path::PathBuf::from("."));
            if !search_dir.is_dir() {
                anyhow::bail!("{} is not a directory", search_dir.display());
            }

            let entries =
                gguf_parser::scan_directory(&search_dir).map_err(|e| anyhow::anyhow!("{e}"))?;

            if entries.is_empty() {
                println!("No GGUF models found in {}", search_dir.display());
                return Ok(());
            }

            println!("{:<40} {:<12} {:<10} {:<8}", "Name", "Quant", "Size", "Ctx");
            println!("{}", "-".repeat(74));
            for entry in &entries {
                let size = human_size(entry.file_size);
                let quant = entry.quantization.as_deref().unwrap_or("-");
                let ctx = entry
                    .context_length
                    .map(|c| format!("{c}"))
                    .unwrap_or_else(|| "-".into());
                println!("{:<40} {:<12} {:<10} {:<8}", entry.name, quant, size, ctx);
            }
            println!("\n{} model(s) found.", entries.len());
        }
        crate::cli::ModelsAction::Info { path } => {
            let scan = gguf_parser::quick_scan(&path).map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&scan)?);
        }
    }
    Ok(())
}

fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    for &unit in UNITS {
        if size < 1024.0 {
            return format!("{size:.1} {unit}");
        }
        size /= 1024.0;
    }
    format!("{size:.1} PiB")
}
