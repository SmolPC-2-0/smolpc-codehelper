import type { AppMode } from '$lib/types/mode';

import type { ToolExecutionResultDto } from '$lib/types/provider';

export interface Message {
	id: string;
	role: 'user' | 'assistant';
	content: string;
	timestamp: number;
	isStreaming?: boolean;
	explain?: string | null;
	undoable?: boolean;
	toolResults?: ToolExecutionResultDto[];
	plan?: unknown;
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
