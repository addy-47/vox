use std::sync::OnceLock;

/// Singleton tiktoken BPE tokenizer instance (cl100k_base).
static BPE_TOKENIZER: OnceLock<Option<tiktoken_rs::CoreBPE>> = OnceLock::new();

/// Retrieves or initializes the static CoreBPE instance.
fn get_bpe() -> Option<&'static tiktoken_rs::CoreBPE> {
    BPE_TOKENIZER
        .get_or_init(|| tiktoken_rs::cl100k_base().ok())
        .as_ref()
}

/// Computes exact token count for text using BPE tokenization.
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    if let Some(bpe) = get_bpe() {
        bpe.encode_with_special_tokens(text).len()
    } else {
        let mut token_estimate = 0usize;
        for c in text.chars() {
            if c.is_ascii() {
                token_estimate += 1;
            } else {
                token_estimate += 3;
            }
        }
        (token_estimate as f64 / 3.0).ceil() as usize
    }
}
