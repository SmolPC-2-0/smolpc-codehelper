export interface OllamaMessage {
	role: 'system' | 'user' | 'assistant';
	content: string;
}

export interface Message {
	id: string;
	role: 'user' | 'assistant';
	content: string;
	timestamp: number;
	isStreaming?: boolean;
}

export interface OllamaStatus {
	connected: boolean;
	checking: boolean;
	error?: string;
}
