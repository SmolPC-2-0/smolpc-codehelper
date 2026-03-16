import type { AppMode } from '$lib/types/mode';
import type { ToolExecutionResultDto } from '$lib/types/provider';

export interface AssistantMessageDto {
	role: string;
	content: string;
}

export interface AssistantSendRequestDto {
	mode: AppMode;
	chatId: string | null;
	messages: AssistantMessageDto[];
	userText: string;
}

export interface AssistantResponseDto {
	reply: string;
	explain: string | null;
	undoable: boolean;
	plan: unknown;
	toolResults: ToolExecutionResultDto[];
}

export type AssistantStreamEvent =
	| { kind: 'status'; phase: string; detail: string }
	| { kind: 'tool_call'; name: string; arguments: unknown }
	| { kind: 'tool_result'; name: string; result: ToolExecutionResultDto }
	| { kind: 'token'; token: string }
	| { kind: 'complete'; response: AssistantResponseDto }
	| { kind: 'error'; code: string; message: string };
