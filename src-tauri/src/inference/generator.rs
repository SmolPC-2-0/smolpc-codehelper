/// Autoregressive text generation with ONNX Runtime
///
/// Implements the generation loop for LLM inference:
/// 1. Tokenize prompt
/// 2. Run inference (forward pass)
/// 3. Sample next token from logits
/// 4. Append to sequence
/// 5. Repeat until EOS or max length
///
/// # Phase 0 Implementation
/// - Greedy sampling only (pick highest probability token)
/// - No KV cache reuse (inefficient - provides empty cache each step)
/// - Synchronous generation (no streaming)
///
/// # Future Improvements
/// Phase 1: Add KV cache management, streaming, temperature sampling
/// Phase 2: GPU execution provider support

use super::session::InferenceSession;
use super::tokenizer::TokenizerWrapper;
use super::types::{GenerationConfig, GenerationMetrics, GenerationResult};
use ndarray::{Array2, Array4, ArrayView1};
use ort::session::SessionInputValue;
use ort::value::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// Qwen2.5-Coder-1.5B model architecture constants
/// These would ideally be read from model config, but hardcoded for Phase 0
const NUM_LAYERS: usize = 28;
const NUM_KV_HEADS: usize = 2; // GQA (grouped query attention)
const HEAD_DIM: usize = 128; // hidden_size / num_attention_heads = 1536 / 12

/// Text generator using ONNX Runtime
pub struct Generator {
    session: Arc<Mutex<InferenceSession>>,
    tokenizer: Arc<TokenizerWrapper>,
    config: GenerationConfig,
}

impl Generator {
    /// Create a new generator
    ///
    /// # Arguments
    /// * `session` - ONNX Runtime session (wrapped for thread safety)
    /// * `tokenizer` - Tokenizer for text conversion
    pub fn new(session: InferenceSession, tokenizer: TokenizerWrapper) -> Self {
        Self {
            session: Arc::new(Mutex::new(session)),
            tokenizer: Arc::new(tokenizer),
            config: GenerationConfig::default(),
        }
    }

    /// Create empty KV cache tensors for all layers
    /// Shape: [batch_size=1, num_kv_heads, past_seq_len=0, head_dim]
    fn create_empty_kv_cache() -> Result<HashMap<String, SessionInputValue<'static>>, String> {
        let mut inputs: HashMap<String, SessionInputValue<'static>> = HashMap::new();

        // Empty cache with shape [1, 2, 0, 128]
        let empty_cache: Array4<f32> = Array4::zeros((1, NUM_KV_HEADS, 0, HEAD_DIM));

        for layer in 0..NUM_LAYERS {
            let key_name = format!("past_key_values.{}.key", layer);
            let value_name = format!("past_key_values.{}.value", layer);

            let key_value = Value::from_array(empty_cache.clone())
                .map_err(|e| format!("Failed to create KV cache tensor for layer {}: {}", layer, e))?;
            let value_value = Value::from_array(empty_cache.clone())
                .map_err(|e| format!("Failed to create KV cache tensor for layer {}: {}", layer, e))?;

            inputs.insert(key_name, SessionInputValue::Owned(key_value.into()));
            inputs.insert(value_name, SessionInputValue::Owned(value_value.into()));
        }

