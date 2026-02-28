//! Safe RAII wrapper around `llama_model`.

use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr;

use tracing::{debug, info};

use crate::error::{LlamaError, Result};

/// Owns a `llama_model` pointer and frees it on drop.
pub struct LlamaModel {
    ptr: *mut llama_sys::llama_model,
}

// Safety: llama_model is internally read-only after creation.
unsafe impl Send for LlamaModel {}
unsafe impl Sync for LlamaModel {}

impl LlamaModel {
    /// Load a GGUF model from `path`.
    pub fn load_from_file(path: &Path, params: &ModelParams) -> Result<Self> {
        let path_str = path.to_str().ok_or_else(|| LlamaError::ModelLoadFailed {
            path: path.display().to_string(),
            reason: "Invalid UTF-8 in path".into(),
        })?;
        let c_path = CString::new(path_str).map_err(|_| LlamaError::ModelLoadFailed {
            path: path_str.into(),
            reason: "Path contains null byte".into(),
        })?;

        let mut raw = unsafe { llama_sys::llama_model_default_params() };
        raw.n_gpu_layers = params.n_gpu_layers;
        raw.use_mmap = params.use_mmap;
        raw.use_mlock = params.use_mlock;

        info!(path = %path.display(), "Loading model…");
        let model = unsafe { llama_sys::llama_model_load_from_file(c_path.as_ptr(), raw) };

        if model.is_null() {
            return Err(LlamaError::ModelLoadFailed {
                path: path_str.into(),
                reason: "llama_model_load_from_file returned null".into(),
            });
        }

        info!(path = %path.display(), "Model loaded");
        Ok(Self { ptr: model })
    }

    //  Accessors

    pub(crate) fn as_ptr(&self) -> *mut llama_sys::llama_model {
        self.ptr
    }

    /// Vocabulary handle (valid for the lifetime of the model).
    pub fn vocab(&self) -> *const llama_sys::llama_vocab {
        unsafe { llama_sys::llama_model_get_vocab(self.ptr) }
    }

    pub fn n_params(&self) -> u64 {
        unsafe { llama_sys::llama_model_n_params(self.ptr) }
    }

    pub fn size(&self) -> u64 {
        unsafe { llama_sys::llama_model_size(self.ptr) }
    }

    pub fn desc(&self) -> String {
        let mut buf = vec![0u8; 256];
        let len = unsafe {
            llama_sys::llama_model_desc(
                self.ptr,
                buf.as_mut_ptr() as *mut std::ffi::c_char,
                buf.len(),
            )
        };
        if len > 0 {
            buf.truncate(len as usize);
            String::from_utf8_lossy(&buf).into_owned()
        } else {
            String::new()
        }
    }

    pub fn n_ctx_train(&self) -> i32 {
        unsafe { llama_sys::llama_model_n_ctx_train(self.ptr) }
    }

    pub fn n_embd(&self) -> i32 {
        unsafe { llama_sys::llama_model_n_embd(self.ptr) }
    }

    /// Built-in chat template, if any.
    pub fn chat_template(&self) -> Option<String> {
        unsafe {
            let p = llama_sys::llama_model_chat_template(self.ptr, ptr::null());
            if p.is_null() {
                None
            } else {
                Some(CStr::from_ptr(p).to_string_lossy().into_owned())
            }
        }
    }

    /// Read an arbitrary metadata string by key.
    pub fn meta_val_str(&self, key: &str) -> Option<String> {
        let c_key = CString::new(key).ok()?;
        let mut buf = vec![0u8; 512];
        let len = unsafe {
            llama_sys::llama_model_meta_val_str(
                self.ptr,
                c_key.as_ptr(),
                buf.as_mut_ptr() as *mut std::ffi::c_char,
                buf.len(),
            )
        };
        if len > 0 {
            buf.truncate(len as usize);
            Some(String::from_utf8_lossy(&buf).into_owned())
        } else {
            None
        }
    }

    pub fn meta_count(&self) -> i32 {
        unsafe { llama_sys::llama_model_meta_count(self.ptr) }
    }

    pub fn has_encoder(&self) -> bool {
        unsafe { llama_sys::llama_model_has_encoder(self.ptr) }
    }

    pub fn has_decoder(&self) -> bool {
        unsafe { llama_sys::llama_model_has_decoder(self.ptr) }
    }

    //  Vocabulary helpers

    pub fn n_vocab(&self) -> i32 {
        unsafe { llama_sys::llama_vocab_n_tokens(self.vocab()) }
    }
    pub fn token_bos(&self) -> i32 {
        unsafe { llama_sys::llama_vocab_bos(self.vocab()) }
    }
    pub fn token_eos(&self) -> i32 {
        unsafe { llama_sys::llama_vocab_eos(self.vocab()) }
    }
    pub fn token_eot(&self) -> i32 {
        unsafe { llama_sys::llama_vocab_eot(self.vocab()) }
    }
}

impl Drop for LlamaModel {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            debug!("Freeing llama model");
            unsafe { llama_sys::llama_model_free(self.ptr) }
        }
    }
}

//  ModelParams

/// Parameters for [`LlamaModel::load_from_file`].
#[derive(Debug, Clone)]
pub struct ModelParams {
    /// Layers to offload to GPU. -1 = all.
    pub n_gpu_layers: i32,
    /// Use memory-mapped I/O.
    pub use_mmap: bool,
    /// Lock model memory (prevent swapping).
    pub use_mlock: bool,
}

impl Default for ModelParams {
    fn default() -> Self {
        Self {
            n_gpu_layers: -1,
            use_mmap: true,
            use_mlock: false,
        }
    }
}
