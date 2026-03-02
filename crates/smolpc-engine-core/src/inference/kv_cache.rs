//! KV Cache implementation with Attention Sinks for efficient streaming inference.
//!
//! This module implements a pre-allocated, shift-and-append KV cache that supports
//! "infinite" context through the Attention Sinks technique.
//!
//! # References
//! - [StreamingLLM Paper](https://arxiv.org/abs/2309.17453)
//! - [MIT Han Lab Implementation](https://github.com/mit-han-lab/streaming-llm)
//!
//! # Memory Layout
//! Each layer has two buffers (Key and Value), stored as flattened `Vec<f32>`:
//! - Shape: `[KV_HEADS, MAX_CONTEXT, HEAD_DIM]`
//! - Flattened as: head0_pos0_dim0..dim127, head0_pos1_dim0..dim127, ..., head1_pos0_dim0...
//!
//! # Attention Sinks Strategy
//! When the cache exceeds `max_context`:
//! 1. Keep first `sink_size` tokens fixed (they act as "attention sinks")
//! 2. Shift the sliding window left by 1 (discard oldest non-sink token)
//! 3. Append new token at the end

use ndarray::Array4;

/// Model architecture constants for Qwen2.5-Coder-1.5B
/// TODO: Read from model config in future phases
pub const NUM_LAYERS: usize = 28;
pub const NUM_KV_HEADS: usize = 2; // GQA (Grouped Query Attention)
pub const HEAD_DIM: usize = 128;

/// Single layer's KV cache (Key or Value buffer)
#[derive(Debug, Clone)]
pub struct LayerCache {
    /// Flattened storage: [KV_HEADS, MAX_CONTEXT, HEAD_DIM] -> Vec<f32>
    /// Layout: contiguous by head, then position, then dimension
    data: Vec<f32>,

    /// Maximum sequence length (physical buffer capacity)
    max_context: usize,

    /// Current number of tokens stored (physical length, <= max_context)
    current_length: usize,
}

impl LayerCache {
    /// Create a new pre-allocated layer cache
    ///
    /// # Arguments
    /// * `max_context` - Maximum context window size (e.g., 4096)
    pub fn new(max_context: usize) -> Self {
        let capacity = NUM_KV_HEADS * max_context * HEAD_DIM;
        Self {
            data: vec![0.0f32; capacity],
            max_context,
            current_length: 0,
        }
    }

    /// Get the stride for one head (positions * dimensions)
    #[inline]
    const fn head_stride(&self) -> usize {
        self.max_context * HEAD_DIM
    }

    /// Get the stride for one position (dimensions)
    #[inline]
    const fn pos_stride(&self) -> usize {
        HEAD_DIM
    }

    /// Calculate byte offset for a specific (head, position) pair
    #[inline]
    fn offset(&self, head: usize, pos: usize) -> usize {
        head * self.head_stride() + pos * self.pos_stride()
    }

    /// Get a mutable slice for a specific (head, position) pair
    fn get_position_mut(&mut self, head: usize, pos: usize) -> &mut [f32] {
        let start = self.offset(head, pos);
        &mut self.data[start..start + HEAD_DIM]
    }

    /// Write embeddings for a single token at the specified position
    ///
    /// # Arguments
    /// * `position` - Token position in the cache (0-indexed)
    /// * `embeddings` - Embedding data, shape: [KV_HEADS, HEAD_DIM] flattened
    ///
    /// # Panics
    /// Panics if position >= max_context or embeddings length is wrong
    fn write_at(&mut self, position: usize, embeddings: &[f32]) {
        debug_assert!(position < self.max_context, "Position out of bounds");
        debug_assert_eq!(
            embeddings.len(),
            NUM_KV_HEADS * HEAD_DIM,
            "Embedding size mismatch"
        );

        for head in 0..NUM_KV_HEADS {
            let src_start = head * HEAD_DIM;
            let src_end = src_start + HEAD_DIM;
            let dst = self.get_position_mut(head, position);
            dst.copy_from_slice(&embeddings[src_start..src_end]);
        }
    }

