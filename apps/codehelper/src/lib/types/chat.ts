import type { AppMode } from '$lib/types/mode';

export interface Message {
	id: string;
	role: 'user' | 'assistant';
	content: string;
	timestamp: number;
	isStreaming?: boolean;
}

export interface Chat {
	id: string;
	mode: AppMode;
	title: string;
	messages: Message[];
	createdAt: number;
	updatedAt: number;
	model: string;
	pinned?: boolean;
	archived?: boolean;
}

export interface ChatGroup {
	label: string;
	chats: Chat[];
}

export type TimeGroup = 'today' | 'yesterday' | 'lastWeek' | 'older';
