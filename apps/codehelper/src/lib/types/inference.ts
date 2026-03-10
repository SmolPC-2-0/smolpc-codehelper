/**
 * TypeScript types for shared-engine inference integration.
 * Must match Tauri command DTOs and shared engine contract fields.
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
 * Active inference backend state exposed by the shared engine host.
 */
export type InferenceRuntimeMode = 'auto' | 'cpu' | 'dml';
export type StartupModeDto = 'auto' | 'directml_required';
export type EngineReadinessState =
	| 'idle'
	| 'starting'
	| 'probing'
	| 'resolving_assets'
	| 'loading_model'
	| 'ready'
	| 'failed';

export interface StartupPolicyDto {
	default_model_id?: string | null;
}

export interface EnsureStartedRequestDto {
	mode: StartupModeDto;
	startup_policy?: StartupPolicyDto | null;
}

export interface EngineReadinessDto {
	attempt_id: string;
	state: EngineReadinessState;
	state_since: string;
	active_backend: string | null;
	active_model_id: string | null;
	error_code: string | null;
	error_message: string | null;
	retryable: boolean;
}

export interface BackendStatus {
	/** Active backend identifier ("cpu" or "directml") */
	active_backend: string | null;

	/** Resolved active model path on disk */
	active_model_path: string | null;

	/** Runtime implementation in use (e.g. "ort_cpu", "genai_dml") */
	runtime_engine: string | null;

	/** Available backend identifiers on current machine */
	available_backends: string[];

	/** Selection lifecycle state ("pending", "ready", "fallback", "error") */
	selection_state: string | null;

	/** Selection reason code from host */
	selection_reason: string | null;

	/** Selected DirectML device id when applicable */
	selected_device_id: number | null;

	/** Selected DirectML device name when applicable */
	selected_device_name: string | null;

	/** Runtime gate state for DML policy visibility */
	dml_gate_state?: string | null;

	/** Runtime gate reason for DML policy visibility */
	dml_gate_reason?: string | null;

	/** Force override mode applied by host policy when present */
	force_override?: string | null;
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
	/** Readiness payload from engine contract adapter */
	readiness: EngineReadinessDto | null;

	/** Readiness state used for startup/inference gating */
	readinessState: EngineReadinessState | 'unknown';

	/** Whether engine reports readiness state as ready */
	isReady: boolean;

	/** Whether a model is currently loaded */
	isLoaded: boolean;

	/** Name of the currently loaded model */
	currentModel: string | null;

	/** Whether generation is in progress */
	isGenerating: boolean;

	/** Error message if any */
	error: string | null;

	/** Startup failure code from readiness payload */
	startupErrorCode: string | null;

	/** Startup failure message from readiness payload */
	startupErrorMessage: string | null;

	/** Whether startup failure is retryable */
	startupRetryable: boolean;

	/** Active backend identifier ("cpu" or "directml") */
	activeBackend: string | null;

	/** Runtime implementation in use (e.g. "ort_cpu", "genai_dml") */
	runtimeEngine: string | null;

	/** Resolved active model path on disk */
	activeModelPath: string | null;

	/** Selection lifecycle state ("pending", "ready", "fallback", "error") */
	selectionState: string | null;

	/** Selection reason code from host */
	selectionReason: string | null;

	/** Selected DirectML device name when applicable */
	selectedDeviceName: string | null;

	/** Runtime mode preference used by host policy */
	runtimeMode: InferenceRuntimeMode;

	/** Runtime gate state for DML policy visibility */
	dmlGateState: string | null;

	/** Runtime gate reason for DML policy visibility */
	dmlGateReason: string | null;
}