    /// Shift the window portion left by `count` positions, preserving sink tokens
    ///
    /// Uses `copy_within` for safe, efficient memory movement.
    ///
    /// # Arguments
    /// * `sink_size` - Number of sink tokens to preserve at the start
    /// * `count` - Number of positions to shift left (tokens to discard)
    fn shift_left(&mut self, sink_size: usize, count: usize) {
        if count == 0 || self.current_length <= sink_size {
            return;
        }

        // For each head, shift positions [sink_size + count .. current_length)
        // to [sink_size .. current_length - count)
        for head in 0..NUM_KV_HEADS {
            let src_start = self.offset(head, sink_size + count);
            let src_end = self.offset(head, self.current_length);
            let dst_start = self.offset(head, sink_size);

            // Safe shift using copy_within
            self.data.copy_within(src_start..src_end, dst_start);
        }

        self.current_length -= count;
    }

    /// Clear the cache (reset to empty)
    #[cfg(test)]
    pub fn clear(&mut self) {
        self.current_length = 0;
        // Note: We don't zero the buffer for performance; it will be overwritten
    }

    /// Get current number of cached tokens
    #[inline]
    pub fn len(&self) -> usize {
        self.current_length
    }

    /// Check if cache is empty
    #[inline]
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.current_length == 0
    }

    /// Check if cache is at capacity
    #[inline]
    #[cfg(test)]
    pub fn is_full(&self) -> bool {
        self.current_length >= self.max_context
    }

    /// Create an ndarray with proper shape for ONNX
    ///
    /// Returns shape: [1, KV_HEADS, current_length, HEAD_DIM]
    ///
    /// # Performance Note
    /// Uses bulk `extend_from_slice` for efficient copying instead of
    /// element-by-element assignment. True zero-copy via IoBinding
    /// is planned for Phase 2 (GPU/NPU optimization).
    pub fn to_array(&self) -> Array4<f32> {
        if self.current_length == 0 {
            return Array4::zeros((1, NUM_KV_HEADS, 0, HEAD_DIM));
        }

        // Pre-allocate vec with exact capacity needed
        let total_elements = NUM_KV_HEADS * self.current_length * HEAD_DIM;
        let mut data = Vec::with_capacity(total_elements);

        // Bulk copy each head's valid data (contiguous per head)
        for head in 0..NUM_KV_HEADS {
            let start = self.offset(head, 0);
            let end = start + self.current_length * HEAD_DIM;
            data.extend_from_slice(&self.data[start..end]);
        }

        // Reshape into ONNX format: [batch=1, heads, seq_len, head_dim]
        Array4::from_shape_vec((1, NUM_KV_HEADS, self.current_length, HEAD_DIM), data)
            .expect("Shape mismatch in LayerCache::to_array - this is a bug")
    }
}

/// Full KV cache for all transformer layers
#[derive(Debug)]
pub struct KVCache {
    /// Key caches for each layer
    key_caches: Vec<LayerCache>,

    /// Value caches for each layer
    value_caches: Vec<LayerCache>,

    /// Maximum context window
    max_context: usize,

    /// Number of sink tokens to preserve (default: 4)
    sink_size: usize,

    /// Logical token count (total tokens ever seen, for position IDs)
    /// This grows indefinitely even as physical cache wraps
    logical_length: usize,
}

impl KVCache {
    /// Create a new KV cache with pre-allocated buffers
    ///
    /// # Arguments
    /// * `max_context` - Maximum context window size (e.g., 4096)
    /// * `sink_size` - Number of attention sink tokens to preserve (default: 4)
    ///
    /// # Memory Usage
    /// Allocates: `2 * NUM_LAYERS * NUM_KV_HEADS * max_context * HEAD_DIM * 4` bytes
    /// For 28 layers, 2 heads, 4096 context, 128 dim: ~224 MB
    pub fn new(max_context: usize, sink_size: usize) -> Self {
        assert!(
            sink_size < max_context,
            "sink_size must be less than max_context"
        );
        assert!(max_context > 0, "max_context must be positive");

        let key_caches = (0..NUM_LAYERS)
            .map(|_| LayerCache::new(max_context))
            .collect();
        let value_caches = (0..NUM_LAYERS)
            .map(|_| LayerCache::new(max_context))
            .collect();

        Self {
            key_caches,
            value_caches,
            max_context,
            sink_size,
            logical_length: 0,
        }
    }

