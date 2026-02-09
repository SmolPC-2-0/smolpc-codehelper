/**
 * TypeScript types for ONNX Runtime inference
 * Must match Rust types in src-tauri/src/inference/types.rs
 */

/**
 * Performance metrics for text generation
 */
export interface GenerationMetrics {
	/** Total tokens generated (excluding prompt) */
	total_tokens: number;

	/** Time to generate first token (milliseconds) */
	time_to_first_token_ms: number | null;

	/** Average tokens per second */
	tokens_per_second: number;

	/** Total generation time (milliseconds) */
	total_time_ms: number;
}

/**
 * Result of text generation
 */
export interface GenerationResult {
	/** Generated text */
	text: string;

	/** Performance metrics */
	metrics: GenerationMetrics;
}

/**
 * Model metadata
 */
export interface ModelInfo {
	/** Model file name */
	name: string;

	/** Model input names */
	inputs: string[];

	/** Model output names */
	outputs: string[];
}

/**
 * Available model for inference
 */
export interface AvailableModel {
	/** Display name */
	name: string;

	/** Model ID (directory name) */
	id: string;

	/** Path to model files */
	path: string;

	/** Size category (e.g., "1.5B", "0.5B") */
	size: string;
}

/**
 * Generation configuration
 */
export interface GenerationConfig {
	/** Maximum tokens to generate */
	max_length: number;

	/** Temperature for sampling (0 = greedy, higher = more random) */
	temperature: number;

	/** Top-k filtering (only consider top k tokens) */
	top_k: number | null;

	/** Top-p (nucleus) sampling threshold */
	top_p: number | null;

	/** Repetition penalty (1.0 = disabled, >1.0 = penalize repeats) */
	repetition_penalty: number;

	/** Number of recent tokens to consider for repetition penalty (0 = all generated tokens) */
	repetition_penalty_last_n: number;
}

/**
 * Current inference state
 */
export interface InferenceStatus {
	/** Whether a model is currently loaded */
	isLoaded: boolean;

	/** Name of the currently loaded model */
	currentModel: string | null;

	/** Whether generation is in progress */
	isGenerating: boolean;

	/** Error message if any */
	error: string | null;
}
