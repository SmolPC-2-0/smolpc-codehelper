/**
 * Inference store for ONNX Runtime model management and text generation
 *
 * Supports both blocking and streaming generation modes.
 */
import { invoke, Channel } from '@tauri-apps/api/core';
import type {
	GenerationResult,
	GenerationMetrics,
	GenerationConfig,
	AvailableModel,
	InferenceStatus,
	BackendStatus
} from '$lib/types/inference';

// State
let isLoaded = $state(false);
let currentModel = $state<string | null>(null);
let isGenerating = $state(false);
let error = $state<string | null>(null);
let availableModels = $state<AvailableModel[]>([]);
let lastResult = $state<GenerationResult | null>(null);
let lastMetrics = $state<GenerationMetrics | null>(null);
let backendStatus = $state<BackendStatus | null>(null);

function normalizeBackendName(raw: string | null | undefined): string | null {
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
	return raw.toLowerCase();
}

export const inferenceStore = {
	// Getters
	get isLoaded() {
		return isLoaded;
	},
	get currentModel() {
		return currentModel;
	},
	get isGenerating() {
		return isGenerating;
	},
	get error() {
		return error;
	},
	get availableModels() {
		return availableModels;
	},
	get lastResult() {
		return lastResult;
	},
	get lastMetrics() {
		return lastMetrics;
	},

	// Get status object for display
	get status(): InferenceStatus {
		return {
			isLoaded,
			currentModel,
			isGenerating,
			error,
			activeBackend: normalizeBackendName(backendStatus?.active_backend),
			runtimeEngine: backendStatus?.runtime_engine ?? null,
			activeModelPath: backendStatus?.active_model_path ?? null,
			selectionState: backendStatus?.selection_state ?? null,
			selectionReason: backendStatus?.selection_reason ?? null,
			selectedDeviceName: backendStatus?.selected_device_name ?? null
		};
	},

	// Actions

	/**
	 * List available ONNX models
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
	 * Load a model by ID
	 */
	async loadModel(modelId: string): Promise<boolean> {
		if (isLoaded && currentModel === modelId) {
			return true; // Already loaded
		}

		error = null;

		try {
			await invoke('load_model', { modelId });
			isLoaded = true;
			currentModel = modelId;
			await this.refreshBackendStatus();
			return true;
		} catch (e) {
			error = String(e);
			console.error('Failed to load model:', e);
			return false;
		}
	},

	/**
	 * Unload the current model
	 */
	async unloadModel(): Promise<void> {
		try {
			await invoke('unload_model');
			isLoaded = false;
			currentModel = null;
			backendStatus = null;
		} catch (e) {
			error = String(e);
			console.error('Failed to unload model:', e);
		}
	},

	/**
	 * Generate text from a prompt (blocking, returns full result)
	 */
	async generate(prompt: string): Promise<GenerationResult | null> {
		if (!isLoaded) {
			error = 'No model loaded';
			return null;
		}

		if (isGenerating) {
			error = 'Generation already in progress';
			return null;
		}

		isGenerating = true;
		error = null;

		try {
			const result = await invoke<GenerationResult>('generate_text', { prompt });
			lastResult = result;
			return result;
		} catch (e) {
			error = String(e);
			console.error('Generation failed:', e);
			return null;
		} finally {
			await this.refreshBackendStatus();
			isGenerating = false;
		}
	},

	/**
	 * Generate text with streaming output via Tauri Channel
	 *
	 * @param prompt - Input prompt
	 * @param onToken - Callback for each generated token
	 * @param config - Optional generation configuration
	 * @returns Metrics on success, null on cancellation or error
	 */
	async generateStream(
		prompt: string,
		onToken: (token: string) => void,
		config?: Partial<GenerationConfig>
	): Promise<GenerationMetrics | null> {
		if (!isLoaded) {
			error = 'No model loaded';
			return null;
		}

		if (isGenerating) {
			error = 'Generation already in progress';
			return null;
		}

		isGenerating = true;
		error = null;
		lastMetrics = null;

		try {
			// Create channel — tokens delivered via onmessage callback
			const onTokenChannel = new Channel<string>();
			onTokenChannel.onmessage = onToken;

			// Build config with defaults
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

			// invoke() now returns metrics directly when generation completes
			const metrics = await invoke<GenerationMetrics>('inference_generate', {
				prompt,
				config: fullConfig,
				onToken: onTokenChannel
			});

			lastMetrics = metrics;
			return metrics;
		} catch (e) {
			const message = String(e);

			// Cancellation is not an error — return null
			if (
				message.includes('INFERENCE_GENERATION_CANCELLED') ||
				message.includes('Generation cancelled')
			) {
				return null;
			}

			error = message;
			console.error('Streaming generation failed:', e);
			return null;
		} finally {
			await this.refreshBackendStatus();
			isGenerating = false;
		}
	},

	/**
	 * Cancel the current generation
	 */
	async cancel(): Promise<void> {
		if (!isGenerating) {
			return;
		}

		try {
			await invoke('inference_cancel');
		} catch (e) {
			console.error('Failed to cancel generation:', e);
		}
	},

	/**
	 * Check if a specific model exists
	 */
	async checkModelExists(modelId: string): Promise<boolean> {
		try {
			return await invoke<boolean>('check_model_exists', { modelId });
		} catch (e) {
			console.error('Failed to check model:', e);
			return false;
		}
	},

	/**
	 * Get the currently loaded model info from backend
	 */
	async syncStatus(): Promise<void> {
		try {
			const model = await invoke<string | null>('get_current_model');
			if (model) {
				isLoaded = true;
				currentModel = model;
				await this.refreshBackendStatus();
			} else {
				isLoaded = false;
				currentModel = null;
				backendStatus = null;
			}
		} catch (e) {
			console.error('Failed to sync status:', e);
		}
	},

	/**
	 * Fetch backend/runtime status from shared engine host.
	 */
	async refreshBackendStatus(): Promise<void> {
		try {
			const status = await invoke<BackendStatus>('get_inference_backend_status');
			backendStatus = status;
		} catch (e) {
			backendStatus = null;
			console.warn('Failed to fetch backend status:', e);
		}
	},

	/**
	 * Clear error state
	 */
	clearError(): void {
		error = null;
	}
};