    /// Append a single token's KV embeddings to all layers
    ///
    /// Implements Attention Sinks: when full, preserves first `sink_size` tokens
    /// and shifts the sliding window.
    ///
    /// # Arguments
    /// * `key_embeddings` - Key embeddings for all layers
    ///   Shape: [NUM_LAYERS, NUM_KV_HEADS, HEAD_DIM] flattened
    /// * `value_embeddings` - Value embeddings for all layers
    ///   Shape: [NUM_LAYERS, NUM_KV_HEADS, HEAD_DIM] flattened
    ///
    /// # Returns
    /// The position at which the token was written (physical index)
    pub fn append(&mut self, key_embeddings: &[f32], value_embeddings: &[f32]) -> usize {
        let embedding_size = NUM_KV_HEADS * HEAD_DIM;
        let total_size = NUM_LAYERS * embedding_size;

        debug_assert_eq!(
            key_embeddings.len(),
            total_size,
            "Key embeddings size mismatch"
        );
        debug_assert_eq!(
            value_embeddings.len(),
            total_size,
            "Value embeddings size mismatch"
        );

        // Determine physical write position
        let physical_length = self.key_caches[0].len();
        let write_pos = if physical_length >= self.max_context {
            // Cache full: shift and write at end
            self.shift_all_caches();
            self.max_context - 1
        } else {
            // Cache has room: write at current end
            physical_length
        };

        // Write embeddings to all layers
        for layer in 0..NUM_LAYERS {
            let layer_offset = layer * embedding_size;
            let layer_key = &key_embeddings[layer_offset..layer_offset + embedding_size];
            let layer_value = &value_embeddings[layer_offset..layer_offset + embedding_size];

            self.key_caches[layer].write_at(write_pos, layer_key);
            self.value_caches[layer].write_at(write_pos, layer_value);
        }

        // Update lengths
        let new_length = if physical_length >= self.max_context {
            // After shift + write, we're back at max_context
            self.max_context
        } else {
            // Growing the cache
            physical_length + 1
        };

        for layer in 0..NUM_LAYERS {
            self.key_caches[layer].current_length = new_length;
            self.value_caches[layer].current_length = new_length;
        }

        self.logical_length += 1;
        write_pos
    }

    /// Extend the cache with multiple tokens (for prefill phase)
    ///
    /// # Arguments
    /// * `key_embeddings` - Key embeddings for all tokens and layers
    ///   Shape: [num_tokens, NUM_LAYERS, NUM_KV_HEADS, HEAD_DIM] flattened
    /// * `value_embeddings` - Value embeddings for all tokens and layers
    ///   Shape: [num_tokens, NUM_LAYERS, NUM_KV_HEADS, HEAD_DIM] flattened
    /// * `num_tokens` - Number of tokens to add
    pub fn extend(&mut self, key_embeddings: &[f32], value_embeddings: &[f32], num_tokens: usize) {
        let token_size = NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM;

        debug_assert_eq!(
            key_embeddings.len(),
            num_tokens * token_size,
            "Key embeddings size mismatch for extend"
        );

        for token_idx in 0..num_tokens {
            let token_offset = token_idx * token_size;
            let token_key = &key_embeddings[token_offset..token_offset + token_size];
            let token_value = &value_embeddings[token_offset..token_offset + token_size];
            self.append(token_key, token_value);
        }
    }

    /// Shift all layer caches left, preserving sink tokens
    fn shift_all_caches(&mut self) {
        for layer in 0..NUM_LAYERS {
            self.key_caches[layer].shift_left(self.sink_size, 1);
            self.value_caches[layer].shift_left(self.sink_size, 1);
        }
    }

