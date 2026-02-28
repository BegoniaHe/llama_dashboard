use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tracing::info;

use crate::cli::RunArgs;

pub async fn execute(args: RunArgs) -> anyhow::Result<()> {
    let _backend = llama_core::LlamaBackend::init();

    info!(model = %args.model.display(), "Loading model for interactive chat…");

    let model_params = llama_core::ModelParams {
        n_gpu_layers: args.n_gpu_layers,
        ..Default::default()
    };
    let model = Arc::new(llama_core::LlamaModel::load_from_file(
        &args.model,
        &model_params,
    )?);

    let n_threads = args.threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4)
    });

    let ctx_params = llama_core::ContextParams {
        n_ctx: args.ctx_size,
        n_threads,
        n_threads_batch: n_threads,
        ..Default::default()
    };
    let ctx = llama_core::LlamaContext::new(model.clone(), &ctx_params)?;
    let ctx = Arc::new(Mutex::new(ctx));

    let template = model.chat_template();
    let system_msg = args
        .system
        .as_deref()
        .unwrap_or("You are a helpful assistant.");

    let mut history: Vec<llama_core::ChatMessage> = vec![llama_core::ChatMessage {
        role: "system".into(),
        content: system_msg.into(),
    }];

    println!("Model loaded. Type your message (Ctrl-D to quit).\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        stdout.flush()?;

        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            println!();
            break; // EOF
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        history.push(llama_core::ChatMessage {
            role: "user".into(),
            content: line.to_string(),
        });

        // Apply chat template
        let prompt = llama_core::apply_template(template.as_deref(), &history, true)
            .unwrap_or_else(|| {
                // Fallback: simple concatenation
                history
                    .iter()
                    .map(|m| format!("{}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\nassistant:"
            });

        let tokens = llama_core::tokenize(model.vocab(), &prompt, true, true)?;

        let sampling = llama_core::SamplingParams {
            temperature: args.temp,
            ..Default::default()
        };

        let request = llama_core::GenerateRequest {
            tokens,
            max_tokens: 2048,
            stop_words: vec![],
            sampling_params: sampling,
        };

        let (tx, mut rx) = mpsc::channel(64);

        // Move the Arc<Mutex<Context>> into the blocking task
        let ctx_clone = ctx.clone();
        tokio::task::spawn_blocking(move || {
            let mut ctx_guard = ctx_clone.lock().unwrap();
            ctx_guard.kv_cache_clear();
            llama_core::generate::generate_blocking(&mut ctx_guard, &request, tx);
        });

        let mut assistant_reply = String::new();

        while let Some(event) = rx.recv().await {
            match event {
                llama_core::GenerateEvent::Token(piece) => {
                    print!("{piece}");
                    stdout.flush()?;
                    assistant_reply.push_str(&piece);
                }
                llama_core::GenerateEvent::Done {
                    finish_reason,
                    prompt_tokens,
                    completion_tokens,
                } => {
                    println!();
                    eprintln!(
                        "  [{finish_reason} | prompt: {prompt_tokens} tok, gen: {completion_tokens} tok]"
                    );
                    break;
                }
                llama_core::GenerateEvent::Error(e) => {
                    eprintln!("\nError: {e}");
                    break;
                }
            }
        }

        history.push(llama_core::ChatMessage {
            role: "assistant".into(),
            content: assistant_reply,
        });

        println!();
    }

    Ok(())
}
