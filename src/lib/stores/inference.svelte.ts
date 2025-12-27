/**
 * Inference store for ONNX Runtime model management and text generation
 *
 * Supports both blocking and streaming generation modes.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
	GenerationResult,
	GenerationMetrics,
	GenerationConfig,
	AvailableModel,
	InferenceStatus
} from '$lib/types/inference';

// State
let isLoaded = $state(false);
let currentModel = $state<string | null>(null);
let isGenerating = $state(false);
let error = $state<string | null>(null);
let availableModels = $state<AvailableModel[]>([]);
let lastResult = $state<GenerationResult | null>(null);
let lastMetrics = $state<GenerationMetrics | null>(null);

// Event listeners for streaming
let tokenUnlisten: UnlistenFn | null = null;
let doneUnlisten: UnlistenFn | null = null;
let errorUnlisten: UnlistenFn | null = null;
let cancelledUnlisten: UnlistenFn | null = null;

// Callback for streaming tokens
let streamCallback: ((token: string) => void) | null = null;

/**
 * Set up event listeners for streaming generation
 */
async function setupStreamListeners(): Promise<void> {
	// Clean up any existing listeners
	await cleanupStreamListeners();

	tokenUnlisten = await listen<string>('inference_token', (event) => {
		if (streamCallback) {
			streamCallback(event.payload);
		}
	});

	doneUnlisten = await listen<GenerationMetrics>('inference_done', (event) => {
		lastMetrics = event.payload;
		isGenerating = false;
	});

	errorUnlisten = await listen<string>('inference_error', (event) => {
		error = event.payload;
		isGenerating = false;
	});

	cancelledUnlisten = await listen('inference_cancelled', () => {
		isGenerating = false;
	});
}

/**
 * Clean up event listeners
 */
async function cleanupStreamListeners(): Promise<void> {
	if (tokenUnlisten) {
		tokenUnlisten();
		tokenUnlisten = null;
	}
	if (doneUnlisten) {
		doneUnlisten();
		doneUnlisten = null;
	}
	if (errorUnlisten) {
		errorUnlisten();
		errorUnlisten = null;
	}
	if (cancelledUnlisten) {
		cancelledUnlisten();
		cancelledUnlisten = null;
	}
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
			error
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
			isGenerating = false;
		}
	},

	/**
	 * Generate text with streaming output
	 *
	 * @param prompt - Input prompt
	 * @param onToken - Callback for each generated token
	 * @param config - Optional generation configuration
	 * @returns Promise that resolves when generation is complete
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
		streamCallback = onToken;

		try {
			// Set up event listeners
			await setupStreamListeners();

			// Build config with defaults
			const fullConfig: GenerationConfig | undefined = config
				? {
						max_length: config.max_length ?? 2048,
						temperature: config.temperature ?? 0.7,
						top_k: config.top_k ?? 40,
						top_p: config.top_p ?? 0.9
					}
				: undefined;

			// Start streaming generation
			await invoke('inference_generate', { prompt, config: fullConfig });

			// Return metrics (will be set by event listener)
			return lastMetrics;
		} catch (e) {
			error = String(e);
			console.error('Streaming generation failed:', e);
			isGenerating = false;
			return null;
		} finally {
			streamCallback = null;
			await cleanupStreamListeners();
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
			} else {
				isLoaded = false;
				currentModel = null;
			}
		} catch (e) {
			console.error('Failed to sync status:', e);
		}
	},

	/**
	 * Clear error state
	 */
	clearError(): void {
		error = null;
	}
};