    /// Compute position IDs for new tokens based on logical sequence position.
    ///
    /// NOTE: Intentionally unused in production inference. The StreamingLLM approach
    /// relies on the ONNX model deriving positions internally from cache length,
    /// which gives correct relative distances for RoPE after Attention Sinks shifting.
    /// Wiring this into the model would produce INCORRECT results after a cache shift.
    /// Kept for testing and debugging purposes.
    ///
    /// # Arguments
    /// * `num_new_tokens` - Number of new tokens being processed (1 for decode, N for prefill)
    ///
    /// # Returns
    /// Position IDs array with shape matching the new tokens
    ///
    /// # Position ID Logic
    /// - During prefill (cache empty): [0, 1, 2, ..., num_new_tokens - 1]
    /// - During decode (cache has data): [logical_length] (single position)
    /// - With Attention Sinks (cache full): continues logical counting
    #[cfg(test)]
    pub fn get_position_ids(&self, num_new_tokens: usize) -> Vec<i64> {
        if self.logical_length == 0 {
            // Prefill: positions are 0..num_new_tokens
            (0..num_new_tokens as i64).collect()
        } else {
            // Decode: position is the logical length (next position)
            (self.logical_length as i64..self.logical_length as i64 + num_new_tokens as i64)
                .collect()
        }
    }

    /// Get key cache for a specific layer as ONNX-compatible array
    ///
    /// Returns shape: [1, NUM_KV_HEADS, current_length, HEAD_DIM]
    pub fn get_key_array(&self, layer: usize) -> Array4<f32> {
        self.key_caches[layer].to_array()
    }

    /// Get value cache for a specific layer as ONNX-compatible array
    ///
    /// Returns shape: [1, NUM_KV_HEADS, current_length, HEAD_DIM]
    pub fn get_value_array(&self, layer: usize) -> Array4<f32> {
        self.value_caches[layer].to_array()
    }

    /// Get the physical length of the cache (tokens currently stored)
    #[inline]
    pub fn physical_length(&self) -> usize {
        if self.key_caches.is_empty() {
            0
        } else {
            self.key_caches[0].len()
        }
    }

    /// Get the logical length (total tokens ever seen)
    #[inline]
    #[cfg(test)]
    pub fn logical_length(&self) -> usize {
        self.logical_length
    }

    /// Check if the cache has any data
    #[inline]
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.physical_length() == 0
    }

    /// Check if the cache is at maximum capacity
    #[inline]
    #[cfg(test)]
    pub fn is_full(&self) -> bool {
        self.physical_length() >= self.max_context
    }

    /// Clear the cache, resetting all state
    #[cfg(test)]
    pub fn clear(&mut self) {
        for cache in &mut self.key_caches {
            cache.clear();
        }
        for cache in &mut self.value_caches {
            cache.clear();
        }
        self.logical_length = 0;
    }

    /// Get the maximum context size
    #[inline]
    pub fn max_context(&self) -> usize {
        self.max_context
    }

    /// Get the sink size
    #[inline]
    #[cfg(test)]
    pub fn sink_size(&self) -> usize {
        self.sink_size
    }

    /// Estimate memory usage in bytes
    #[cfg(test)]
    pub fn memory_usage_bytes(&self) -> usize {
        let per_layer = 2 * NUM_KV_HEADS * self.max_context * HEAD_DIM * std::mem::size_of::<f32>();
        NUM_LAYERS * per_layer
    }
}

/// Fixed-window KV cache for DirectML exported models.
///
/// This cache keeps full-size layer tensors resident at
/// `[1, NUM_KV_HEADS, max_sequence_length, HEAD_DIM]` and only mutates
/// `valid_length` plus in-place shifts when the window is full.
#[derive(Debug)]
pub struct DmlKvCache {
    key_caches: Vec<Array4<f32>>,
    value_caches: Vec<Array4<f32>>,
    max_sequence_length: usize,
    sink_size: usize,
    valid_length: usize,
}

