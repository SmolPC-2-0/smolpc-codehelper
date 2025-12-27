/**
 * Inference store for ONNX Runtime model management and text generation
 */
import { invoke } from '@tauri-apps/api/core';
import type {
	GenerationResult,
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
	 * Generate text from a prompt
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
