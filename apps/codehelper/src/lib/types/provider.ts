import type { AppMode } from '$lib/types/mode';

export interface ProviderStateDto {
	mode: AppMode;
	state: string;
	detail: string | null;
	supportsTools: boolean;
	supportsUndo: boolean;
}

export interface ToolDefinitionDto {
	name: string;
	description: string;
	inputSchema: unknown;
}

export interface ToolExecutionResultDto {
	name: string;
	ok: boolean;
	summary: string;
	payload: unknown;
}

export interface ModeStatusDto {
	mode: AppMode;
	engineReady: boolean;
	providerState: ProviderStateDto;
	availableTools: ToolDefinitionDto[];
	lastError: string | null;
}

