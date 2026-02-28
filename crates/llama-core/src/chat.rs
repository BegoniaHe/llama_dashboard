//! Chat-template formatting.

use std::ffi::CString;

use crate::model::LlamaModel;

/// A single chat message (role + content).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Apply a Jinja-style chat template to a list of messages.
///
/// * `template` — explicit template string; pass `None` to use the
///   built-in ChatML default.
/// * `add_assistant` — append an empty assistant turn (for generation).
///
/// Returns the formatted prompt, or `None` if the template cannot be
/// applied.
pub fn apply_template(
    template: Option<&str>,
    messages: &[ChatMessage],
    add_assistant: bool,
) -> Option<String> {
    let c_tmpl = template.and_then(|t| CString::new(t).ok());
    let tmpl_ptr = c_tmpl.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());

    // Build C-compatible message array.  The CStrings must live until the
    // call completes.
    let c_roles: Vec<CString> = messages
        .iter()
        .map(|m| CString::new(m.role.as_str()).unwrap_or_default())
        .collect();
    let c_contents: Vec<CString> = messages
        .iter()
        .map(|m| CString::new(m.content.as_str()).unwrap_or_default())
        .collect();
    let c_msgs: Vec<llama_sys::llama_chat_message> = c_roles
        .iter()
        .zip(c_contents.iter())
        .map(|(r, c)| llama_sys::llama_chat_message {
            role: r.as_ptr(),
            content: c.as_ptr(),
        })
        .collect();

    // First call: measure required buffer length.
    let needed = unsafe {
        llama_sys::llama_chat_apply_template(
            tmpl_ptr,
            c_msgs.as_ptr(),
            c_msgs.len(),
            add_assistant,
            std::ptr::null_mut(),
            0,
        )
    };
    if needed <= 0 {
        return None;
    }

    let mut buf = vec![0u8; (needed + 1) as usize];
    let wrote = unsafe {
        llama_sys::llama_chat_apply_template(
            tmpl_ptr,
            c_msgs.as_ptr(),
            c_msgs.len(),
            add_assistant,
            buf.as_mut_ptr() as *mut std::ffi::c_char,
            buf.len() as i32,
        )
    };
    if wrote > 0 {
        buf.truncate(wrote as usize);
        Some(String::from_utf8_lossy(&buf).into_owned())
    } else {
        None
    }
}

/// Convenience: apply the model's own chat template.
pub fn apply_model_template(
    model: &LlamaModel,
    messages: &[ChatMessage],
    add_assistant: bool,
) -> Option<String> {
    let tmpl = model.chat_template();
    apply_template(tmpl.as_deref(), messages, add_assistant)
}
