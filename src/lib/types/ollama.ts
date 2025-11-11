export interface OllamaMessage {
	role: 'system' | 'user' | 'assistant';
	content: string;
}

export interface OllamaStatus {
	connected: boolean;
	checking: boolean;
	error?: string;
}

export interface OllamaModel {
	name: string;
	modified_at: string;
	size: number;
}

export interface OllamaGenerateRequest {
	model: string;
	prompt?: string;
	messages?: OllamaMessage[];
	stream: boolean;
	options?: {
		temperature?: number;
		top_p?: number;
		top_k?: number;
	};
}

export interface OllamaStreamChunk {
	model: string;
	message?: {
		role: string;
		content: string;
	};
	done: boolean;
}
