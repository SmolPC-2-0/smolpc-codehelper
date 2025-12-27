/// Tokenizer wrapper for text <-> token conversion
///
/// Wraps Hugging Face tokenizers library to provide encoding/decoding
/// for Qwen2.5-Coder models.

use std::path::Path;
use tokenizers::Tokenizer;

/// Wrapper around Hugging Face tokenizer
pub struct TokenizerWrapper {
    tokenizer: Tokenizer,
    eos_token_id: u32,
}

impl TokenizerWrapper {
    /// Load tokenizer from tokenizer.json file
    ///
    /// # Arguments
    /// * `path` - Path to tokenizer.json file
    ///
    /// # Qwen2.5-Coder Special Tokens
    /// - BOS (Beginning of Sequence): 151643
    /// - EOS (End of Sequence): 151645
    /// - PAD (Padding): 151643 (same as BOS)
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let tokenizer = Tokenizer::from_file(path)
            .map_err(|e| format!("Failed to load tokenizer: {e}"))?;

        // Qwen2.5-Coder specific EOS token
        let eos_token_id = 151645;

        log::info!("Tokenizer loaded successfully");
        log::debug!("EOS token ID: {}", eos_token_id);

        Ok(Self {
            tokenizer,
            eos_token_id,
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

    /// Get EOS (end-of-sequence) token ID
    pub fn eos_token_id(&self) -> u32 {
        self.eos_token_id
    }

    /// Get vocabulary size
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

        let tokenizer = TokenizerWrapper::from_file(tokenizer_path)
            .expect("Failed to load tokenizer");

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
        let tokenizer = TokenizerWrapper::from_file(tokenizer_path)
            .expect("Failed to load tokenizer");

        println!("EOS token ID: {}", tokenizer.eos_token_id());

        // Test with special tokens
        let text = "print('hello')";
        let tokens_with_special = tokenizer.encode(text, true).unwrap();
        let tokens_without_special = tokenizer.encode(text, false).unwrap();

        println!("With special tokens: {:?}", tokens_with_special);
        println!("Without special tokens: {:?}", tokens_without_special);

        assert!(tokens_with_special.len() >= tokens_without_special.len());
    }
}
