//! Safe Rust wrapper around the llama.cpp C API.
//!
//! Provides RAII-managed types for model loading, context creation,
//! sampling, tokenization, and streaming text generation.

pub mod backend;
pub mod batch;
pub mod chat;
pub mod context;
pub mod error;
pub mod generate;
pub mod model;
pub mod sampler;
pub mod token;

pub use backend::LlamaBackend;
pub use batch::LlamaBatch;
pub use chat::{ChatMessage, apply_template};
pub use context::{ContextParams, LlamaContext, PerfData};
pub use error::{LlamaError, Result};
pub use generate::{FinishReason, GenerateEvent, GenerateRequest};
pub use model::{LlamaModel, ModelParams};
pub use sampler::{SamplerChain, SamplingParams};
pub use token::{detokenize, token_to_piece, tokenize};
