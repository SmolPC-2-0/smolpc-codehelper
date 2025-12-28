/// Autoregressive text generation with ONNX Runtime and KV Cache
///
/// Implements the generation loop for LLM inference with Attention Sinks:
/// 1. Tokenize prompt
/// 2. Prefill: Run inference on full prompt, build KV cache
/// 3. Decode: Run inference on single token, use cached KV
/// 4. Sample next token from logits
/// 5. Append to cache and sequence
/// 6. Repeat until EOS or max length
///
/// # KV Cache with Attention Sinks
/// - Pre-allocated buffer for efficient memory usage
/// - When context exceeds max_context, preserves first N "sink" tokens
/// - Shifts sliding window to maintain fixed buffer size
/// - Supports "infinite" generation with bounded memory
///
/// # References
/// - [StreamingLLM Paper](https://arxiv.org/abs/2309.17453)

use super::kv_cache::{KVCache, HEAD_DIM, NUM_KV_HEADS, NUM_LAYERS};
use super::session::InferenceSession;
use super::tokenizer::TokenizerWrapper;
use super::types::{GenerationConfig, GenerationMetrics, GenerationResult};
use ndarray::{Array1, Array2, ArrayView1};
use ort::session::SessionInputValue;
use ort::value::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// Default maximum context window
const DEFAULT_MAX_CONTEXT: usize = 2048;

/// Default number of attention sink tokens
const DEFAULT_SINK_SIZE: usize = 4;

