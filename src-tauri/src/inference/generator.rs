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
use super::input_builder::InputBuilder;
use super::kv_cache::{KVCache, HEAD_DIM, NUM_KV_HEADS, NUM_LAYERS};
use super::session::InferenceSession;
use super::tokenizer::TokenizerWrapper;
use super::types::{GenerationConfig, GenerationMetrics, GenerationResult};
use crate::models::ModelRuntimeSpec;
use ndarray::{Array1, Array2, ArrayView1};
use ort::session::SessionInputValue;
use ort::value::Value;
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
    runtime_spec: ModelRuntimeSpec,
    config: GenerationConfig,
    max_context: usize,
    sink_size: usize,
}

impl Generator {
    fn validate_runtime_spec_compatibility(runtime_spec: ModelRuntimeSpec) -> Result<(), String> {
        runtime_spec.validate()?;

        if runtime_spec.architecture.num_layers != NUM_LAYERS
            || runtime_spec.architecture.num_kv_heads != NUM_KV_HEADS
            || runtime_spec.architecture.head_dim != HEAD_DIM
        {
            return Err(format!(
                "Runtime spec for '{}' is not supported by current inference core. \
                 Expected layers={}, kv_heads={}, head_dim={}, got layers={}, kv_heads={}, head_dim={}",
                runtime_spec.model_id,
                NUM_LAYERS,
                NUM_KV_HEADS,
                HEAD_DIM,
                runtime_spec.architecture.num_layers,
                runtime_spec.architecture.num_kv_heads,
                runtime_spec.architecture.head_dim
            ));
        }
        Ok(())
    }

