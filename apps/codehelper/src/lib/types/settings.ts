import type { InferenceRuntimeMode } from '$lib/types/inference';

export interface AppSettings {
	selectedModel: string;
	runtimeModePreference: InferenceRuntimeMode;
	contextEnabled: boolean;
	temperature: number;
	theme: 'light' | 'dark' | 'system';
}

export const DEFAULT_SETTINGS: AppSettings = {
	selectedModel: 'qwen2.5-1.5b-instruct',
	runtimeModePreference: 'auto',
	contextEnabled: true,
	temperature: 0.7,
	theme: 'system'
};
