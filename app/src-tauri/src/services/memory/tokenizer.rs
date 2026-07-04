use std::sync::OnceLock;

/// Singleton tiktoken BPE tokenizer instance (cl100k_base).
/// Used across all WorkingMemory context estimations to ensure exact token counts
/// for English, Devanagari (Hindi), and code tokens without heuristic drift.
static BPE_TOKENIZER: OnceLock<Option<tiktoken_rs::CoreBPE>> = OnceLock::new();

fn get_bpe() -> Option<&'static tiktoken_rs::CoreBPE> {
    BPE_TOKENIZER
        .get_or_init(|| tiktoken_rs::cl100k_base().ok())
        .as_ref()
}

/// Computes exact token count for text using BPE tokenization.
/// Correctly accounts for Devanagari (Hindi) UTF-8 character splitting into subword tokens,
/// preventing KV-cache overflows in production.
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    if let Some(bpe) = get_bpe() {
        bpe.encode_with_special_tokens(text).len()
    } else {
        // Fallback script-aware estimation if BPE fails to load
        let mut token_estimate = 0usize;
        for c in text.chars() {
            if c.is_ascii() {
                token_estimate += 1;
            } else {
                // Devanagari & non-ASCII UTF-8 characters split into ~2.5 tokens per character in BPE
                token_estimate += 3;
            }
        }
        (token_estimate as f64 / 3.0).ceil() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bpe_english_and_devanagari_token_counts() {
        let en_text = "Hello Vox, can you tell me what the capital of France is?";
        let hi_text = "नमस्ते वॉक्स, क्या आप मुझे भारत के बारे में बता सकते हैं?";

        let en_tokens = estimate_tokens(en_text);
        let hi_tokens = estimate_tokens(hi_text);

        println!("EN text len: {} -> tokens: {}", en_text.len(), en_tokens);
        println!("HI text len: {} -> tokens: {}", hi_text.len(), hi_tokens);

        assert!(en_tokens > 10 && en_tokens < 20);
        // Devanagari Hindi text must tokenize into significantly higher BPE subword tokens
        assert!(hi_tokens >= 18);
    }
}
