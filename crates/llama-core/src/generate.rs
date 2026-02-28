//! Streaming token generation.

use tokio::sync::mpsc;
use tracing::debug;

use crate::batch::LlamaBatch;
use crate::context::LlamaContext;
use crate::sampler::SamplingParams;
use crate::token::token_to_piece;

/// Parameters for a generation request.
#[derive(Debug, Clone)]
pub struct GenerateRequest {
    /// Pre-tokenized prompt.
    pub tokens: Vec<i32>,
    /// Maximum tokens to generate.
    pub max_tokens: u32,
    /// Stop-word strings.
    pub stop_words: Vec<String>,
    /// Sampling configuration.
    pub sampling_params: SamplingParams,
}

/// Events emitted during streaming generation.
#[derive(Debug, Clone)]
pub enum GenerateEvent {
    /// A new text piece was decoded.
    Token(String),
    /// Generation finished.
    Done {
        finish_reason: FinishReason,
        prompt_tokens: u32,
        completion_tokens: u32,
    },
    /// An error occurred mid-generation.
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    /// Natural stop (EOS / EOT token).
    Stop,
    /// Reached `max_tokens`.
    Length,
    /// Matched a stop word.
    StopWord(String),
}

impl std::fmt::Display for FinishReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stop => write!(f, "stop"),
            Self::Length => write!(f, "length"),
            Self::StopWord(w) => write!(f, "stop_word:{w}"),
        }
    }
}

/// Run a synchronous (blocking) generation loop.
///
/// This is intended to be called inside `tokio::task::spawn_blocking`.
/// Produced tokens are sent over `tx`; the function returns when
/// generation finishes or the receiver is dropped.
pub fn generate_blocking(
    ctx: &mut LlamaContext,
    request: &GenerateRequest,
    tx: mpsc::Sender<GenerateEvent>,
) {
    let vocab = ctx.model().vocab();
    let n_ctx = ctx.n_ctx() as i32;
    let eos = ctx.model().token_eos();
    let eot = ctx.model().token_eot();

    //  Prompt processing
    let batch_cap = request.tokens.len().max(1) as i32;
    let mut batch = LlamaBatch::new(batch_cap, 0, 1);
    for (i, &tok) in request.tokens.iter().enumerate() {
        let logits = i == request.tokens.len() - 1;
        batch.add(tok, i as i32, &[0], logits);
    }

    if let Err(e) = ctx.decode(&mut batch) {
        let _ = tx.blocking_send(GenerateEvent::Error(format!("prompt decode: {e}")));
        return;
    }

    let prompt_tokens = request.tokens.len() as u32;
    let mut n_cur = request.tokens.len() as i32;
    let mut completion_tokens = 0u32;
    let mut generated_text = String::new();
    let mut sampler = request.sampling_params.clone().into_chain();

    //  Token generation loop
    loop {
        // Max-tokens guard
        if completion_tokens >= request.max_tokens {
            let _ = tx.blocking_send(GenerateEvent::Done {
                finish_reason: FinishReason::Length,
                prompt_tokens,
                completion_tokens,
            });
            break;
        }

        let new_token = sampler.sample(ctx, batch.n_tokens() - 1);
        completion_tokens += 1;

        // EOS / EOT
        if new_token == eos || new_token == eot {
            let _ = tx.blocking_send(GenerateEvent::Done {
                finish_reason: FinishReason::Stop,
                prompt_tokens,
                completion_tokens,
            });
            break;
        }

        let piece = token_to_piece(vocab, new_token);
        generated_text.push_str(&piece);

        // Stop-word check
        let mut stopped = false;
        for sw in &request.stop_words {
            if generated_text.ends_with(sw.as_str()) {
                let _ = tx.blocking_send(GenerateEvent::Done {
                    finish_reason: FinishReason::StopWord(sw.clone()),
                    prompt_tokens,
                    completion_tokens,
                });
                stopped = true;
                break;
            }
        }
        if stopped {
            break;
        }

        // Send token to receiver
        if tx.blocking_send(GenerateEvent::Token(piece)).is_err() {
            debug!("Generation cancelled (receiver dropped)");
            break;
        }

        // Context-size guard
        if n_cur >= n_ctx {
            let _ = tx.blocking_send(GenerateEvent::Done {
                finish_reason: FinishReason::Length,
                prompt_tokens,
                completion_tokens,
            });
            break;
        }

        // Next decode step
        batch.clear();
        batch.add(new_token, n_cur, &[0], true);
        n_cur += 1;

        if let Err(e) = ctx.decode(&mut batch) {
            let _ = tx.blocking_send(GenerateEvent::Error(format!("decode: {e}")));
            break;
        }
    }
}
