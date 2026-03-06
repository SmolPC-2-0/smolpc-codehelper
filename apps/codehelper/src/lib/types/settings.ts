export interface AppSettings {
	selectedModel: string;
	contextEnabled: boolean;
	temperature: number;
	theme: 'light' | 'dark' | 'system';
}

export const DEFAULT_SETTINGS: AppSettings = {
	selectedModel: 'qwen3-4b-instruct-2507',
	contextEnabled: true,
	temperature: 0.7,
	theme: 'system'
};
