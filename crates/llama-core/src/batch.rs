//! Safe wrapper around `llama_batch`.

/// RAII batch of tokens to feed into the decoder.
pub struct LlamaBatch {
    inner: llama_sys::llama_batch,
    capacity: i32,
    /// `true` when we own the internal allocations (and must free them).
    owned: bool,
}

impl LlamaBatch {
    /// Allocate a new batch with room for `n_tokens_max` tokens.
    ///
    /// `embd` — if > 0, allocate embedding storage instead of token storage.
    /// `n_seq_max` — max sequences per token position.
    pub fn new(n_tokens_max: i32, embd: i32, n_seq_max: i32) -> Self {
        let inner = unsafe { llama_sys::llama_batch_init(n_tokens_max, embd, n_seq_max) };
        Self {
            inner,
            capacity: n_tokens_max,
            owned: true,
        }
    }

    /// Return the raw batch struct (passed by value — `Copy` in C).
    pub fn raw(&self) -> llama_sys::llama_batch {
        self.inner
    }

    /// Number of tokens currently stored.
    pub fn n_tokens(&self) -> i32 {
        self.inner.n_tokens
    }

    /// Remove all tokens.
    pub fn clear(&mut self) {
        self.inner.n_tokens = 0;
    }

    /// Push a token into the batch.
    ///
    /// * `token`   — token id
    /// * `pos`     — absolute position
    /// * `seq_ids` — sequence ids this token belongs to
    /// * `logits`  — request logits output for this position
    pub fn add(&mut self, token: i32, pos: i32, seq_ids: &[i32], logits: bool) {
        let i = self.inner.n_tokens as usize;
        assert!(
            (i as i32) < self.capacity,
            "LlamaBatch capacity ({}) exceeded",
            self.capacity
        );

        unsafe {
            *self.inner.token.add(i) = token;
            *self.inner.pos.add(i) = pos;
            *self.inner.n_seq_id.add(i) = seq_ids.len() as i32;
            for (j, &sid) in seq_ids.iter().enumerate() {
                *(*self.inner.seq_id.add(i)).add(j) = sid;
            }
            *self.inner.logits.add(i) = i8::from(logits);
        }
        self.inner.n_tokens += 1;
    }
}

impl Drop for LlamaBatch {
    fn drop(&mut self) {
        if self.owned {
            unsafe { llama_sys::llama_batch_free(self.inner) }
        }
    }
}
