/**
 * Inference store for shared engine startup/readiness and text generation.
 */
import { invoke, Channel } from '@tauri-apps/api/core';
import type {
	AvailableModel,
	BackendStatus,
	CheckModelResponse,
	EngineReadinessDto,
	EngineReadinessState,
	EnsureStartedRequestDto,
	GenerationConfig,
	GenerationMetrics,
	InferenceCancelState,
	InferenceChatMessage,
	InferenceBackend,
	MemoryPressureRequest,
	MemoryPressureStatus,
	InferenceRuntimeMode,
	InferenceStatus,
	StartupModeDto
} from '$lib/types/inference';

const READINESS_STATES: ReadonlySet<string> = new Set([
	'idle',
	'starting',
	'probing',
	'resolving_assets',
	'loading_model',
	'ready',
	'failed'
]);

// State
let readiness = $state<EngineReadinessDto | null>(null);
let isGenerating = $state(false);
let cancelState = $state<InferenceCancelState>('idle');
let error = $state<string | null>(null);
let availableModels = $state<AvailableModel[]>([]);
let lastMetrics = $state<GenerationMetrics | null>(null);
let backendStatus = $state<BackendStatus | null>(null);
let runtimeMode = $state<InferenceRuntimeMode>('auto');
let engineHealthy = $state(true);
let memoryPressure = $state<MemoryPressureStatus | null>(null);
let cancelTimeoutId: ReturnType<typeof setTimeout> | null = null;
let cancelTimeoutSessionId: number | null = null;
let generationSessionCounter = 0;
let activeGenerationSessionId = 0;

function clearCancelTimeout(sessionId?: number): void {
	if (sessionId !== undefined && cancelTimeoutSessionId !== sessionId) {
		return;
	}

	if (cancelTimeoutId) {
		clearTimeout(cancelTimeoutId);
	}

	cancelTimeoutId = null;
	cancelTimeoutSessionId = null;
}

function beginGenerationSession(): number {
	clearCancelTimeout();
	const sessionId = ++generationSessionCounter;
	activeGenerationSessionId = sessionId;
	isGenerating = true;
	cancelState = 'idle';
	error = null;
	lastMetrics = null;
	return sessionId;
}

function isActiveGenerationSession(sessionId: number): boolean {
	return activeGenerationSessionId === sessionId;
}

function normalizeBackendName(raw: string | null | undefined): InferenceBackend | null {
	if (!raw) {
		return null;
	}
	const normalized = raw.toLowerCase().replaceAll('_', '');
	if (normalized === 'directml') {
		return 'directml';
	}
	if (normalized === 'cpu') {
		return 'cpu';
	}
	if (normalized === 'openvinonpu') {
		return 'openvino_npu';
	}
	return null;
}

function normalizeRuntimeMode(raw: string | null | undefined): InferenceRuntimeMode {
	if (!raw) {
		return 'auto';
	}
	const normalized = raw.toLowerCase().replaceAll('_', '');
	if (normalized === 'directml' || normalized === 'dml') {
		return 'dml';
	}
	if (normalized === 'cpu') {
		return 'cpu';
	}
	if (normalized === 'openvinonpu' || normalized === 'openvino' || normalized === 'npu') {
		return 'npu';
	}
	return 'auto';
}

function normalizeReadinessState(raw: string | null | undefined): EngineReadinessState {
	const normalized = raw?.trim().toLowerCase() ?? '';
	if (READINESS_STATES.has(normalized)) {
		return normalized as EngineReadinessState;
	}
	return 'failed';
}

function normalizeReadiness(dto: EngineReadinessDto): EngineReadinessDto {
	return {
		attempt_id: dto.attempt_id,
		state: normalizeReadinessState(dto.state),
		state_since: dto.state_since,
		active_backend: normalizeBackendName(dto.active_backend),
		active_model_id: dto.active_model_id ?? null,
		error_code: dto.error_code ?? null,
		error_message: dto.error_message ?? null,
		retryable: Boolean(dto.retryable)
	};
}

function buildEnsureStartedRequest(
	mode: StartupModeDto,
	defaultModelId: string | null = null
): EnsureStartedRequestDto {
	const modelId = defaultModelId?.trim();
	return {
		mode,
		startup_policy: modelId ? { default_model_id: modelId } : null
	};
}

function defaultStartupRequest(defaultModelId: string | null = null): EnsureStartedRequestDto {
	return buildEnsureStartedRequest('auto', defaultModelId);
}

