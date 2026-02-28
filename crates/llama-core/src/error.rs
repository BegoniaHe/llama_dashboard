use thiserror::Error;

#[derive(Error, Debug)]
pub enum LlamaError {
    #[error("Failed to load model from '{path}': {reason}")]
    ModelLoadFailed { path: String, reason: String },

    #[error("Failed to create context: {0}")]
    ContextCreationFailed(String),

    #[error("Decode failed with code {0}")]
    DecodeFailed(i32),

    #[error("Encode failed with code {0}")]
    EncodeFailed(i32),

    #[error("Tokenization failed: {0}")]
    TokenizationFailed(String),

    #[error("Sampler error: {0}")]
    SamplerError(String),

    #[error("Backend not initialized")]
    BackendNotInitialized,

    #[error("Model not loaded")]
    ModelNotLoaded,

    #[error("Null pointer from FFI call")]
    NullPointer,

    #[error("FFI panic: {0}")]
    FfiPanic(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, LlamaError>;
