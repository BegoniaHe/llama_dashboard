//! Global llama.cpp backend initialization and system queries.

use std::ffi::CStr;
use std::sync::Once;
use tracing::{debug, info};

static BACKEND_INIT: Once = Once::new();

/// RAII guard for the llama.cpp backend.
///
/// The first call to [`LlamaBackend::init`] initializes the C backend;
/// subsequent calls are no-ops. The backend is freed at process exit.
pub struct LlamaBackend {
    _private: (),
}

impl LlamaBackend {
    /// Initialize the llama.cpp backend (idempotent).
    pub fn init() -> Self {
        BACKEND_INIT.call_once(|| {
            unsafe {
                llama_sys::llama_backend_init();
            }
            info!("llama.cpp backend initialized");
        });
        Self { _private: () }
    }

    /// Initialize NUMA optimizations.
    pub fn numa_init(&self, strategy: NumaStrategy) {
        unsafe {
            llama_sys::llama_numa_init(strategy.as_raw());
        }
        debug!(?strategy, "NUMA initialized");
    }

    /// Set the global log callback, bridging llama.cpp logs to the Rust
    /// `tracing` subsystem. Call once after backend init.
    pub fn set_log_callback(&self) {
        unsafe extern "C" fn cb(
            level: llama_sys::ggml_log_level,
            text: *const std::ffi::c_char,
            _user_data: *mut std::ffi::c_void,
        ) {
            if text.is_null() {
                return;
            }
            let msg = unsafe { CStr::from_ptr(text) }.to_string_lossy();
            let msg = msg.trim();
            if msg.is_empty() {
                return;
            }
            // ggml_log_level: DEBUG=1, INFO=2, WARN=3, ERROR=4
            match level {
                4 => tracing::error!(target: "llama.cpp", "{msg}"),
                3 => tracing::warn!(target: "llama.cpp", "{msg}"),
                2 => tracing::info!(target: "llama.cpp", "{msg}"),
                _ => tracing::debug!(target: "llama.cpp", "{msg}"),
            }
        }

        unsafe {
            llama_sys::llama_log_set(Some(cb), std::ptr::null_mut());
        }
        debug!("llama.cpp log callback installed");
    }

    /// Return a human-readable system information string.
    pub fn system_info() -> String {
        unsafe {
            CStr::from_ptr(llama_sys::llama_print_system_info())
                .to_string_lossy()
                .into_owned()
        }
    }
}

// Backend is process-global; we never explicitly free it during normal
// execution — it is cleaned up at process exit.
impl Drop for LlamaBackend {
    fn drop(&mut self) {}
}

//  NUMA strategy

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumaStrategy {
    Disabled,
    Distribute,
    Isolate,
    NUMACtl,
    Mirror,
}

impl NumaStrategy {
    fn as_raw(self) -> llama_sys::ggml_numa_strategy {
        match self {
            Self::Disabled => 0,
            Self::Distribute => 1,
            Self::Isolate => 2,
            Self::NUMACtl => 3,
            Self::Mirror => 4,
        }
    }
}