function isReadyState(value: EngineReadinessDto | null): boolean {
	return value?.state === 'ready';
}

export const inferenceStore = {
	// Getters
	get readiness() {
		return readiness;
	},
	get isReady() {
		return isReadyState(readiness);
	},
	get isLoaded() {
		return isReadyState(readiness);
	},
	get currentModel() {
		return readiness?.active_model_id ?? null;
	},
	get isGenerating() {
		return isGenerating;
	},
	get cancelState() {
		return cancelState;
	},
	get error() {
		return error;
	},
	get availableModels() {
		return availableModels;
	},
	get lastMetrics() {
		return lastMetrics;
	},
	get runtimeMode() {
		return runtimeMode;
	},
	get backendStatus() {
		return backendStatus;
	},
	get engineHealthy() {
		return engineHealthy;
	},
	get memoryPressure() {
		return memoryPressure;
	},

	// Get status object for display
	get status(): InferenceStatus {
		return {
			readiness,
			readinessState: readiness?.state ?? 'unknown',
			isReady: isReadyState(readiness),
			isLoaded: isReadyState(readiness),
			currentModel: readiness?.active_model_id ?? null,
			isGenerating,
			error,
			startupErrorCode: readiness?.error_code ?? null,
			startupErrorMessage: readiness?.error_message ?? null,
			startupRetryable: readiness?.retryable ?? false,
			activeBackend:
				normalizeBackendName(readiness?.active_backend) ??
				normalizeBackendName(backendStatus?.active_backend),
			activeArtifactBackend: normalizeBackendName(backendStatus?.active_artifact_backend),
			runtimeEngine: backendStatus?.runtime_engine ?? null,
			activeModelPath: backendStatus?.active_model_path ?? null,
			selectionState: backendStatus?.selection_state ?? null,
			selectionReason: backendStatus?.selection_reason ?? null,
			decisionPersistenceState: backendStatus?.decision_persistence_state ?? null,
			selectedDeviceName: backendStatus?.selected_device?.device_name ?? null,
			runtimeMode,
			directmlPreflightState: backendStatus?.lanes.directml.preflight_state ?? null,
			directmlFailureClass: backendStatus?.lanes.directml.last_failure_class ?? null
		};
	},

	// Actions

	/**
	 * List available models from engine registry.
	 */
	async listModels(): Promise<void> {
		try {
			const models = await invoke<AvailableModel[]>('list_models');
			availableModels = models;
		} catch (e) {
			error = String(e);
			console.error('Failed to list models:', e);
		}
	},

	/**
	 * Blocking startup handshake from app perspective.
	 */
	async ensureStarted(
		request: EnsureStartedRequestDto = defaultStartupRequest()
	): Promise<EngineReadinessDto | null> {
		// If we're reconnecting after the engine became unhealthy (e.g. it crashed
		// during generation and was auto-restarted), force-clear stale generation state.
		if (isGenerating || cancelState !== 'idle') {
			console.warn('ensureStarted: clearing stale generation state from prior session');
			clearCancelTimeout();
			isGenerating = false;
			cancelState = 'idle';
			activeGenerationSessionId = 0;
		}
		error = null;
		try {
			const payload = await invoke<EngineReadinessDto>('engine_ensure_started', { request });
			readiness = normalizeReadiness(payload);
			if (readiness.state === 'failed') {
				error = readiness.error_message ?? 'Engine startup failed';
			}
			await this.refreshBackendStatus();
			return readiness;
		} catch (e) {
			error = String(e);
			console.error('Engine startup handshake failed:', e);
			return null;
		}
	},

	/**
	 * Poll readiness status from engine adapter.
	 */
	async refreshReadiness(): Promise<void> {
		try {
			const payload = await invoke<EngineReadinessDto>('engine_status');
			readiness = normalizeReadiness(payload);
			if (readiness.state !== 'failed') {
				return;
			}
			error = readiness.error_message ?? error;
		} catch (e) {
			console.warn('Failed to refresh readiness status:', e);
		}
	},

	/**
	 * Backward-compatible status sync wrapper.
	 */
	async syncStatus(): Promise<void> {
		await this.refreshReadiness();
		await this.refreshBackendStatus();
	},

	/**
	 * Lightweight health check — updates engineHealthy state.
	 */
	async checkHealth(): Promise<boolean> {
		try {
			await invoke('engine_status');
			engineHealthy = true;
			return true;
		} catch {
			engineHealthy = false;
			return false;
		}
	},

	async evaluateMemoryPressure(
		request: MemoryPressureRequest = {}
	): Promise<MemoryPressureStatus | null> {
		try {
			const payload = await invoke<MemoryPressureStatus>('evaluate_memory_pressure', {
				request: {
					activeMode: request.activeMode ?? null,
					appMinimized: Boolean(request.appMinimized)
				}
			});
			memoryPressure = payload;
			return payload;
		} catch (e) {
			console.warn('Failed to evaluate memory pressure:', e);
			return null;
		}
	},

	/**
	 * Load a model by ID.
	 */
	async loadModel(modelId: string): Promise<boolean> {
		error = null;
		try {
			await invoke('load_model', { modelId });
			await this.syncStatus();
			return this.isReady;
		} catch (e) {
			error = String(e);
			console.error('Failed to load model:', e);
			await this.syncStatus();
			return false;
		}
	},

	/**
	 * Unload the current model.
	 */
	async unloadModel(): Promise<void> {
		try {
			await invoke('unload_model');
			await this.syncStatus();
		} catch (e) {
			error = String(e);
			console.error('Failed to unload model:', e);
		}
	},

	/**
	 * Generate text with streaming output via Tauri Channel.
	 */
	async generateStream(
		prompt: string,
		onToken: (token: string) => void,
		config?: Partial<GenerationConfig>
	): Promise<GenerationMetrics | null> {
		if (!this.isReady) {
			error = 'Engine is not ready';
			return null;
		}

		if (isGenerating) {
			error = 'Generation already in progress';
			return null;
		}

		const sessionId = beginGenerationSession();

		try {
			const onTokenChannel = new Channel<string>();
			onTokenChannel.onmessage = onToken;

			const fullConfig: GenerationConfig | undefined = config
				? {
						max_length: config.max_length ?? 2048,
						temperature: config.temperature ?? 0.7,
						top_k: config.top_k ?? 40,
						top_p: config.top_p ?? 0.9,
						repetition_penalty: config.repetition_penalty ?? 1.1,
						repetition_penalty_last_n: config.repetition_penalty_last_n ?? 64
					}
				: undefined;

			const metrics = await invoke<GenerationMetrics>('inference_generate', {
				prompt,
				config: fullConfig,
				onToken: onTokenChannel
			});

			lastMetrics = metrics;
			return metrics;
		} catch (e) {
			const message = String(e);
			if (
				message.includes('INFERENCE_GENERATION_CANCELLED') ||
				message.includes('Generation cancelled')
			) {
				return null;
			}

			error = message;
			console.error('Streaming generation failed:', e);
			throw e;
		} finally {
			clearCancelTimeout(sessionId);
			if (isActiveGenerationSession(sessionId)) {
				isGenerating = false;
				cancelState = 'idle';
				void this.syncStatus();
			}
		}
	},

	async generateStreamMessages(
		messages: InferenceChatMessage[],
		onToken: (token: string) => void,
		config?: Partial<GenerationConfig>
	): Promise<GenerationMetrics | null> {
		if (!this.isReady) {
			error = 'Engine is not ready';
			return null;
		}

		if (isGenerating) {
			error = 'Generation already in progress';
			return null;
		}

		const sessionId = beginGenerationSession();

		try {
			const onTokenChannel = new Channel<string>();
			onTokenChannel.onmessage = onToken;

			const fullConfig: GenerationConfig | undefined = config
				? {
						max_length: config.max_length ?? 2048,
						temperature: config.temperature ?? 0.7,
						top_k: config.top_k ?? 40,
						top_p: config.top_p ?? 0.9,
						repetition_penalty: config.repetition_penalty ?? 1.1,
						repetition_penalty_last_n: config.repetition_penalty_last_n ?? 64
					}
				: undefined;

			const metrics = await invoke<GenerationMetrics>('inference_generate_messages', {
				messages,
				config: fullConfig,
				onToken: onTokenChannel
			});

			lastMetrics = metrics;
			return metrics;
		} catch (e) {
			const message = String(e);
			if (
				message.includes('INFERENCE_GENERATION_CANCELLED') ||
				message.includes('Generation cancelled')
			) {
				return null;
			}

			error = message;
			console.error('Streaming generation failed:', e);
			throw e;
		} finally {
			clearCancelTimeout(sessionId);
			if (isActiveGenerationSession(sessionId)) {
				isGenerating = false;
				cancelState = 'idle';
				void this.syncStatus();
			}
		}
	},

	/**
	 * Cancel the current generation.
	 *
	 * If the engine is hung in an FFI call that can't be interrupted,
	 * force-reset the UI state after 15 seconds so the user is not stuck behind
	 * the informational syncStatus() calls that still run during normal teardown.
	 */
	async cancel(): Promise<void> {
		if (!isGenerating && cancelState !== 'pending') {
			return;
		}

		const sessionId = activeGenerationSessionId;
		cancelState = 'pending';
		clearCancelTimeout();

		try {
			await invoke('inference_cancel');
		} catch (e) {
			console.error('Failed to cancel generation:', e);
		}

		cancelTimeoutSessionId = sessionId;
		cancelTimeoutId = setTimeout(async () => {
			if (!isActiveGenerationSession(sessionId) || (!isGenerating && cancelState !== 'pending')) {
				clearCancelTimeout(sessionId);
				return;
			}

			// Reconcile with engine host before force-clearing
			try {
				const hostGenerating = await invoke<boolean>('is_generating');
				if (hostGenerating) {
					// Host is stuck — force-clear UI anyway so the user isn't trapped
					console.warn('Host still generating after cancel timeout — forcing UI reset');
				}
			} catch {
				// Can't reach host — fall through to force-clear so UI isn't stuck
			}

			if (isActiveGenerationSession(sessionId)) {
				console.warn('Generation cancel timed out — force-resetting UI state');
				cancelState = 'timed_out';
				isGenerating = false;
				error = 'Generation did not respond to cancel. Try reloading the model.';
			}
			clearCancelTimeout(sessionId);
		}, 15000);
	},

	/**
	 * Fetch lane-based readiness for a specific model.
	 */
	async checkModelReadiness(modelId: string): Promise<CheckModelResponse | null> {
		try {
			return await invoke<CheckModelResponse>('check_model_readiness', { modelId });
		} catch (e) {
			console.error('Failed to check model readiness:', e);
			return null;
		}
	},

	/**
	 * @deprecated Prefer checkModelReadiness() for lane detail.
	 * Compatibility wrapper that returns true when at least one lane is actually ready.
	 */
	async checkModelExists(modelId: string): Promise<boolean> {
		const readiness = await this.checkModelReadiness(modelId);
		if (!readiness) {
			return false;
		}
		return (
			readiness.lanes.openvino_npu.ready ||
			readiness.lanes.directml.ready ||
			readiness.lanes.cpu.ready
		);
	},

	/**
	 * Refresh backend/runtime diagnostics from shared engine host.
	 */
	async refreshBackendStatus(): Promise<void> {
		try {
			const status = await invoke<BackendStatus>('get_inference_backend_status');
			backendStatus = status;
			runtimeMode = normalizeRuntimeMode(status.force_override);
		} catch (e) {
			console.warn('Failed to fetch backend status:', e);
		}
	},

	/**
	 * Retry startup using the same startup contract call.
	 */
	async retryStartup(defaultModelId: string | null = null): Promise<EngineReadinessDto | null> {
		return this.ensureStarted(defaultStartupRequest(defaultModelId));
	},

	async setRuntimeMode(
		mode: InferenceRuntimeMode,
		modelId: string | null = null
	): Promise<boolean> {
		if (isGenerating) {
			error = 'Cannot switch runtime mode while generation is in progress';
			return false;
		}

		error = null;
		try {
			const status = await invoke<BackendStatus>('set_inference_runtime_mode', {
				mode,
				modelId
			});
			backendStatus = status;
			runtimeMode = normalizeRuntimeMode(status.force_override);
			await this.syncStatus();
			return true;
		} catch (e) {
			error = String(e);
			console.error('Failed to switch runtime mode:', e);
			await this.refreshBackendStatus();
			return false;
		}
	},

	/**
	 * Clear non-readiness error state.
	 */
	clearError(): void {
		error = null;
	},

	/**
	 * Force-reset all generation state. Called when engine dies to ensure
	 * the UI is never stuck in a "generating" state with a dead engine.
	 */
	forceResetGenerationState(): void {
		clearCancelTimeout();
		isGenerating = false;
		cancelState = 'idle';
		activeGenerationSessionId = 0;
	}
};
