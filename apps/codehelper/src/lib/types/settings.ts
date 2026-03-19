export interface AppSettings {
	selectedModel: string;
	contextEnabled: boolean;
	temperature: number;
	theme: 'light' | 'dark' | 'system';
}

export const DEFAULT_SETTINGS: AppSettings = {
	selectedModel: 'qwen2.5-1.5b-instruct',
	contextEnabled: true,
	temperature: 0.7,
	theme: 'system'
};
