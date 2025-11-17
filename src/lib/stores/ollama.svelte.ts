import { invoke } from '@tauri-apps/api/core';
import type { OllamaStatus, OllamaModel } from '$lib/types/ollama';

// Svelte 5 state using runes
let status = $state<OllamaStatus>({
	connected: false,
	checking: false,
	error: undefined
});

let availableModels = $state<string[]>([]);

// Store object with methods
export const ollamaStore = {
	// Getters
	get status() {
		return status;
	},
	get availableModels() {
		return availableModels;
	},
	get isConnected() {
		return status.connected;
	},

	// Actions
	async checkConnection(): Promise<boolean> {
		status.checking = true;
		status.error = undefined;

		try {
			const connected = await invoke<boolean>('check_ollama');
			status.connected = connected;
			status.checking = false;

			if (connected) {
				await this.fetchModels();
			}

			return connected;
		} catch (error) {
			status.connected = false;
			status.checking = false;
			status.error = error instanceof Error ? error.message : 'Failed to connect to Ollama';
			return false;
		}
	},

	async fetchModels(): Promise<void> {
		try {
			const models = await invoke<string[]>('get_ollama_models');
			availableModels = models;
		} catch (error) {
			console.error('Failed to fetch models:', error);
			availableModels = [];
		}
	},

	setError(error: string) {
		status.error = error;
		status.connected = false;
	},

	clearError() {
		status.error = undefined;
	}
};