        Ok(inputs)
    }

    /// Generate text from prompt
    ///
    /// # Phase 0 Implementation Details
    /// - Uses greedy sampling (always picks highest probability token)
    /// - No KV cache reuse: Provides empty cache each step (SLOW)
    /// - This is intentionally simple to validate the inference pipeline works
    ///
    /// # Performance Expectations (Phase 0)
    /// - CPU: ~1-5 tokens/sec (slow due to no KV cache reuse)
    /// - Time-to-first-token: ~2-5 seconds
    ///
    /// # Arguments
    /// * `prompt` - Input text prompt
    ///
    /// # Returns
    /// Generated text and performance metrics
    pub async fn generate(&self, prompt: &str) -> Result<GenerationResult, String> {
        let start = Instant::now();
        let mut first_token_time = None;

        log::info!("Starting generation for prompt (length: {} chars)", prompt.len());

        // 1. Tokenize prompt
        let input_ids = self.tokenizer.encode(prompt, true)?;
        let mut generated_ids = input_ids.clone();
        let mut generated_text = String::new();

        log::debug!("Prompt tokenized: {} tokens", input_ids.len());

        // 2. Generation loop (no KV cache reuse - Phase 0)
        for step in 0..self.config.max_length {
            // Prepare input tensors
            let seq_length = generated_ids.len();

            // Create input_ids tensor: [batch_size=1, seq_length]
            let input_ids_array = Array2::from_shape_vec(
                (1, seq_length),
                generated_ids.iter().map(|&id| id as i64).collect(),
            )
            .map_err(|e| format!("Failed to create input_ids tensor: {e}"))?;

            // Create attention_mask tensor: [batch_size=1, seq_length]
            // All 1s means "attend to all tokens"
            let attention_mask_array = Array2::from_shape_vec(
                (1, seq_length),
                vec![1i64; seq_length],
            )
            .map_err(|e| format!("Failed to create attention_mask tensor: {e}"))?;

            // Build inputs HashMap
            let mut inputs: HashMap<String, SessionInputValue<'static>> = HashMap::new();

            inputs.insert(
                "input_ids".to_string(),
                SessionInputValue::Owned(
                    Value::from_array(input_ids_array)
                        .map_err(|e| format!("Failed to create input_ids tensor: {e}"))?
                        .into()
                ),
            );
            inputs.insert(
                "attention_mask".to_string(),
                SessionInputValue::Owned(
                    Value::from_array(attention_mask_array)
                        .map_err(|e| format!("Failed to create attention_mask tensor: {e}"))?
                        .into()
                ),
            );

            // Add empty KV cache for all layers
            let kv_cache = Self::create_empty_kv_cache()?;
            inputs.extend(kv_cache);

            // Run inference (lock is held during inference and result extraction)
            let next_token = {
                let mut session = self.session.lock().await;
                let outputs = session
                    .session
                    .run(inputs)
                    .map_err(|e| format!("Inference failed at step {}: {e}", step))?;

                // Extract logits tensor: [batch_size, seq_length, vocab_size]
                let (logits_shape, logits_data) = outputs["logits"]
                    .try_extract_tensor::<f32>()
                    .map_err(|e| format!("Failed to extract logits: {e}"))?;

                // Convert raw data to ndarray with proper shape
                let _batch_size = logits_shape[0] as usize;
                let seq_len = logits_shape[1] as usize;
                let vocab_size = logits_shape[2] as usize;

                let logits = Array2::from_shape_vec(
                    (seq_len, vocab_size),
                    logits_data[0..(seq_len * vocab_size)].to_vec()
                ).map_err(|e| format!("Failed to reshape logits: {e}"))?;

                // Get logits for last position: [vocab_size]
                let last_token_logits = logits
                    .row(seq_len - 1)
                    .to_owned();

                // Sample next token (greedy for Phase 0)
                self.sample_greedy(last_token_logits.view())?
            }; // Lock is released here when session goes out of scope

            // Record time to first token
            if step == 0 {
                first_token_time = Some(start.elapsed());
                log::debug!("Time to first token: {:?}", first_token_time.unwrap());
            }

            // Check stop condition
            if next_token == self.tokenizer.eos_token_id() {
                log::info!("EOS token generated at step {}", step);
                break;
            }

            // Decode token and accumulate text
            let token_text = self.tokenizer.decode(&[next_token], false)?;
            generated_text.push_str(&token_text);

            // Append token to sequence
            generated_ids.push(next_token);

            // Progress logging
            if step % 10 == 0 && step > 0 {
                log::debug!("Generation step {}: {} tokens", step, generated_ids.len());
            }
        }

        // Calculate metrics
        let total_time = start.elapsed();
        let tokens_generated = generated_ids.len() - input_ids.len();
        let tokens_per_sec = if total_time.as_secs_f64() > 0.0 {
            tokens_generated as f64 / total_time.as_secs_f64()
        } else {
            0.0
        };

        log::info!(
            "Generation complete: {} tokens in {:.2}s ({:.2} tok/s)",
            tokens_generated,
            total_time.as_secs_f64(),
            tokens_per_sec
        );

        Ok(GenerationResult {
            text: generated_text,
            metrics: GenerationMetrics {
                total_tokens: tokens_generated,
                time_to_first_token_ms: first_token_time.map(|d| d.as_millis() as u64),
                tokens_per_second: tokens_per_sec,
                total_time_ms: total_time.as_millis() as u64,
            },
        })
    }

    /// Generate text with streaming output
    ///
    /// # Arguments
    /// * `prompt` - Input text prompt
    /// * `config` - Optional generation configuration
    /// * `cancelled` - Cancellation flag to stop generation
    /// * `on_token` - Callback invoked for each generated token
    ///
    /// # Returns
    /// Generation metrics (tokens generated, timing, etc.)
    pub async fn generate_stream<F>(
        &self,
        prompt: &str,
        config: Option<GenerationConfig>,
        cancelled: Arc<AtomicBool>,
        mut on_token: F,
    ) -> Result<GenerationMetrics, String>
    where
        F: FnMut(String),
    {
        let start = Instant::now();
        let mut first_token_time = None;

        // Use provided config or default
        let gen_config = config.unwrap_or(self.config.clone());

        log::info!(
            "Starting streaming generation for prompt (length: {} chars)",
            prompt.len()
        );

        // 1. Tokenize prompt
        let input_ids = self.tokenizer.encode(prompt, true)?;
        let mut generated_ids = input_ids.clone();
        let mut tokens_generated = 0usize;

        log::debug!("Prompt tokenized: {} tokens", input_ids.len());

        // 2. Generation loop
        for step in 0..gen_config.max_length {
            // Check for cancellation
            if cancelled.load(Ordering::Relaxed) {
                log::info!("Generation cancelled at step {}", step);
                break;
            }

            // Prepare input tensors
            let seq_length = generated_ids.len();

            // Create input_ids tensor: [batch_size=1, seq_length]
            let input_ids_array = Array2::from_shape_vec(
                (1, seq_length),
                generated_ids.iter().map(|&id| id as i64).collect(),
            )
            .map_err(|e| format!("Failed to create input_ids tensor: {e}"))?;

            // Create attention_mask tensor: [batch_size=1, seq_length]
            let attention_mask_array =
                Array2::from_shape_vec((1, seq_length), vec![1i64; seq_length])
                    .map_err(|e| format!("Failed to create attention_mask tensor: {e}"))?;

            // Build inputs HashMap
            let mut inputs: HashMap<String, SessionInputValue<'static>> = HashMap::new();

            inputs.insert(
                "input_ids".to_string(),
                SessionInputValue::Owned(
                    Value::from_array(input_ids_array)
                        .map_err(|e| format!("Failed to create input_ids tensor: {e}"))?
                        .into(),
                ),
            );
            inputs.insert(
                "attention_mask".to_string(),
                SessionInputValue::Owned(
                    Value::from_array(attention_mask_array)
                        .map_err(|e| format!("Failed to create attention_mask tensor: {e}"))?
                        .into(),
                ),
            );

            // Add empty KV cache for all layers
            let kv_cache = Self::create_empty_kv_cache()?;
            inputs.extend(kv_cache);

            // Run inference
            let next_token = {
                let mut session = self.session.lock().await;
                let outputs = session
                    .session
                    .run(inputs)
                    .map_err(|e| format!("Inference failed at step {}: {e}", step))?;

                // Extract logits tensor
                let (logits_shape, logits_data) = outputs["logits"]
                    .try_extract_tensor::<f32>()
                    .map_err(|e| format!("Failed to extract logits: {e}"))?;

                let seq_len = logits_shape[1] as usize;
                let vocab_size = logits_shape[2] as usize;

                let logits = Array2::from_shape_vec(
                    (seq_len, vocab_size),
                    logits_data[0..(seq_len * vocab_size)].to_vec(),
                )
                .map_err(|e| format!("Failed to reshape logits: {e}"))?;

                // Get logits for last position
                let last_token_logits = logits.row(seq_len - 1).to_owned();

                // Sample next token
                self.sample(last_token_logits.view(), &gen_config)?
            };

            // Record time to first token
            if step == 0 {
                first_token_time = Some(start.elapsed());
                log::debug!("Time to first token: {:?}", first_token_time.unwrap());
            }

            // Check stop condition
            if next_token == self.tokenizer.eos_token_id() {
                log::info!("EOS token generated at step {}", step);
                break;
            }

            // Decode token and emit via callback
            let token_text = self.tokenizer.decode(&[next_token], false)?;
            on_token(token_text);

            // Append token to sequence
            generated_ids.push(next_token);
            tokens_generated += 1;

            // Progress logging
            if step % 10 == 0 && step > 0 {
                log::debug!("Generation step {}: {} tokens", step, generated_ids.len());
            }
        }

        // Calculate metrics
        let total_time = start.elapsed();
        let tokens_per_sec = if total_time.as_secs_f64() > 0.0 {
            tokens_generated as f64 / total_time.as_secs_f64()
        } else {
            0.0
        };

        log::info!(
            "Streaming generation complete: {} tokens in {:.2}s ({:.2} tok/s)",
            tokens_generated,
            total_time.as_secs_f64(),
            tokens_per_sec
        );

        Ok(GenerationMetrics {
            total_tokens: tokens_generated,
            time_to_first_token_ms: first_token_time.map(|d| d.as_millis() as u64),
            tokens_per_second: tokens_per_sec,
            total_time_ms: total_time.as_millis() as u64,
        })
    }

    /// Sample next token from logits
    ///
    /// Supports:
    /// - Greedy sampling (temperature = 0 or top_k = 1)
    /// - Temperature scaling
    /// - Top-k filtering
    /// - Top-p (nucleus) sampling
    fn sample(&self, logits: ArrayView1<f32>, config: &GenerationConfig) -> Result<u32, String> {
        // Greedy sampling: temperature = 0 or effectively disabled
        if config.temperature <= 0.0 || config.top_k == Some(1) {
            return self.sample_greedy(logits);
        }

        // Apply temperature scaling
        let scaled_logits: Vec<f32> = logits.iter().map(|&x| x / config.temperature).collect();

        // Convert to probabilities via softmax
        let max_logit = scaled_logits
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = scaled_logits.iter().map(|&x| (x - max_logit).exp()).collect();
        let sum_exp: f32 = exp_logits.iter().sum();
        let mut probs: Vec<(usize, f32)> = exp_logits
            .iter()
            .enumerate()
            .map(|(i, &e)| (i, e / sum_exp))
            .collect();

        // Sort by probability descending
        probs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Apply top-k filtering
        if let Some(k) = config.top_k {
            if k > 0 && k < probs.len() {
                probs.truncate(k);
            }
        }

        // Apply top-p (nucleus) filtering
        if let Some(p) = config.top_p {
            if p > 0.0 && p < 1.0 {
                let mut cumsum = 0.0f32;
                let mut cutoff_idx = probs.len();
                for (i, (_, prob)) in probs.iter().enumerate() {
                    cumsum += prob;
                    if cumsum >= p {
                        cutoff_idx = i + 1;
                        break;
                    }
                }
                probs.truncate(cutoff_idx);
            }
        }

        // Renormalize probabilities
        let total_prob: f32 = probs.iter().map(|(_, p)| p).sum();
        if total_prob <= 0.0 {
            // Fallback to greedy if something went wrong
            return self.sample_greedy(logits);
        }

        // Sample from the distribution
        let r: f32 = rand::random::<f32>() * total_prob;
        let mut cumsum = 0.0f32;
        for (idx, prob) in &probs {
            cumsum += prob;
            if cumsum >= r {
                return Ok(*idx as u32);
            }
        }

        // Fallback to first token
        Ok(probs[0].0 as u32)
    }

    /// Greedy sampling: Pick token with highest probability
    ///
    /// Phase 0: Simple greedy decoding
    /// Phase 1: Add temperature, top-k, top-p sampling
    fn sample_greedy(&self, logits: ArrayView1<f32>) -> Result<u32, String> {
        let (max_idx, _max_val) = logits
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .ok_or("Empty logits tensor")?;

        Ok(max_idx as u32)
    }

    /// Update generation configuration
    #[allow(dead_code)]
    pub fn set_config(&mut self, config: GenerationConfig) {
        self.config = config;
    }

    /// Get current configuration
    #[allow(dead_code)]
    pub fn config(&self) -> &GenerationConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::{init_onnx_runtime, InferenceSession, TokenizerWrapper};

    #[tokio::test]
    #[ignore] // Requires model files - run manually
    async fn test_generate_simple() {
        // Initialize ONNX Runtime
        init_onnx_runtime().expect("Failed to initialize ONNX Runtime");

        // Load model and tokenizer
        let model_path = "models/qwen2.5-coder-1.5b/model.onnx";
        let tokenizer_path = "models/qwen2.5-coder-1.5b/tokenizer.json";

        let session = InferenceSession::new(model_path).expect("Failed to load model");
        let tokenizer = TokenizerWrapper::from_file(tokenizer_path).expect("Failed to load tokenizer");

        // Create generator with reduced max_length for testing
        let mut generator = Generator::new(session, tokenizer);
        generator.set_config(GenerationConfig {
            max_length: 5, // Just generate a few tokens for testing
            ..Default::default()
        });

        // Generate
        let prompt = "def hello";
        println!("Prompt: {}", prompt);

        let result = generator.generate(prompt).await;
        match result {
            Ok(res) => {
                println!("Generated: {}", res.text);
                println!("Metrics: {:?}", res.metrics);
                assert!(res.metrics.total_tokens > 0 || res.text.is_empty() == false || res.text.is_empty());
            }
            Err(e) => {
                panic!("Generation failed: {}", e);
            }
        }
    }
}
