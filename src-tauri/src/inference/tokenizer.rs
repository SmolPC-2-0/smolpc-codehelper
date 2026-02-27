/// Tokenizer wrapper for text <-> token conversion
///
/// Wraps Hugging Face tokenizers library to provide encoding/decoding
/// for Qwen2.5-Coder models.
use std::path::Path;
use tokenizers::Tokenizer;

/// Wrapper around Hugging Face tokenizer
pub struct TokenizerWrapper {
    tokenizer: Tokenizer,
    stop_token_ids: Vec<u32>,
}

impl TokenizerWrapper {
    /// Load tokenizer from tokenizer.json file
    ///
    /// # Arguments
    /// * `path` - Path to tokenizer.json file
    ///
    /// # Qwen2.5-Coder Special Tokens
    /// - 151643: `<|endoftext|>` — raw completion EOS
    /// - 151644: `<|im_start|>` — ChatML turn start
    /// - 151645: `<|im_end|>` — ChatML turn end
    #[cfg(test)]
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        // Default stop tokens for Qwen2.5-Coder
        Self::from_file_with_stop_tokens(path, &[151643, 151645])
    }

    /// Load tokenizer from tokenizer.json with explicit stop token IDs
    pub fn from_file_with_stop_tokens<P: AsRef<Path>>(
        path: P,
        stop_token_ids: &[u32],
    ) -> Result<Self, String> {
        let tokenizer =
            Tokenizer::from_file(path).map_err(|e| format!("Failed to load tokenizer: {e}"))?;
        let stop_token_ids = stop_token_ids.to_vec();

        log::info!("Tokenizer loaded successfully");
        log::debug!("Stop token IDs: {:?}", stop_token_ids);

        Ok(Self {
            tokenizer,
            stop_token_ids,
        })
    }

    /// Encode text to token IDs
    ///
    /// # Arguments
    /// * `text` - Input text string
    /// * `add_special_tokens` - Whether to add BOS/EOS tokens
    ///
    /// # Returns
    /// Vector of token IDs (u32)
    pub fn encode(&self, text: &str, add_special_tokens: bool) -> Result<Vec<u32>, String> {
        let encoding = self
            .tokenizer
            .encode(text, add_special_tokens)
            .map_err(|e| format!("Tokenization failed: {e}"))?;

        Ok(encoding.get_ids().to_vec())
    }

    /// Decode token IDs to text
    ///
    /// # Arguments
    /// * `token_ids` - Vector of token IDs
    /// * `skip_special_tokens` - Whether to skip special tokens (BOS/EOS/PAD) in output
    ///
    /// # Returns
    /// Decoded text string
    pub fn decode(&self, token_ids: &[u32], skip_special_tokens: bool) -> Result<String, String> {
        self.tokenizer
            .decode(token_ids, skip_special_tokens)
            .map_err(|e| format!("Detokenization failed: {e}"))
    }

    /// Check if a token ID is a stop token (EOS or im_end)
    pub fn is_stop_token(&self, token_id: u32) -> bool {
        self.stop_token_ids.contains(&token_id)
    }

    /// Get vocabulary size
    #[cfg(test)]
    pub fn vocab_size(&self) -> usize {
        self.tokenizer.get_vocab_size(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires tokenizer file - run manually
    fn test_tokenizer_encode_decode() {
        // This test requires a tokenizer file at the specified path
        // Run with: cargo test test_tokenizer_encode_decode -- --ignored --nocapture

        let tokenizer_path = "models/qwen2.5-coder-1.5b/tokenizer.json";

        let tokenizer =
            TokenizerWrapper::from_file(tokenizer_path).expect("Failed to load tokenizer");

        // Test encoding
        let text = "def hello_world():";
        let tokens = tokenizer.encode(text, false).expect("Failed to encode");

        println!("Text: {}", text);
        println!("Tokens: {:?}", tokens);
        println!("Vocab size: {}", tokenizer.vocab_size());

        // Test decoding
        let decoded = tokenizer.decode(&tokens, false).expect("Failed to decode");
        println!("Decoded: {}", decoded);

        assert_eq!(text, decoded);
    }

    #[test]
    #[ignore]
    fn test_special_tokens() {
        let tokenizer_path = "models/qwen2.5-coder-1.5b/tokenizer.json";
        let tokenizer =
            TokenizerWrapper::from_file(tokenizer_path).expect("Failed to load tokenizer");

        println!(
            "Is 151643 a stop token: {}",
            tokenizer.is_stop_token(151643)
        );
        println!(
            "Is 151645 a stop token: {}",
            tokenizer.is_stop_token(151645)
        );
        assert!(tokenizer.is_stop_token(151643)); // <|endoftext|>
        assert!(tokenizer.is_stop_token(151645)); // <|im_end|>
        assert!(!tokenizer.is_stop_token(0)); // regular token

        // Test with special tokens
        let text = "print('hello')";
        let tokens_with_special = tokenizer.encode(text, true).unwrap();
        let tokens_without_special = tokenizer.encode(text, false).unwrap();

        println!("With special tokens: {:?}", tokens_with_special);
        println!("Without special tokens: {:?}", tokens_without_special);

        assert!(tokens_with_special.len() >= tokens_without_special.len());
    }
}