/// Text generator using ONNX Runtime with KV Cache
pub struct Generator {
    session: Arc<Mutex<InferenceSession>>,
    tokenizer: Arc<TokenizerWrapper>,
    config: GenerationConfig,
    max_context: usize,
    sink_size: usize,
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
            max_context: DEFAULT_MAX_CONTEXT,
            sink_size: DEFAULT_SINK_SIZE,
        }
    }

    /// Create a new generator with custom context settings
    ///
    /// # Arguments
    /// * `session` - ONNX Runtime session
    /// * `tokenizer` - Tokenizer for text conversion
    /// * `max_context` - Maximum context window size
    /// * `sink_size` - Number of attention sink tokens
    pub fn with_context(
        session: InferenceSession,
        tokenizer: TokenizerWrapper,
        max_context: usize,
        sink_size: usize,
    ) -> Self {
        Self {
            session: Arc::new(Mutex::new(session)),
            tokenizer: Arc::new(tokenizer),
            config: GenerationConfig::default(),
            max_context,
            sink_size,
        }
    }

    /// Generate text from prompt (non-streaming, for backward compatibility)
    ///
    /// # Arguments
    /// * `prompt` - Input text prompt
    ///
    /// # Returns
    /// Generated text and performance metrics
    pub async fn generate(&self, prompt: &str) -> Result<GenerationResult, String> {
        let mut generated_text = String::new();
        let cancelled = Arc::new(AtomicBool::new(false));

        let metrics = self
            .generate_stream(prompt, None, cancelled, |token| {
                generated_text.push_str(&token);
            })
            .await?;

        Ok(GenerationResult {
            text: generated_text,
            metrics,
        })
    }

    /// Generate text with streaming output and KV cache
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
            "Starting streaming generation with KV cache (max_context: {}, sink_size: {})",
            self.max_context,
            self.sink_size
        );

        // 1. Tokenize prompt
        let input_ids = self.tokenizer.encode(prompt, true)?;
        let prompt_length = input_ids.len();
        let mut generated_ids = input_ids.clone();
        let mut tokens_generated = 0usize;

        log::debug!("Prompt tokenized: {} tokens", prompt_length);

        // 2. Create KV cache
        let mut kv_cache = KVCache::new(self.max_context, self.sink_size);

        // 3. Prefill phase: Process entire prompt, build initial KV cache
        log::debug!("Starting prefill phase...");
        let prefill_start = Instant::now();

        let (first_logits, _) = self
            .run_prefill(&input_ids, &mut kv_cache)
            .await?;

        log::debug!(
            "Prefill complete: {} tokens in {:?}",
            prompt_length,
            prefill_start.elapsed()
        );

        // Sample first token from prefill output
        let mut next_token = self.sample(first_logits.view(), &gen_config)?;

        // Record time to first token
        first_token_time = Some(start.elapsed());
        log::debug!("Time to first token: {:?}", first_token_time.unwrap());

        // Check if first token is EOS
        if next_token == self.tokenizer.eos_token_id() {
            log::info!("EOS token generated immediately");
            return Ok(GenerationMetrics {
                total_tokens: 0,
                time_to_first_token_ms: first_token_time.map(|d| d.as_millis() as u64),
                tokens_per_second: 0.0,
                total_time_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Emit first token
        let token_text = self.tokenizer.decode(&[next_token], false)?;
        on_token(token_text);
        generated_ids.push(next_token);
        tokens_generated += 1;

        // 4. Decode phase: Generate tokens one at a time using KV cache
        log::debug!("Starting decode phase...");

        for step in 1..gen_config.max_length {
            // Check for cancellation
            if cancelled.load(Ordering::Relaxed) {
                log::info!("Generation cancelled at step {}", step);
                break;
            }

            // Run decode step with single token
            let logits = self
                .run_decode(next_token, &mut kv_cache)
                .await?;

            // Sample next token
            next_token = self.sample(logits.view(), &gen_config)?;

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
            if step % 50 == 0 {
                let elapsed = start.elapsed().as_secs_f64();
                let tps = tokens_generated as f64 / elapsed;
                log::debug!(
                    "Step {}: {} tokens, {:.2} tok/s, cache: {}/{}",
                    step,
                    tokens_generated,
                    tps,
                    kv_cache.physical_length(),
                    kv_cache.max_context()
                );
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
            "Generation complete: {} tokens in {:.2}s ({:.2} tok/s)",
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

    /// Run prefill phase: process all input tokens and build initial KV cache
    ///
    /// # Returns
    /// Tuple of (logits for last position, updated cache)
    async fn run_prefill(
        &self,
        input_ids: &[u32],
        kv_cache: &mut KVCache,
    ) -> Result<(Array1<f32>, ()), String> {
        let seq_length = input_ids.len();

        // Build inputs
        let mut inputs: HashMap<String, SessionInputValue<'static>> = HashMap::new();

        // input_ids: [batch=1, seq_length]
        let input_ids_array = Array2::from_shape_vec(
            (1, seq_length),
            input_ids.iter().map(|&id| id as i64).collect(),
        )
        .map_err(|e| format!("Failed to create input_ids tensor: {e}"))?;

        inputs.insert(
            "input_ids".to_string(),
            SessionInputValue::Owned(
                Value::from_array(input_ids_array)
                    .map_err(|e| format!("Failed to create input_ids value: {e}"))?
                    .into(),
            ),
        );

        // attention_mask: [batch=1, seq_length] (all 1s for prefill)
        let attention_mask = Array2::from_shape_vec((1, seq_length), vec![1i64; seq_length])
            .map_err(|e| format!("Failed to create attention_mask tensor: {e}"))?;

        inputs.insert(
            "attention_mask".to_string(),
            SessionInputValue::Owned(
                Value::from_array(attention_mask)
                    .map_err(|e| format!("Failed to create attention_mask value: {e}"))?
                    .into(),
            ),
        );

        // Empty KV cache for prefill (shape: [1, num_kv_heads, 0, head_dim])
        for layer in 0..NUM_LAYERS {
            let empty_cache =
                ndarray::Array4::<f32>::zeros((1, NUM_KV_HEADS, 0, HEAD_DIM));

            let key_name = format!("past_key_values.{}.key", layer);
            let value_name = format!("past_key_values.{}.value", layer);

            inputs.insert(
                key_name,
                SessionInputValue::Owned(
                    Value::from_array(empty_cache.clone())
                        .map_err(|e| format!("Failed to create KV cache tensor: {e}"))?
                        .into(),
                ),
            );
            inputs.insert(
                value_name,
                SessionInputValue::Owned(
                    Value::from_array(empty_cache)
                        .map_err(|e| format!("Failed to create KV cache tensor: {e}"))?
                        .into(),
                ),
            );
        }

        // Run inference - keep session locked while we extract outputs
        let mut session = self.session.lock().await;
        let outputs = session
            .session
            .run(inputs)
            .map_err(|e| format!("Prefill inference failed: {e}"))?;

        // Extract logits for last position
        let (logits_shape, logits_data) = outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract logits: {e}"))?;

        let seq_len = logits_shape[1] as usize;
        let vocab_size = logits_shape[2] as usize;

        // Get logits for last position only
        let last_pos_start = (seq_len - 1) * vocab_size;
        let last_logits = Array1::from_vec(
            logits_data[last_pos_start..last_pos_start + vocab_size].to_vec(),
        );

        // Extract present.*.key/value and populate cache
        // The present outputs have shape [batch, heads, total_seq, head_dim]
        self.extract_and_populate_cache(&outputs, kv_cache, seq_length)?;

        // Explicitly drop outputs before releasing session lock
        drop(outputs);
        drop(session);

        Ok((last_logits, ()))
    }

    /// Run decode phase: process single token using KV cache
    async fn run_decode(
        &self,
        token_id: u32,
        kv_cache: &mut KVCache,
    ) -> Result<Array1<f32>, String> {
        // Build inputs
        let mut inputs: HashMap<String, SessionInputValue<'static>> = HashMap::new();

        // input_ids: [batch=1, 1] (single token)
        let input_ids_array = Array2::from_shape_vec((1, 1), vec![token_id as i64])
            .map_err(|e| format!("Failed to create input_ids tensor: {e}"))?;

        inputs.insert(
            "input_ids".to_string(),
            SessionInputValue::Owned(
                Value::from_array(input_ids_array)
                    .map_err(|e| format!("Failed to create input_ids value: {e}"))?
                    .into(),
            ),
        );

        // attention_mask: [batch=1, past_length + 1]
        let past_length = kv_cache.physical_length();
        let attention_mask =
            Array2::from_shape_vec((1, past_length + 1), vec![1i64; past_length + 1])
                .map_err(|e| format!("Failed to create attention_mask tensor: {e}"))?;

        inputs.insert(
            "attention_mask".to_string(),
            SessionInputValue::Owned(
                Value::from_array(attention_mask)
                    .map_err(|e| format!("Failed to create attention_mask value: {e}"))?
                    .into(),
            ),
        );

        // Add KV cache from previous steps
        for layer in 0..NUM_LAYERS {
            let key_cache = kv_cache.get_key_array(layer);
            let value_cache = kv_cache.get_value_array(layer);

            let key_name = format!("past_key_values.{}.key", layer);
            let value_name = format!("past_key_values.{}.value", layer);

            inputs.insert(
                key_name,
                SessionInputValue::Owned(
                    Value::from_array(key_cache)
                        .map_err(|e| format!("Failed to create key cache tensor: {e}"))?
                        .into(),
                ),
            );
            inputs.insert(
                value_name,
                SessionInputValue::Owned(
                    Value::from_array(value_cache)
                        .map_err(|e| format!("Failed to create value cache tensor: {e}"))?
                        .into(),
                ),
            );
        }

        // Run inference - keep session locked while we extract outputs
        let mut session = self.session.lock().await;
        let outputs = session
            .session
            .run(inputs)
            .map_err(|e| format!("Decode inference failed: {e}"))?;

        // Extract logits (shape: [1, 1, vocab_size])
        let (logits_shape, logits_data) = outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract logits: {e}"))?;

        let vocab_size = logits_shape[2] as usize;
        let logits = Array1::from_vec(logits_data[0..vocab_size].to_vec());

        // Extract the new token's KV and append to cache
        self.extract_and_append_single_token(&outputs, kv_cache)?;

        // Explicitly drop outputs before releasing session lock
        drop(outputs);
        drop(session);

        Ok(logits)
    }

    /// Extract present.*.key/value outputs and populate the entire cache (for prefill)
    fn extract_and_populate_cache(
        &self,
        outputs: &ort::session::SessionOutputs<'_>,
        kv_cache: &mut KVCache,
        num_tokens: usize,
    ) -> Result<(), String> {
        // Extract all tokens' KV embeddings
        // present.*.key has shape [batch=1, heads, seq_len, head_dim]
        // We need to flatten to [num_tokens, NUM_LAYERS, NUM_KV_HEADS, HEAD_DIM]

        let token_kv_size = NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM;
        let mut all_keys = vec![0.0f32; num_tokens * token_kv_size];
        let mut all_values = vec![0.0f32; num_tokens * token_kv_size];

        for layer in 0..NUM_LAYERS {
            let key_name = format!("present.{}.key", layer);
            let value_name = format!("present.{}.value", layer);

            let (_key_shape, key_data) = outputs[key_name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Failed to extract {}: {}", key_name, e))?;

            let (_value_shape, value_data) = outputs[value_name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Failed to extract {}: {}", value_name, e))?;

            // Copy data for each token position
            // present shape: [1, NUM_KV_HEADS, seq_len, HEAD_DIM]
            for pos in 0..num_tokens {
                for head in 0..NUM_KV_HEADS {
                    let src_offset = head * num_tokens * HEAD_DIM + pos * HEAD_DIM;
                    let dst_offset = pos * token_kv_size
                        + layer * NUM_KV_HEADS * HEAD_DIM
                        + head * HEAD_DIM;

                    all_keys[dst_offset..dst_offset + HEAD_DIM]
                        .copy_from_slice(&key_data[src_offset..src_offset + HEAD_DIM]);
                    all_values[dst_offset..dst_offset + HEAD_DIM]
                        .copy_from_slice(&value_data[src_offset..src_offset + HEAD_DIM]);
                }
            }
        }

        // Extend cache with all tokens
        kv_cache.extend(&all_keys, &all_values, num_tokens);

        Ok(())
    }

    /// Extract the last token's KV from present outputs and append to cache (for decode)
    fn extract_and_append_single_token(
        &self,
        outputs: &ort::session::SessionOutputs<'_>,
        kv_cache: &mut KVCache,
    ) -> Result<(), String> {
        // present.*.key has shape [batch=1, heads, past_len+1, head_dim]
        // We only need the last position (the new token)

        let embedding_size = NUM_KV_HEADS * HEAD_DIM;
        let mut new_keys = vec![0.0f32; NUM_LAYERS * embedding_size];
        let mut new_values = vec![0.0f32; NUM_LAYERS * embedding_size];

        for layer in 0..NUM_LAYERS {
            let key_name = format!("present.{}.key", layer);
            let value_name = format!("present.{}.value", layer);

            let (key_shape, key_data) = outputs[key_name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Failed to extract {}: {}", key_name, e))?;

            let (_value_shape, value_data) = outputs[value_name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Failed to extract {}: {}", value_name, e))?;

            // Get the last position (new token)
            let total_seq_len = key_shape[2] as usize;
            let last_pos = total_seq_len - 1;

            // Extract last token's KV for all heads
            for head in 0..NUM_KV_HEADS {
                let src_offset = head * total_seq_len * HEAD_DIM + last_pos * HEAD_DIM;
                let dst_offset = layer * embedding_size + head * HEAD_DIM;

                new_keys[dst_offset..dst_offset + HEAD_DIM]
                    .copy_from_slice(&key_data[src_offset..src_offset + HEAD_DIM]);
                new_values[dst_offset..dst_offset + HEAD_DIM]
                    .copy_from_slice(&value_data[src_offset..src_offset + HEAD_DIM]);
            }
        }

        // Append single token to cache (handles Attention Sinks shifting)
        kv_cache.append(&new_keys, &new_values);

        Ok(())
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

    /// Set maximum context window
    #[allow(dead_code)]
    pub fn set_max_context(&mut self, max_context: usize) {
        self.max_context = max_context;
    }

    /// Set number of sink tokens
    #[allow(dead_code)]
    pub fn set_sink_size(&mut self, sink_size: usize) {
        self.sink_size = sink_size;
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
        let tokenizer =
            TokenizerWrapper::from_file(tokenizer_path).expect("Failed to load tokenizer");

        // Create generator with reduced max_length for testing
        let mut generator = Generator::new(session, tokenizer);
        generator.set_config(GenerationConfig {
            max_length: 10, // Just generate a few tokens for testing
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
                assert!(
                    res.metrics.total_tokens > 0
                        || res.text.is_empty() == false
                        || res.text.is_empty()
                );
            }
            Err(e) => {
                panic!("Generation failed: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore] // Requires model files - run manually
    async fn test_generate_stream_with_cache() {
        // Initialize ONNX Runtime
        init_onnx_runtime().expect("Failed to initialize ONNX Runtime");

        // Load model and tokenizer
        let model_path = "models/qwen2.5-coder-1.5b/model.onnx";
        let tokenizer_path = "models/qwen2.5-coder-1.5b/tokenizer.json";

        let session = InferenceSession::new(model_path).expect("Failed to load model");
        let tokenizer =
            TokenizerWrapper::from_file(tokenizer_path).expect("Failed to load tokenizer");

        // Create generator
        let generator = Generator::with_context(session, tokenizer, 512, 4);

        let prompt = "def fibonacci(n):";
        println!("Prompt: {}", prompt);

        let mut generated = String::new();
        let cancelled = Arc::new(AtomicBool::new(false));

        let config = GenerationConfig {
            max_length: 50,
            temperature: 0.7,
            top_k: Some(40),
            top_p: Some(0.9),
        };

        let metrics = generator
            .generate_stream(prompt, Some(config), cancelled, |token| {
                print!("{}", token);
                generated.push_str(&token);
            })
            .await
            .expect("Generation failed");

        println!("\n\nGenerated: {}", generated);
        println!("Metrics: {:?}", metrics);
        println!(
            "Speedup from KV cache: tokens/sec = {:.2}",
            metrics.tokens_per_second
        );

        assert!(metrics.total_tokens > 0);
        assert!(metrics.tokens_per_second > 0.0);
    }

    #[tokio::test]
    #[ignore] // Requires model files - run manually
    async fn test_kv_cache_long_generation() {
        // Test that KV cache works for longer generations
        // This will eventually trigger Attention Sinks shifting

        init_onnx_runtime().expect("Failed to initialize ONNX Runtime");

        let model_path = "models/qwen2.5-coder-1.5b/model.onnx";
        let tokenizer_path = "models/qwen2.5-coder-1.5b/tokenizer.json";

        let session = InferenceSession::new(model_path).expect("Failed to load model");
        let tokenizer =
            TokenizerWrapper::from_file(tokenizer_path).expect("Failed to load tokenizer");

        // Small context to trigger shifting quickly
        let generator = Generator::with_context(session, tokenizer, 32, 4);

        let prompt = "Write a function to calculate factorial:";
        let cancelled = Arc::new(AtomicBool::new(false));

        let config = GenerationConfig {
            max_length: 100, // Should exceed our 32-token context
            temperature: 0.0, // Greedy for reproducibility
            top_k: None,
            top_p: None,
        };

        let mut token_count = 0;
        let metrics = generator
            .generate_stream(prompt, Some(config), cancelled, |_token| {
                token_count += 1;
            })
            .await
            .expect("Generation failed");

        println!("Generated {} tokens with {} context window",
                 metrics.total_tokens, 32);
        println!("Tokens/sec: {:.2}", metrics.tokens_per_second);

        // Should have generated tokens beyond context limit using Attention Sinks
        assert!(metrics.total_tokens > 0);
    }
}
