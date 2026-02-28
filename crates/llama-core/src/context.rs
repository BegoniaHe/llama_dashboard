//! Safe RAII wrapper around `llama_context`.

use std::sync::Arc;

use tracing::debug;

use crate::batch::LlamaBatch;
use crate::error::{LlamaError, Result};
use crate::model::LlamaModel;

/// Owns a `llama_context` pointer and its parent model reference.
pub struct LlamaContext {
    ptr: *mut llama_sys::llama_context,
    /// Keep the model alive for the lifetime of the context.
    model: Arc<LlamaModel>,
}

// Safety: all use of the context is &mut self (single-threaded access
// enforced by Mutex at the service layer).
unsafe impl Send for LlamaContext {}

impl LlamaContext {
    /// Create a new inference context.
    pub fn new(model: Arc<LlamaModel>, params: &ContextParams) -> Result<Self> {
        let mut raw = unsafe { llama_sys::llama_context_default_params() };
        raw.n_ctx = params.n_ctx;
        raw.n_batch = params.n_batch;
        raw.n_ubatch = params.n_ubatch;
        raw.n_threads = params.n_threads;
        raw.n_threads_batch = params.n_threads_batch;
        raw.embeddings = params.embeddings;

        let ctx = unsafe { llama_sys::llama_init_from_model(model.as_ptr(), raw) };
        if ctx.is_null() {
            return Err(LlamaError::ContextCreationFailed(
                "llama_init_from_model returned null".into(),
            ));
        }

        debug!(n_ctx = params.n_ctx, "Context created");
        Ok(Self { ptr: ctx, model })
    }

    //  Accessors

    pub(crate) fn as_ptr(&self) -> *mut llama_sys::llama_context {
        self.ptr
    }

    pub fn model(&self) -> &LlamaModel {
        &self.model
    }

    pub fn n_ctx(&self) -> u32 {
        unsafe { llama_sys::llama_n_ctx(self.ptr) }
    }

    pub fn n_batch(&self) -> u32 {
        unsafe { llama_sys::llama_n_batch(self.ptr) }
    }

    //  Core operations

    /// Decode (process) a batch of tokens.
    pub fn decode(&mut self, batch: &mut LlamaBatch) -> Result<()> {
        let rc = unsafe { llama_sys::llama_decode(self.ptr, batch.raw()) };
        if rc != 0 {
            return Err(LlamaError::DecodeFailed(rc));
        }
        Ok(())
    }

    /// Logits for the token at index `i` in the last batch.
    pub fn get_logits_ith(&self, i: i32) -> Option<&[f32]> {
        unsafe {
            let p = llama_sys::llama_get_logits_ith(self.ptr, i);
            if p.is_null() {
                None
            } else {
                Some(std::slice::from_raw_parts(p, self.model.n_vocab() as usize))
            }
        }
    }

    /// Pooled embeddings (only valid when `embeddings = true`).
    pub fn get_embeddings(&self) -> Option<&[f32]> {
        unsafe {
            let p = llama_sys::llama_get_embeddings(self.ptr);
            if p.is_null() {
                None
            } else {
                Some(std::slice::from_raw_parts(p, self.model.n_embd() as usize))
            }
        }
    }

    //  KV cache

    pub fn kv_cache_clear(&mut self) {
        unsafe {
            let mem = llama_sys::llama_get_memory(self.ptr);
            if !mem.is_null() {
                llama_sys::llama_memory_clear(mem, false);
            }
        }
    }

    pub fn kv_cache_seq_rm(&mut self, seq_id: i32, p0: i32, p1: i32) -> bool {
        unsafe {
            let mem = llama_sys::llama_get_memory(self.ptr);
            if mem.is_null() {
                return false;
            }
            llama_sys::llama_memory_seq_rm(mem, seq_id, p0, p1)
        }
    }

    //  Performance

    pub fn perf(&self) -> PerfData {
        let d = unsafe { llama_sys::llama_perf_context(self.ptr) };
        PerfData {
            t_start_ms: d.t_start_ms,
            t_load_ms: d.t_load_ms,
            t_p_eval_ms: d.t_p_eval_ms,
            t_eval_ms: d.t_eval_ms,
            n_p_eval: d.n_p_eval,
            n_eval: d.n_eval,
        }
    }

    pub fn perf_reset(&mut self) {
        unsafe { llama_sys::llama_perf_context_reset(self.ptr) }
    }
}

impl Drop for LlamaContext {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            debug!("Freeing llama context");
            unsafe { llama_sys::llama_free(self.ptr) }
        }
    }
}

//  ContextParams

#[derive(Debug, Clone)]
pub struct ContextParams {
    pub n_ctx: u32,
    pub n_batch: u32,
    pub n_ubatch: u32,
    pub n_threads: i32,
    pub n_threads_batch: i32,
    pub embeddings: bool,
}

impl Default for ContextParams {
    fn default() -> Self {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4);
        Self {
            n_ctx: 0, // 0 → use model's training context size
            n_batch: 2048,
            n_ubatch: 512,
            n_threads: threads,
            n_threads_batch: threads,
            embeddings: false,
        }
    }
}

//  PerfData

#[derive(Debug, Clone)]
pub struct PerfData {
    pub t_start_ms: f64,
    pub t_load_ms: f64,
    pub t_p_eval_ms: f64,
    pub t_eval_ms: f64,
    pub n_p_eval: i32,
    pub n_eval: i32,
}

impl PerfData {
    /// Prompt processing speed (tokens/s).
    pub fn prompt_tokens_per_sec(&self) -> f64 {
        if self.t_p_eval_ms > 0.0 {
            self.n_p_eval as f64 / (self.t_p_eval_ms / 1000.0)
        } else {
            0.0
        }
    }

    /// Generation speed (tokens/s).
    pub fn generation_tokens_per_sec(&self) -> f64 {
        if self.t_eval_ms > 0.0 {
            self.n_eval as f64 / (self.t_eval_ms / 1000.0)
        } else {
            0.0
        }
    }
}