impl DmlKvCache {
    pub fn new(max_sequence_length: usize, sink_size: usize) -> Self {
        assert!(
            max_sequence_length > 0,
            "max_sequence_length must be positive"
        );
        assert!(
            sink_size < max_sequence_length,
            "sink_size must be less than max_sequence_length"
        );

        let key_caches = (0..NUM_LAYERS)
            .map(|_| Array4::<f32>::zeros((1, NUM_KV_HEADS, max_sequence_length, HEAD_DIM)))
            .collect();
        let value_caches = (0..NUM_LAYERS)
            .map(|_| Array4::<f32>::zeros((1, NUM_KV_HEADS, max_sequence_length, HEAD_DIM)))
            .collect();

        Self {
            key_caches,
            value_caches,
            max_sequence_length,
            sink_size,
            valid_length: 0,
        }
    }

    fn shift_single_layer_left(
        layer: &mut Array4<f32>,
        current_length: usize,
        sink_size: usize,
        max_sequence_length: usize,
    ) {
        if current_length <= sink_size + 1 {
            return;
        }

        let Some(data) = layer.as_slice_mut() else {
            return;
        };

        for head in 0..NUM_KV_HEADS {
            let head_stride = max_sequence_length * HEAD_DIM;
            let src_start = head * head_stride + (sink_size + 1) * HEAD_DIM;
            let src_end = head * head_stride + current_length * HEAD_DIM;
            let dst_start = head * head_stride + sink_size * HEAD_DIM;
            data.copy_within(src_start..src_end, dst_start);
        }
    }

    fn shift_all_layers_left(&mut self) {
        let current_length = self.valid_length;
        for layer in 0..NUM_LAYERS {
            Self::shift_single_layer_left(
                &mut self.key_caches[layer],
                current_length,
                self.sink_size,
                self.max_sequence_length,
            );
            Self::shift_single_layer_left(
                &mut self.value_caches[layer],
                current_length,
                self.sink_size,
                self.max_sequence_length,
            );
        }
        self.valid_length = self.valid_length.saturating_sub(1);
    }

    /// Prepares the cache for a decode step.
    ///
    /// If the buffer is full, shifts the non-sink window left by one so the next token
    /// has space to be written by the DirectML model.
    pub fn prepare_decode_step(&mut self) {
        if self.valid_length >= self.max_sequence_length {
            self.shift_all_layers_left();
        }
    }

    /// Marks one newly generated token as valid after a decode step completes.
    #[cfg(test)]
    pub fn complete_decode_step(&mut self) -> Result<(), String> {
        if self.valid_length >= self.max_sequence_length {
            return Err(format!(
                "DML KV cache overflow: valid_length={} max_sequence_length={}",
                self.valid_length, self.max_sequence_length
            ));
        }
        self.valid_length += 1;
        Ok(())
    }

    /// Marks prefill completion for `prompt_length` tokens.
    pub fn complete_prefill(&mut self, prompt_length: usize) -> Result<(), String> {
        if prompt_length > self.max_sequence_length {
            return Err(format!(
                "Prompt length {} exceeds DirectML max_sequence_length {}",
                prompt_length, self.max_sequence_length
            ));
        }
        self.valid_length = prompt_length;
        Ok(())
    }

    #[inline]
    pub fn valid_length(&self) -> usize {
        self.valid_length
    }

    #[inline]
    pub fn max_sequence_length(&self) -> usize {
        self.max_sequence_length
    }

    #[inline]
    pub fn key_array(&self, layer: usize) -> &Array4<f32> {
        &self.key_caches[layer]
    }

    #[inline]
    pub fn key_array_mut(&mut self, layer: usize) -> &mut Array4<f32> {
        &mut self.key_caches[layer]
    }

    #[inline]
    pub fn value_array(&self, layer: usize) -> &Array4<f32> {
        &self.value_caches[layer]
    }

