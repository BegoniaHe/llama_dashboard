//! Tokenization / detokenization helpers.

use std::ffi::CString;

use crate::error::{LlamaError, Result};

/// Tokenize `text` using the model's vocabulary.
pub fn tokenize(
    vocab: *const llama_sys::llama_vocab,
    text: &str,
    add_special: bool,
    parse_special: bool,
) -> Result<Vec<i32>> {
    let c_text = CString::new(text)
        .map_err(|_| LlamaError::TokenizationFailed("text contains null byte".into()))?;

    // First call: query required buffer size (returns negative count).
    let n = unsafe {
        llama_sys::llama_tokenize(
            vocab,
            c_text.as_ptr(),
            text.len() as i32,
            std::ptr::null_mut(),
            0,
            add_special,
            parse_special,
        )
    };

    let capacity = (-n) as usize;
    let mut tokens = vec![0i32; capacity];

    let actual = unsafe {
        llama_sys::llama_tokenize(
            vocab,
            c_text.as_ptr(),
            text.len() as i32,
            tokens.as_mut_ptr(),
            tokens.len() as i32,
            add_special,
            parse_special,
        )
    };

    if actual < 0 {
        return Err(LlamaError::TokenizationFailed(format!(
            "llama_tokenize returned {actual}"
        )));
    }

    tokens.truncate(actual as usize);
    Ok(tokens)
}

/// Convert a single token id to its text piece.
pub fn token_to_piece(vocab: *const llama_sys::llama_vocab, token: i32) -> String {
    let mut buf = vec![0u8; 128];
    let len = unsafe {
        llama_sys::llama_token_to_piece(
            vocab,
            token,
            buf.as_mut_ptr() as *mut std::ffi::c_char,
            buf.len() as i32,
            0,     // lstrip
            false, // special
        )
    };

    if len < 0 {
        // Buffer too small — retry.
        buf.resize((-len) as usize, 0);
        let len = unsafe {
            llama_sys::llama_token_to_piece(
                vocab,
                token,
                buf.as_mut_ptr() as *mut std::ffi::c_char,
                buf.len() as i32,
                0,
                false,
            )
        };
        if len > 0 {
            buf.truncate(len as usize);
        } else {
            return String::new();
        }
    } else {
        buf.truncate(len as usize);
    }

    String::from_utf8_lossy(&buf).into_owned()
}

/// Detokenize a token sequence back to text.
pub fn detokenize(vocab: *const llama_sys::llama_vocab, tokens: &[i32]) -> Result<String> {
    let mut buf = vec![0u8; tokens.len() * 16];
    let len = unsafe {
        llama_sys::llama_detokenize(
            vocab,
            tokens.as_ptr(),
            tokens.len() as i32,
            buf.as_mut_ptr() as *mut std::ffi::c_char,
            buf.len() as i32,
            false,
            false,
        )
    };

    if len < 0 {
        buf.resize((-len) as usize, 0);
        let len2 = unsafe {
            llama_sys::llama_detokenize(
                vocab,
                tokens.as_ptr(),
                tokens.len() as i32,
                buf.as_mut_ptr() as *mut std::ffi::c_char,
                buf.len() as i32,
                false,
                false,
            )
        };
        if len2 > 0 {
            buf.truncate(len2 as usize);
        } else {
            return Err(LlamaError::TokenizationFailed(
                "detokenize failed on retry".into(),
            ));
        }
    } else {
        buf.truncate(len as usize);
    }

    Ok(String::from_utf8_lossy(&buf).into_owned())
}
