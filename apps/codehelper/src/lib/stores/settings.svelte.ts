import type { AppSettings } from '$lib/types/settings';
import { DEFAULT_SETTINGS } from '$lib/types/settings';
import { saveToStorage, loadFromStorage } from '$lib/utils/storage';

const STORAGE_KEY = 'smolpc_settings';

// Load initial state from localStorage
const initialSettings = loadFromStorage<AppSettings>(STORAGE_KEY, DEFAULT_SETTINGS);

// Svelte 5 state using runes
let settings = $state<AppSettings>(initialSettings);

// Store object with methods
export const settingsStore = {
	// Getters
	get settings() {
		return settings;
	},
	get selectedModel() {
		return settings.selectedModel;
	},
	get contextEnabled() {
		return settings.contextEnabled;
	},
	get temperature() {
		return settings.temperature;
	},
	get theme() {
		return settings.theme;
	},

	// Actions
	setModel(model: string) {
		settings.selectedModel = model;
		this.persist();
	},

	toggleContext() {
		settings.contextEnabled = !settings.contextEnabled;
		this.persist();
	},

	setContextEnabled(enabled: boolean) {
		settings.contextEnabled = enabled;
		this.persist();
	},

	setTemperature(temp: number) {
		settings.temperature = Math.max(0, Math.min(1, temp));
		this.persist();
	},

	setTheme(theme: 'light' | 'dark' | 'system') {
		settings.theme = theme;
		this.persist();
	},

	resetToDefaults() {
		settings = { ...DEFAULT_SETTINGS };
		this.persist();
	},

	persist() {
		saveToStorage(STORAGE_KEY, settings);
	}
};
