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
	active_backend: InferenceBackend | null;
	active_model_id: string | null;
	error_code: string | null;
	error_message: string | null;
	retryable: boolean;
}

export type InferenceBackend = 'cpu' | 'directml' | 'openvino_npu';
export type BackendSelectionState = 'pending' | 'ready' | 'fallback' | 'error';
export type DecisionPersistenceState = 'none' | 'persisted' | 'temporary_fallback';
export type LaneStartupProbeState = 'not_started' | 'ready' | 'error';
export type LanePreflightState = 'not_started' | 'pending' | 'ready' | 'timeout' | 'error';
export type LaneCacheState = 'unknown' | 'cold' | 'warm';

export interface BackendSelectedDevice {
	backend: InferenceBackend;
	device_id: number | null;
	device_name: string | null;
}

export interface BackendRuntimeBundleStatus {
	root: string | null;
	fingerprint: string | null;
	validated: boolean;
	failure: string | null;
}

export interface BackendRuntimeBundlesStatus {
	load_mode: string | null;
	ort: BackendRuntimeBundleStatus;
	directml: BackendRuntimeBundleStatus;
	openvino: BackendRuntimeBundleStatus;
}

export interface BackendLaneStatus {
	detected: boolean;
	bundle_ready: boolean;
	artifact_ready: boolean;
	startup_probe_state: LaneStartupProbeState;
	preflight_state: LanePreflightState;
	persisted_eligibility: boolean;
	last_failure_class: string | null;
	last_failure_message: string | null;
	driver_version: string | null;
	runtime_version: string | null;
	cache_state: LaneCacheState;
	device_id: number | null;
	device_name: string | null;
}

export interface BackendDecisionKey {
	model_id: string;
	model_artifact_fingerprint: string | null;
	app_version: string;
	selector_engine_id: string;
	ort_runtime_version: string | null;
	ort_bundle_fingerprint: string | null;
	openvino_runtime_version: string | null;
	openvino_genai_version: string | null;
	openvino_tokenizers_version: string | null;
	openvino_bundle_fingerprint: string | null;
	gpu_adapter_identity: string | null;
	gpu_driver_version: string | null;
	gpu_device_id: number | null;
	npu_adapter_identity: string | null;
	npu_driver_version: string | null;
	openvino_npu_max_prompt_len: number | null;
	openvino_npu_min_response_len: number | null;
	openvino_npu_prefill_hint: string | null;
	openvino_npu_generate_hint: string | null;
	openvino_npu_prefill_chunk_size: number | null;
	openvino_message_mode: string | null;
	selection_profile: string | null;
}

export interface BackendOpenVinoTuningStatus {
	max_prompt_len: number | null;
	min_response_len: number | null;
	prefill_hint: string | null;
	generate_hint: string | null;
	prefill_chunk_size: number | null;
}

export interface FailureCounters {
	directml_init_failures: number;
	directml_runtime_failures: number;
	directml_total_failures: number;
	directml_consecutive_failures: number;
	demotions: number;
	last_failure_stage: string | null;
	last_failure_reason: string | null;
	last_failure_at: string | null;
}

export interface BackendStatus {
	/** Active backend identifier */
	active_backend: InferenceBackend | null;

	/** Resolved active model path on disk */
	active_model_path: string | null;

	/** Active artifact backend identifier */
	active_artifact_backend: InferenceBackend | null;

	/** Runtime implementation in use (e.g. "ov_genai_cpu", "genai_dml") */
	runtime_engine: string | null;

	/** Available backend identifiers on current machine */
	available_backends: InferenceBackend[];

	/** Selected device details when a lane exposes one */
	selected_device: BackendSelectedDevice | null;

	/** Selection lifecycle state ("pending", "ready", "fallback", "error") */
	selection_state: BackendSelectionState | null;

	/** Selection reason code from host */
	selection_reason: string | null;

	/** Whether the current active backend is persisted or a temporary fallback */
	decision_persistence_state: DecisionPersistenceState;

	/** Opaque selection fingerprint for the active model load */
	selection_fingerprint: string | null;

	/** Full decision key used for persistence */
	decision_key: BackendDecisionKey | null;

	/** Runtime bundle validation grouped by runtime family */
	runtime_bundles: BackendRuntimeBundlesStatus;

	/** Lane-based readiness and failure state */
	lanes: {
		openvino_npu: BackendLaneStatus;
		directml: BackendLaneStatus;
		cpu: BackendLaneStatus;
	};

	/** Active OpenVINO chat path mode (structured vs legacy prompt compatibility) */
	openvino_message_mode: string | null;

	/** Active OpenVINO NPU tuning values from backend env/config */
	openvino_tuning: BackendOpenVinoTuningStatus | null;

	/** Failure counters tracked by the host */
	failure_counters: FailureCounters;

	/** Force override mode applied by host policy when present */
	force_override: InferenceBackend | null;
}

export interface ModelLaneReadiness {
	artifact_ready: boolean;
	bundle_ready: boolean;
	ready: boolean;
	reason: string;
}

export interface CheckModelResponse {
	model_id: string;
	lanes: {
		openvino_npu: ModelLaneReadiness;
		directml: ModelLaneReadiness;
		cpu: ModelLaneReadiness;
	};
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

export interface InferenceChatMessage {
	role: 'system' | 'user' | 'assistant';
	content: string;
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

	/** Active backend identifier */
	activeBackend: InferenceBackend | null;

	/** Active artifact backend identifier */
	activeArtifactBackend: InferenceBackend | null;

	/** Runtime implementation in use (e.g. "ov_genai_cpu", "genai_dml") */
	runtimeEngine: string | null;

	/** Resolved active model path on disk */
	activeModelPath: string | null;

	/** Selection lifecycle state ("pending", "ready", "fallback", "error") */
	selectionState: BackendSelectionState | null;

	/** Selection reason code from host */
	selectionReason: string | null;

	/** Whether the active backend is persisted or a temporary fallback */
	decisionPersistenceState: DecisionPersistenceState | null;

	/** Selected device name when applicable */
	selectedDeviceName: string | null;

	/** Runtime mode preference used by host policy */
	runtimeMode: InferenceRuntimeMode;

	/** Current DirectML lane preflight state */
	directmlPreflightState: LanePreflightState | null;

	/** Current DirectML lane failure class when known */
	directmlFailureClass: string | null;
}
