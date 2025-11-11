export interface AppSettings {
	selectedModel: string;
	contextEnabled: boolean;
	temperature: number;
	theme: 'light' | 'dark' | 'system';
}

export interface ModelInfo {
	name: string;
	displayName: string;
	size?: string;
}

export const AVAILABLE_MODELS: ModelInfo[] = [
	{ name: 'qwen2.5-coder:7b', displayName: 'Qwen 2.5 Coder', size: '7B' },
	{ name: 'deepseek-coder:6.7b', displayName: 'DeepSeek Coder', size: '6.7B' }
];

export const DEFAULT_SETTINGS: AppSettings = {
	selectedModel: 'qwen2.5-coder:7b',
	contextEnabled: true,
	temperature: 0.7,
	theme: 'system'
};