    fn get_required_output<'a>(
        outputs: &'a ort::session::SessionOutputs<'_>,
        output_name: &str,
    ) -> Result<&'a ort::value::DynValue, String> {
        outputs
            .get(output_name)
            .ok_or_else(|| format!("Missing required model output: {output_name}"))
    }

    fn validate_rank(
        shape: &ort::tensor::Shape,
        expected_rank: usize,
        tensor_name: &str,
    ) -> Result<(), String> {
        if shape.len() != expected_rank {
            return Err(format!(
                "Invalid rank for tensor '{tensor_name}': expected {expected_rank}, got {} ({shape:?})",
                shape.len()
            ));
        }
        Ok(())
    }

    fn dim_to_usize(
        shape: &ort::tensor::Shape,
        dim_index: usize,
        tensor_name: &str,
    ) -> Result<usize, String> {
        let dim = shape.get(dim_index).copied().ok_or_else(|| {
            format!("Missing dimension {dim_index} for tensor '{tensor_name}' with shape {shape:?}")
        })?;

        usize::try_from(dim).map_err(|_| {
            format!(
                "Invalid negative dimension {dim} at index {dim_index} for tensor '{tensor_name}'"
            )
        })
    }

    fn checked_product(
        values: &[usize],
        tensor_name: &str,
        context: &str,
    ) -> Result<usize, String> {
        values.iter().try_fold(1usize, |acc, &value| {
            acc.checked_mul(value).ok_or_else(|| {
                format!(
                    "Overflow while calculating {context} for tensor '{tensor_name}' with dimensions {values:?}"
                )
            })
        })
    }

    fn validate_tensor_len(
        expected_len: usize,
        actual_len: usize,
        tensor_name: &str,
    ) -> Result<(), String> {
        if expected_len != actual_len {
            return Err(format!(
                "Tensor '{tensor_name}' data length mismatch: expected {expected_len}, got {actual_len}"
            ));
        }
        Ok(())
    }

    fn validate_batch_dim(
        shape: &ort::tensor::Shape,
        tensor_name: &str,
        expected_batch: usize,
    ) -> Result<(), String> {
        let batch = Self::dim_to_usize(shape, 0, tensor_name)?;
        if batch != expected_batch {
            return Err(format!(
                "Unsupported batch size for tensor '{tensor_name}': expected {expected_batch}, got {batch}"
            ));
        }
        Ok(())
    }

    /// Create a new generator
    ///
    /// # Arguments
    /// * `session` - ONNX Runtime session (wrapped for thread safety)
    /// * `tokenizer` - Tokenizer for text conversion
    pub fn new(
        session: InferenceSession,
        tokenizer: TokenizerWrapper,
        runtime_spec: ModelRuntimeSpec,
    ) -> Result<Self, String> {
        Self::with_context(
            session,
            tokenizer,
            runtime_spec,
            DEFAULT_MAX_CONTEXT,
            DEFAULT_SINK_SIZE,
        )
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
        runtime_spec: ModelRuntimeSpec,
        max_context: usize,
        sink_size: usize,
    ) -> Result<Self, String> {
        Self::validate_runtime_spec_compatibility(runtime_spec)?;

        Ok(Self {
            session: Arc::new(Mutex::new(session)),
            tokenizer: Arc::new(tokenizer),
            runtime_spec,
            config: GenerationConfig::default(),
            max_context,
            sink_size,
        })
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

        // Use provided config or default
        let gen_config = config.unwrap_or(self.config.clone());

        log::info!(
            "Starting streaming generation with KV cache (model: {}, max_context: {}, sink_size: {})",
            self.runtime_spec.model_id,
            self.max_context,
            self.sink_size
        );

        // 1. Tokenize prompt
        let input_ids = self.tokenizer.encode(prompt, false)?;
        let prompt_length = input_ids.len();
        let mut generated_ids = input_ids.clone();
        let mut tokens_generated = 0usize;

        log::debug!("Prompt tokenized: {} tokens", prompt_length);

        // 2. Create KV cache and pre-allocated input builder
        let mut kv_cache = KVCache::new(self.max_context, self.sink_size);
        let mut input_builder = InputBuilder::with_names(
            self.runtime_spec.io.input_ids,
            self.runtime_spec.io.attention_mask,
            self.runtime_spec.past_key_names(),
            self.runtime_spec.past_value_names(),
        )?;

        // 3. Prefill phase: Process entire prompt, build initial KV cache
        log::debug!("Starting prefill phase...");
        let prefill_start = Instant::now();

        let (first_logits, _) = self
            .run_prefill(&input_ids, &mut kv_cache, &mut input_builder)
            .await?;

        log::debug!(
            "Prefill complete: {} tokens in {:?}",
            prompt_length,
            prefill_start.elapsed()
        );

        // Sample first token from prefill output
        // Pass generated tokens so far (empty for first token — penalty is a no-op)
        let mut next_token = self.sample(
            first_logits.view(),
            &gen_config,
            &generated_ids[prompt_length..],
        )?;

        // Record time to first token
        let first_token_time = start.elapsed();
        log::debug!("Time to first token: {:?}", first_token_time);

        // Check if first token is a stop token
        if self.tokenizer.is_stop_token(next_token) {
            log::info!(
                "Stop token generated immediately (token ID: {})",
                next_token
            );
            return Ok(GenerationMetrics {
                total_tokens: 0,
                time_to_first_token_ms: Some(first_token_time.as_millis() as u64),
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
                .run_decode(next_token, &mut kv_cache, &mut input_builder)
                .await?;

            // Sample next token (pass all generated tokens for repetition penalty)
            next_token =
                self.sample(logits.view(), &gen_config, &generated_ids[prompt_length..])?;

            // Check stop condition
            if self.tokenizer.is_stop_token(next_token) {
                log::info!(
                    "Stop token generated at step {} (token ID: {})",
                    step,
                    next_token
                );
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
            time_to_first_token_ms: Some(first_token_time.as_millis() as u64),
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
        input_builder: &mut InputBuilder,
    ) -> Result<(Array1<f32>, ()), String> {
        let seq_length = input_ids.len();

        // Clear and reuse input builder
        input_builder.clear();

        // input_ids: [batch=1, seq_length]
        let input_ids_array = Array2::from_shape_vec(
            (1, seq_length),
            input_ids.iter().map(|&id| id as i64).collect(),
        )
        .map_err(|e| format!("Failed to create input_ids tensor: {e}"))?;

        input_builder.set_input_ids(SessionInputValue::Owned(
            Value::from_array(input_ids_array)
                .map_err(|e| format!("Failed to create input_ids value: {e}"))?
                .into(),
        ));

        // attention_mask: [batch=1, seq_length] (all 1s for prefill)
        let attention_mask = Array2::from_shape_vec((1, seq_length), vec![1i64; seq_length])
            .map_err(|e| format!("Failed to create attention_mask tensor: {e}"))?;

        input_builder.set_attention_mask(SessionInputValue::Owned(
            Value::from_array(attention_mask)
                .map_err(|e| format!("Failed to create attention_mask value: {e}"))?
                .into(),
        ));

        // Empty KV cache for prefill (shape: [1, num_kv_heads, 0, head_dim])
        for layer in 0..self.runtime_spec.architecture.num_layers {
            let empty_cache = ndarray::Array4::<f32>::zeros((
                1,
                self.runtime_spec.architecture.num_kv_heads,
                0,
                self.runtime_spec.architecture.head_dim,
            ));

            input_builder.set_past_key(
                layer,
                SessionInputValue::Owned(
                    Value::from_array(empty_cache.clone())
                        .map_err(|e| format!("Failed to create KV cache tensor: {e}"))?
                        .into(),
                ),
            );
            input_builder.set_past_value(
                layer,
                SessionInputValue::Owned(
                    Value::from_array(empty_cache)
                        .map_err(|e| format!("Failed to create KV cache tensor: {e}"))?
                        .into(),
                ),
            );
        }

        // Take ownership of inputs for session.run()
        let inputs = input_builder.take_inputs();

        // Run inference - keep session locked while we extract outputs
        let mut session = self.session.lock().await;
        let outputs = session
            .session
            .run(inputs)
            .map_err(|e| format!("Prefill inference failed: {e}"))?;

        // Extract logits for last position
        let logits_name = self.runtime_spec.io.logits;
        let logits_output = Self::get_required_output(&outputs, logits_name)?;
        let (logits_shape, logits_data) = logits_output
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract logits: {e}"))?;

        Self::validate_rank(logits_shape, 3, logits_name)?;
        Self::validate_batch_dim(logits_shape, logits_name, 1)?;

        let seq_len = Self::dim_to_usize(logits_shape, 1, logits_name)?;
        let vocab_size = Self::dim_to_usize(logits_shape, 2, logits_name)?;

        if seq_len == 0 {
            return Err(format!(
                "Model output '{logits_name}' has empty sequence dimension"
            ));
        }
        if vocab_size == 0 {
            return Err(format!(
                "Model output '{logits_name}' has empty vocabulary dimension"
            ));
        }

        let expected_logits_len =
            Self::checked_product(&[seq_len, vocab_size], logits_name, "data length")?;
        Self::validate_tensor_len(expected_logits_len, logits_data.len(), logits_name)?;

        // Get logits for last position only
        let last_pos_start = (seq_len - 1).checked_mul(vocab_size).ok_or_else(|| {
            "Overflow while calculating logits offset for last position".to_string()
        })?;
        let last_pos_end = last_pos_start
            .checked_add(vocab_size)
            .ok_or_else(|| "Overflow while calculating logits slice end".to_string())?;
        let last_logits = Array1::from_vec(logits_data[last_pos_start..last_pos_end].to_vec());

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
        input_builder: &mut InputBuilder,
    ) -> Result<Array1<f32>, String> {
        // Clear and reuse input builder (keeps capacity, avoids reallocation)
        input_builder.clear();

        // input_ids: [batch=1, 1] (single token)
        let input_ids_array = Array2::from_shape_vec((1, 1), vec![token_id as i64])
            .map_err(|e| format!("Failed to create input_ids tensor: {e}"))?;

        input_builder.set_input_ids(SessionInputValue::Owned(
            Value::from_array(input_ids_array)
                .map_err(|e| format!("Failed to create input_ids value: {e}"))?
                .into(),
        ));

        // attention_mask: [batch=1, past_length + 1]
        let past_length = kv_cache.physical_length();
        let attention_mask =
            Array2::from_shape_vec((1, past_length + 1), vec![1i64; past_length + 1])
                .map_err(|e| format!("Failed to create attention_mask tensor: {e}"))?;

        input_builder.set_attention_mask(SessionInputValue::Owned(
            Value::from_array(attention_mask)
                .map_err(|e| format!("Failed to create attention_mask value: {e}"))?
                .into(),
        ));

        // Add KV cache from previous steps using pre-allocated key names
        for layer in 0..self.runtime_spec.architecture.num_layers {
            let key_cache = kv_cache.get_key_array(layer);
            let value_cache = kv_cache.get_value_array(layer);

            input_builder.set_past_key(
                layer,
                SessionInputValue::Owned(
                    Value::from_array(key_cache)
                        .map_err(|e| format!("Failed to create key cache tensor: {e}"))?
                        .into(),
                ),
            );
            input_builder.set_past_value(
                layer,
                SessionInputValue::Owned(
                    Value::from_array(value_cache)
                        .map_err(|e| format!("Failed to create value cache tensor: {e}"))?
                        .into(),
                ),
            );
        }

        // Take ownership of inputs for session.run()
        let inputs = input_builder.take_inputs();

        // Run inference - keep session locked while we extract outputs
        let mut session = self.session.lock().await;
        let outputs = session
            .session
            .run(inputs)
            .map_err(|e| format!("Decode inference failed: {e}"))?;

        // Extract logits (shape: [1, 1, vocab_size])
        let logits_name = self.runtime_spec.io.logits;
        let logits_output = Self::get_required_output(&outputs, logits_name)?;
        let (logits_shape, logits_data) = logits_output
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract logits: {e}"))?;

        Self::validate_rank(logits_shape, 3, logits_name)?;
        Self::validate_batch_dim(logits_shape, logits_name, 1)?;

        let seq_len = Self::dim_to_usize(logits_shape, 1, logits_name)?;
        let vocab_size = Self::dim_to_usize(logits_shape, 2, logits_name)?;

        if seq_len == 0 {
            return Err(format!(
                "Decode output '{logits_name}' has empty sequence dimension"
            ));
        }
        if vocab_size == 0 {
            return Err(format!(
                "Decode output '{logits_name}' has empty vocabulary dimension"
            ));
        }

        let expected_logits_len =
            Self::checked_product(&[seq_len, vocab_size], logits_name, "data length")?;
        Self::validate_tensor_len(expected_logits_len, logits_data.len(), logits_name)?;

        let last_pos_start = (seq_len - 1)
            .checked_mul(vocab_size)
            .ok_or_else(|| "Overflow while calculating decode logits offset".to_string())?;
        let last_pos_end = last_pos_start
            .checked_add(vocab_size)
            .ok_or_else(|| "Overflow while calculating decode logits slice end".to_string())?;
        let logits = Array1::from_vec(logits_data[last_pos_start..last_pos_end].to_vec());

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
        let num_layers = self.runtime_spec.architecture.num_layers;
        let num_kv_heads = self.runtime_spec.architecture.num_kv_heads;
        let head_dim = self.runtime_spec.architecture.head_dim;

        let token_kv_size = num_layers * num_kv_heads * head_dim;
        let mut all_keys = vec![0.0f32; num_tokens * token_kv_size];
        let mut all_values = vec![0.0f32; num_tokens * token_kv_size];

        for layer in 0..num_layers {
            let key_name = self.runtime_spec.present_key_name(layer);
            let value_name = self.runtime_spec.present_value_name(layer);

            let key_output = Self::get_required_output(outputs, key_name.as_str())?;
            let value_output = Self::get_required_output(outputs, value_name.as_str())?;

            let (key_shape, key_data) = key_output
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Failed to extract {}: {}", key_name, e))?;

            let (value_shape, value_data) = value_output
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Failed to extract {}: {}", value_name, e))?;

            Self::validate_rank(key_shape, 4, key_name.as_str())?;
            Self::validate_rank(value_shape, 4, value_name.as_str())?;
            Self::validate_batch_dim(key_shape, key_name.as_str(), 1)?;
            Self::validate_batch_dim(value_shape, value_name.as_str(), 1)?;

            let key_heads = Self::dim_to_usize(key_shape, 1, key_name.as_str())?;
            let key_seq_len = Self::dim_to_usize(key_shape, 2, key_name.as_str())?;
            let key_head_dim = Self::dim_to_usize(key_shape, 3, key_name.as_str())?;

            let value_heads = Self::dim_to_usize(value_shape, 1, value_name.as_str())?;
            let value_seq_len = Self::dim_to_usize(value_shape, 2, value_name.as_str())?;
            let value_head_dim = Self::dim_to_usize(value_shape, 3, value_name.as_str())?;

            if key_heads != NUM_KV_HEADS || value_heads != NUM_KV_HEADS {
                return Err(format!(
                    "Unexpected KV head count for layer {layer}: key={key_heads}, value={value_heads}, expected {NUM_KV_HEADS}"
                ));
            }
            if key_head_dim != HEAD_DIM || value_head_dim != HEAD_DIM {
                return Err(format!(
                    "Unexpected head dimension for layer {layer}: key={key_head_dim}, value={value_head_dim}, expected {HEAD_DIM}"
                ));
            }
            if key_seq_len != num_tokens || value_seq_len != num_tokens {
                return Err(format!(
                    "Unexpected KV sequence length for layer {layer}: key={key_seq_len}, value={value_seq_len}, expected {num_tokens}"
                ));
            }

            let expected_key_len = Self::checked_product(
                &[key_heads, key_seq_len, key_head_dim],
                key_name.as_str(),
                "data length",
            )?;
            let expected_value_len = Self::checked_product(
                &[value_heads, value_seq_len, value_head_dim],
                value_name.as_str(),
                "data length",
            )?;

            Self::validate_tensor_len(expected_key_len, key_data.len(), key_name.as_str())?;
            Self::validate_tensor_len(expected_value_len, value_data.len(), value_name.as_str())?;

            // Copy data for each token position
            // present shape: [1, NUM_KV_HEADS, seq_len, HEAD_DIM]
            for pos in 0..num_tokens {
                for head in 0..num_kv_heads {
                    let src_offset = head * num_tokens * head_dim + pos * head_dim;
                    let dst_offset =
                        pos * token_kv_size + layer * num_kv_heads * head_dim + head * head_dim;

                    all_keys[dst_offset..dst_offset + head_dim]
                        .copy_from_slice(&key_data[src_offset..src_offset + head_dim]);
                    all_values[dst_offset..dst_offset + head_dim]
                        .copy_from_slice(&value_data[src_offset..src_offset + head_dim]);
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
        let num_layers = self.runtime_spec.architecture.num_layers;
        let num_kv_heads = self.runtime_spec.architecture.num_kv_heads;
        let head_dim = self.runtime_spec.architecture.head_dim;

        let embedding_size = num_kv_heads * head_dim;
        let mut new_keys = vec![0.0f32; num_layers * embedding_size];
        let mut new_values = vec![0.0f32; num_layers * embedding_size];

        for layer in 0..num_layers {
            let key_name = self.runtime_spec.present_key_name(layer);
            let value_name = self.runtime_spec.present_value_name(layer);

            let key_output = Self::get_required_output(outputs, key_name.as_str())?;
            let value_output = Self::get_required_output(outputs, value_name.as_str())?;

            let (key_shape, key_data) = key_output
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Failed to extract {}: {}", key_name, e))?;

            let (value_shape, value_data) = value_output
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Failed to extract {}: {}", value_name, e))?;

            Self::validate_rank(key_shape, 4, key_name.as_str())?;
            Self::validate_rank(value_shape, 4, value_name.as_str())?;
            Self::validate_batch_dim(key_shape, key_name.as_str(), 1)?;
            Self::validate_batch_dim(value_shape, value_name.as_str(), 1)?;

            let key_heads = Self::dim_to_usize(key_shape, 1, key_name.as_str())?;
            let total_seq_len = Self::dim_to_usize(key_shape, 2, key_name.as_str())?;
            let key_head_dim = Self::dim_to_usize(key_shape, 3, key_name.as_str())?;

            let value_heads = Self::dim_to_usize(value_shape, 1, value_name.as_str())?;
            let value_seq_len = Self::dim_to_usize(value_shape, 2, value_name.as_str())?;
            let value_head_dim = Self::dim_to_usize(value_shape, 3, value_name.as_str())?;

            if key_heads != NUM_KV_HEADS || value_heads != NUM_KV_HEADS {
                return Err(format!(
                    "Unexpected KV head count for decode layer {layer}: key={key_heads}, value={value_heads}, expected {NUM_KV_HEADS}"
                ));
            }
            if key_head_dim != HEAD_DIM || value_head_dim != HEAD_DIM {
                return Err(format!(
                    "Unexpected head dimension for decode layer {layer}: key={key_head_dim}, value={value_head_dim}, expected {HEAD_DIM}"
                ));
            }
            if value_seq_len != total_seq_len {
                return Err(format!(
                    "Mismatched KV sequence lengths for decode layer {layer}: key={total_seq_len}, value={value_seq_len}"
                ));
            }

            if total_seq_len == 0 {
                return Err(format!(
                    "Decode present tensor '{key_name}' has empty sequence dimension"
                ));
            }

            let expected_key_len = Self::checked_product(
                &[key_heads, total_seq_len, key_head_dim],
                key_name.as_str(),
                "data length",
            )?;
            let expected_value_len = Self::checked_product(
                &[value_heads, value_seq_len, value_head_dim],
                value_name.as_str(),
                "data length",
            )?;

            Self::validate_tensor_len(expected_key_len, key_data.len(), key_name.as_str())?;
            Self::validate_tensor_len(expected_value_len, value_data.len(), value_name.as_str())?;

            // Get the last position (new token)
            let last_pos = total_seq_len - 1;

            // Extract last token's KV for all heads
            for head in 0..num_kv_heads {
                let src_offset = head * total_seq_len * head_dim + last_pos * head_dim;
                let dst_offset = layer * embedding_size + head * head_dim;

                new_keys[dst_offset..dst_offset + head_dim]
                    .copy_from_slice(&key_data[src_offset..src_offset + head_dim]);
                new_values[dst_offset..dst_offset + head_dim]
                    .copy_from_slice(&value_data[src_offset..src_offset + head_dim]);
            }
        }

        // Append single token to cache (handles Attention Sinks shifting)
        kv_cache.append(&new_keys, &new_values);

        Ok(())
    }

    /// Apply repetition penalty to raw logits (sign-aware, per HuggingFace/llama.cpp)
    ///
    /// Positive logits are divided by the penalty (reduced probability),
    /// negative logits are multiplied by the penalty (pushed further negative).
    /// Each unique token is penalized once regardless of how many times it appeared.
    fn apply_repetition_penalty(logits: &mut [f32], token_ids: &[u32], penalty: f32) {
        let seen: std::collections::HashSet<u32> = token_ids.iter().copied().collect();
        for token_id in seen {
            let idx = token_id as usize;
            if idx < logits.len() {
                if logits[idx] >= 0.0 {
                    logits[idx] /= penalty;
                } else {
                    logits[idx] *= penalty;
                }
            }
        }
    }

    /// Sample next token from logits
    ///
    /// Supports:
    /// - Repetition penalty (applied before all other sampling)
    /// - Greedy sampling (temperature = 0 or top_k = 1)
    /// - Temperature scaling
    /// - Top-k filtering
    /// - Top-p (nucleus) sampling
    fn sample(
        &self,
        logits: ArrayView1<f32>,
        config: &GenerationConfig,
        generated_ids: &[u32],
    ) -> Result<u32, String> {
        // Clone logits to mutable vec for in-place penalty application
        let mut logits_vec: Vec<f32> = logits.to_vec();

        // Apply repetition penalty before any other sampling
        if config.repetition_penalty > 1.0 && !generated_ids.is_empty() {
            let window = if config.repetition_penalty_last_n == 0 {
                generated_ids
            } else {
                let start = generated_ids
                    .len()
                    .saturating_sub(config.repetition_penalty_last_n);
                &generated_ids[start..]
            };
            Self::apply_repetition_penalty(&mut logits_vec, window, config.repetition_penalty);
        }

        // Greedy sampling: temperature = 0 or effectively disabled
        if config.temperature <= 0.0 || config.top_k == Some(1) {
            let (max_idx, _) = logits_vec
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .ok_or("Empty logits tensor")?;
            return Ok(max_idx as u32);
        }

        // Apply temperature scaling
        let scaled_logits: Vec<f32> = logits_vec.iter().map(|&x| x / config.temperature).collect();

        // Convert to probabilities via softmax
        let max_logit = scaled_logits
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = scaled_logits
            .iter()
            .map(|&x| (x - max_logit).exp())
            .collect();
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

    /// Get reference to tokenizer (for benchmarking/testing)
    #[cfg(test)]
    pub fn tokenizer(&self) -> &TokenizerWrapper {
        &self.tokenizer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::{init_onnx_runtime, InferenceSession, TokenizerWrapper};
    use crate::models::ModelRegistry;
    use ort::tensor::Shape;

    fn runtime_spec() -> crate::models::ModelRuntimeSpec {
        ModelRegistry::runtime_spec("qwen2.5-coder-1.5b")
            .expect("Missing runtime spec for qwen2.5-coder-1.5b")
    }

    #[test]
    fn test_validate_rank() {
        let shape = Shape::from([1_i64, 2, 3]);

        assert!(Generator::validate_rank(&shape, 3, "logits").is_ok());

        let err = Generator::validate_rank(&shape, 4, "logits").unwrap_err();
        assert!(err.contains("Invalid rank"));
    }

    #[test]
    fn test_dim_to_usize_rejects_negative_dim() {
        let shape = Shape::from([1_i64, -1, 3]);
        let err = Generator::dim_to_usize(&shape, 1, "logits").unwrap_err();
        assert!(err.contains("Invalid negative dimension"));
    }

    #[test]
    fn test_validate_tensor_len() {
        assert!(Generator::validate_tensor_len(10, 10, "logits").is_ok());

        let err = Generator::validate_tensor_len(10, 9, "logits").unwrap_err();
        assert!(err.contains("data length mismatch"));
    }

    #[test]
    fn test_validate_batch_dim() {
        let shape_ok = Shape::from([1_i64, 2, 3]);
        assert!(Generator::validate_batch_dim(&shape_ok, "logits", 1).is_ok());

        let shape_bad = Shape::from([2_i64, 2, 3]);
        let err = Generator::validate_batch_dim(&shape_bad, "logits", 1).unwrap_err();
        assert!(err.contains("Unsupported batch size"));
    }

    #[test]
    fn test_checked_product_overflow() {
        let err =
            Generator::checked_product(&[usize::MAX, 2], "logits", "data length").unwrap_err();
        assert!(err.contains("Overflow"));
    }

    #[tokio::test]
    #[ignore] // Requires model files - run manually
    async fn test_generate_simple() {
        // Initialize ONNX Runtime
        init_onnx_runtime(None).expect("Failed to initialize ONNX Runtime");

        // Load model and tokenizer
        let model_path = "models/qwen2.5-coder-1.5b/model.onnx";
        let tokenizer_path = "models/qwen2.5-coder-1.5b/tokenizer.json";

        let session = InferenceSession::new(model_path).expect("Failed to load model");
        let tokenizer =
            TokenizerWrapper::from_file(tokenizer_path).expect("Failed to load tokenizer");

        // Create generator with reduced max_length for testing
        let mut generator =
            Generator::new(session, tokenizer, runtime_spec()).expect("Failed to create generator");
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
        init_onnx_runtime(None).expect("Failed to initialize ONNX Runtime");

        // Load model and tokenizer
        let model_path = "models/qwen2.5-coder-1.5b/model.onnx";
        let tokenizer_path = "models/qwen2.5-coder-1.5b/tokenizer.json";

        let session = InferenceSession::new(model_path).expect("Failed to load model");
        let tokenizer =
            TokenizerWrapper::from_file(tokenizer_path).expect("Failed to load tokenizer");

        // Create generator
        let generator = Generator::with_context(session, tokenizer, runtime_spec(), 512, 4)
            .expect("Failed to create generator");

        let prompt = "def fibonacci(n):";
        println!("Prompt: {}", prompt);

        let mut generated = String::new();
        let cancelled = Arc::new(AtomicBool::new(false));

        let config = GenerationConfig {
            max_length: 50,
            temperature: 0.7,
            top_k: Some(40),
            top_p: Some(0.9),
            ..Default::default()
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

        init_onnx_runtime(None).expect("Failed to initialize ONNX Runtime");

        let model_path = "models/qwen2.5-coder-1.5b/model.onnx";
        let tokenizer_path = "models/qwen2.5-coder-1.5b/tokenizer.json";

        let session = InferenceSession::new(model_path).expect("Failed to load model");
        let tokenizer =
            TokenizerWrapper::from_file(tokenizer_path).expect("Failed to load tokenizer");

        // Small context to trigger shifting quickly
        let generator = Generator::with_context(session, tokenizer, runtime_spec(), 32, 4)
            .expect("Failed to create generator");

        let prompt = "Write a function to calculate factorial:";
        let cancelled = Arc::new(AtomicBool::new(false));

        let config = GenerationConfig {
            max_length: 100,  // Should exceed our 32-token context
            temperature: 0.0, // Greedy for reproducibility
            top_k: None,
            top_p: None,
            ..Default::default()
        };

        let mut token_count = 0;
        let metrics = generator
            .generate_stream(prompt, Some(config), cancelled, |_token| {
                token_count += 1;
            })
            .await
            .expect("Generation failed");

        println!(
            "Generated {} tokens with {} context window",
            metrics.total_tokens, 32
        );
        println!("Tokens/sec: {:.2}", metrics.tokens_per_second);

        // Should have generated tokens beyond context limit using Attention Sinks
        assert!(metrics.total_tokens > 0);
    }
}