    #[inline]
    pub fn value_array_mut(&mut self, layer: usize) -> &mut Array4<f32> {
        &mut self.value_caches[layer]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_cache_basic() {
        let cache = LayerCache::new(10);
        assert!(cache.is_empty());
        assert!(!cache.is_full());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_layer_cache_write() {
        let mut cache = LayerCache::new(10);
        let embedding = vec![1.0f32; NUM_KV_HEADS * HEAD_DIM];

        cache.write_at(0, &embedding);
        cache.current_length = 1;

        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_layer_cache_shift() {
        let mut cache = LayerCache::new(10);

        // Write 6 tokens (sink_size=4, so positions 0-3 are sinks, 4-5 are window)
        for pos in 0..6 {
            let embedding: Vec<f32> = (0..NUM_KV_HEADS * HEAD_DIM)
                .map(|i| (pos * 1000 + i) as f32)
                .collect();
            cache.write_at(pos, &embedding);
        }
        cache.current_length = 6;

        // Shift left by 1, keeping first 4 as sinks
        cache.shift_left(4, 1);

        assert_eq!(cache.len(), 5);

        // Verify sink tokens are unchanged (positions 0-3)
        for pos in 0..4 {
            let offset = cache.offset(0, pos);
            assert_eq!(cache.data[offset], (pos * 1000) as f32);
        }

        // Verify window shifted: old position 5 is now at position 4
        let offset = cache.offset(0, 4);
        assert_eq!(cache.data[offset], (5 * 1000) as f32);
    }

    #[test]
    fn test_kv_cache_new() {
        let cache = KVCache::new(4096, 4);

        assert_eq!(cache.max_context(), 4096);
        assert_eq!(cache.sink_size(), 4);
        assert_eq!(cache.physical_length(), 0);
        assert_eq!(cache.logical_length(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_kv_cache_append() {
        let mut cache = KVCache::new(10, 4);

        let key_emb = vec![1.0f32; NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM];
        let val_emb = vec![2.0f32; NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM];

        let pos = cache.append(&key_emb, &val_emb);

        assert_eq!(pos, 0);
        assert_eq!(cache.physical_length(), 1);
        assert_eq!(cache.logical_length(), 1);
    }

    #[test]
    fn test_kv_cache_attention_sinks() {
        let mut cache = KVCache::new(6, 2); // Small cache for testing

        // Fill the cache completely
        for i in 0..6 {
            let key_emb: Vec<f32> = (0..NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM)
                .map(|j| (i * 1000 + j) as f32)
                .collect();
            let val_emb = key_emb.clone();
            cache.append(&key_emb, &val_emb);
        }

        assert!(cache.is_full());
        assert_eq!(cache.physical_length(), 6);
        assert_eq!(cache.logical_length(), 6);

        // Append one more - should trigger shift
        let key_emb: Vec<f32> = (0..NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM)
            .map(|j| (6 * 1000 + j) as f32)
            .collect();
        let val_emb = key_emb.clone();
        let pos = cache.append(&key_emb, &val_emb);

        // Should write at position 5 (last position)
        assert_eq!(pos, 5);
        // Physical length stays at 6
        assert_eq!(cache.physical_length(), 6);
        // Logical length increases
        assert_eq!(cache.logical_length(), 7);

        // Verify layer 0, key cache, head 0:
        // - Positions 0-1 should be unchanged (sinks)
        // - Position 2 should have old position 3's data
        // - Position 5 should have new data
        let key_arr = cache.get_key_array(0);

        // Sink token 0 should be unchanged
        assert_eq!(key_arr[[0, 0, 0, 0]], 0.0);
        // Sink token 1 should be unchanged
        assert_eq!(key_arr[[0, 0, 1, 0]], 1000.0);
        // New token at position 5
        assert_eq!(key_arr[[0, 0, 5, 0]], 6000.0);
    }

    #[test]
    fn test_position_ids_prefill() {
        let cache = KVCache::new(4096, 4);
        let pos_ids = cache.get_position_ids(5);
        assert_eq!(pos_ids, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_position_ids_decode() {
        let mut cache = KVCache::new(4096, 4);

        // Simulate 10 tokens already in cache
        for _ in 0..10 {
            let key_emb = vec![1.0f32; NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM];
            let val_emb = vec![2.0f32; NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM];
            cache.append(&key_emb, &val_emb);
        }

        // Next token should be at position 10
        let pos_ids = cache.get_position_ids(1);
        assert_eq!(pos_ids, vec![10]);
    }

    #[test]
    fn test_position_ids_with_sinks() {
        let mut cache = KVCache::new(6, 2);

        // Fill completely + 4 more tokens (triggers 4 shifts)
        for _ in 0..10 {
            let key_emb = vec![1.0f32; NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM];
            let val_emb = vec![2.0f32; NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM];
            cache.append(&key_emb, &val_emb);
        }

        // Physical length is 6, but logical is 10
        assert_eq!(cache.physical_length(), 6);
        assert_eq!(cache.logical_length(), 10);

        // Next position should be 10
        let pos_ids = cache.get_position_ids(1);
        assert_eq!(pos_ids, vec![10]);
    }

    #[test]
    fn test_memory_usage() {
        let cache = KVCache::new(4096, 4);
        let expected = 2 * NUM_LAYERS * NUM_KV_HEADS * 4096 * HEAD_DIM * 4;
        assert_eq!(cache.memory_usage_bytes(), expected);
        // For 28 layers, 2 heads, 4096 context, 128 dim: ~234 MB
        println!(
            "KV Cache memory: {} MB",
            cache.memory_usage_bytes() / (1024 * 1024)
        );
    }

    #[test]
    fn test_to_array_shape() {
        let mut cache = KVCache::new(100, 4);

        // Add 5 tokens
        for _ in 0..5 {
            let key_emb = vec![1.0f32; NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM];
            let val_emb = vec![2.0f32; NUM_LAYERS * NUM_KV_HEADS * HEAD_DIM];
            cache.append(&key_emb, &val_emb);
        }

        let arr = cache.get_key_array(0);
        assert_eq!(arr.shape(), &[1, NUM_KV_HEADS, 5, HEAD_DIM]);
    }

    #[test]
    fn test_dml_kv_cache_prefill_and_decode_lengths() {
        let mut cache = DmlKvCache::new(8, 2);
        cache.complete_prefill(5).expect("prefill should fit");
        assert_eq!(cache.valid_length(), 5);

        cache.prepare_decode_step();
        cache.complete_decode_step().expect("decode should fit");
        assert_eq!(cache.valid_length(), 6);

        let overflow = cache
            .complete_prefill(9)
            .expect_err("prefill overflow should fail");
        assert!(overflow.contains("exceeds"));
    }

    #[test]
    fn test_dml_kv_cache_shift_preserves_sink_tokens() {
        let mut cache = DmlKvCache::new(6, 2);

        // Fill one layer/head with position markers in dim 0.
        for pos in 0..6 {
            cache.key_caches[0][[0, 0, pos, 0]] = pos as f32;
            cache.value_caches[0][[0, 0, pos, 0]] = (100 + pos) as f32;
        }
        cache.complete_prefill(6).expect("prefill should fit");

        cache.prepare_decode_step();

        // Sink tokens remain at positions 0..sink_size-1.
        assert_eq!(cache.key_caches[0][[0, 0, 0, 0]], 0.0);
        assert_eq!(cache.key_caches[0][[0, 0, 1, 0]], 1.0);
        // Sliding window shifted left by one: old pos 3 -> new pos 2.
        assert_eq!(cache.key_caches[0][[0, 0, 2, 0]], 3.0);
        assert_eq!(cache.value_caches[0][[0, 0, 2, 0]], 103.0);
        assert_eq!(cache.valid_length(), 5);

        cache
            .complete_decode_step()
            .expect("decode completion should restore full length");
        assert_eq!(cache.valid_length(), 6);
    }
}
