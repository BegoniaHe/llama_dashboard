//! Sampler chain construction and token sampling.

use crate::context::LlamaContext;

/// RAII wrapper around a `llama_sampler` chain.
pub struct SamplerChain {
    ptr: *mut llama_sys::llama_sampler,
}

unsafe impl Send for SamplerChain {}

impl SamplerChain {
    /// Create an empty sampler chain.
    pub fn new(no_perf: bool) -> Self {
        let params = llama_sys::llama_sampler_chain_params { no_perf };
        let ptr = unsafe { llama_sys::llama_sampler_chain_init(params) };
        Self { ptr }
    }

    //  Sampler primitives

    pub fn add_greedy(&mut self) {
        unsafe {
            llama_sys::llama_sampler_chain_add(self.ptr, llama_sys::llama_sampler_init_greedy())
        }
    }

    pub fn add_dist(&mut self, seed: u32) {
        unsafe {
            llama_sys::llama_sampler_chain_add(self.ptr, llama_sys::llama_sampler_init_dist(seed))
        }
    }

    pub fn add_top_k(&mut self, k: i32) {
        unsafe {
            llama_sys::llama_sampler_chain_add(self.ptr, llama_sys::llama_sampler_init_top_k(k))
        }
    }

    pub fn add_top_p(&mut self, p: f32, min_keep: usize) {
        unsafe {
            llama_sys::llama_sampler_chain_add(
                self.ptr,
                llama_sys::llama_sampler_init_top_p(p, min_keep),
            )
        }
    }

    pub fn add_min_p(&mut self, p: f32, min_keep: usize) {
        unsafe {
            llama_sys::llama_sampler_chain_add(
                self.ptr,
                llama_sys::llama_sampler_init_min_p(p, min_keep),
            )
        }
    }

    pub fn add_temp(&mut self, t: f32) {
        unsafe {
            llama_sys::llama_sampler_chain_add(self.ptr, llama_sys::llama_sampler_init_temp(t))
        }
    }

    pub fn add_penalties(&mut self, last_n: i32, repeat: f32, freq: f32, presence: f32) {
        unsafe {
            llama_sys::llama_sampler_chain_add(
                self.ptr,
                llama_sys::llama_sampler_init_penalties(last_n, repeat, freq, presence),
            )
        }
    }

    //  Sampling

    /// Sample the next token from the model output at position `idx`.
    pub fn sample(&mut self, ctx: &LlamaContext, idx: i32) -> i32 {
        unsafe { llama_sys::llama_sampler_sample(self.ptr, ctx.as_ptr(), idx) }
    }
}

impl Drop for SamplerChain {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { llama_sys::llama_sampler_free(self.ptr) }
        }
    }
}

//  High-level SamplingParams

/// User-facing sampling configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SamplingParams {
    #[serde(default = "default_temp")]
    pub temperature: f32,
    #[serde(default = "default_top_k")]
    pub top_k: i32,
    #[serde(default = "default_top_p")]
    pub top_p: f32,
    #[serde(default = "default_min_p")]
    pub min_p: f32,
    #[serde(default = "default_repeat_penalty")]
    pub repeat_penalty: f32,
    #[serde(default)]
    pub frequency_penalty: f32,
    #[serde(default)]
    pub presence_penalty: f32,
    #[serde(default = "default_repeat_last_n")]
    pub repeat_last_n: i32,
    #[serde(default)]
    pub seed: Option<u32>,
}

fn default_temp() -> f32 {
    0.8
}
fn default_top_k() -> i32 {
    40
}
fn default_top_p() -> f32 {
    0.95
}
fn default_min_p() -> f32 {
    0.05
}
fn default_repeat_penalty() -> f32 {
    1.1
}
fn default_repeat_last_n() -> i32 {
    64
}

impl Default for SamplingParams {
    fn default() -> Self {
        Self {
            temperature: default_temp(),
            top_k: default_top_k(),
            top_p: default_top_p(),
            min_p: default_min_p(),
            repeat_penalty: default_repeat_penalty(),
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            repeat_last_n: default_repeat_last_n(),
            seed: None,
        }
    }
}

impl SamplingParams {
    /// Build and return a ready-to-use [`SamplerChain`].
    pub fn into_chain(self) -> SamplerChain {
        let mut chain = SamplerChain::new(false);

        if self.repeat_penalty != 1.0
            || self.frequency_penalty != 0.0
            || self.presence_penalty != 0.0
        {
            chain.add_penalties(
                self.repeat_last_n,
                self.repeat_penalty,
                self.frequency_penalty,
                self.presence_penalty,
            );
        }

        if self.top_k > 0 {
            chain.add_top_k(self.top_k);
        }
        if self.top_p < 1.0 {
            chain.add_top_p(self.top_p, 1);
        }
        if self.min_p > 0.0 {
            chain.add_min_p(self.min_p, 1);
        }

        if self.temperature > 0.0 {
            chain.add_temp(self.temperature);
            chain.add_dist(self.seed.unwrap_or(0));
        } else {
            chain.add_greedy();
        }

        chain
    }
}
