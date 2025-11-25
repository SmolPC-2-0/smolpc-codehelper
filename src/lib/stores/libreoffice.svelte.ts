import { invoke } from '@tauri-apps/api/core';
import type {
	LibreOfficeStatus,
	MCPTool,
	ToolCallResult,
	StatusResponse
} from '$lib/types/libreoffice';

// Svelte 5 state using runes
let status = $state<LibreOfficeStatus>({
	connected: false,
	connecting: false,
	serverName: null,
	serverVersion: null,
	error: undefined
});

let tools = $state<MCPTool[]>([]);

// Store object with methods
export const libreOfficeStore = {
	// Getters
	get status() {
		return status;
	},
	get tools() {
		return tools;
	},
	get isConnected() {
		return status.connected;
	},

	// Actions
	async connect(): Promise<boolean> {
		if (status.connecting) return false;

		status.connecting = true;
		status.error = undefined;

		try {
			const response = await invoke<StatusResponse>('libreoffice_connect');
			status.connected = response.connected;
			status.serverName = response.serverName;
			status.serverVersion = response.serverVersion;
			status.connecting = false;

			if (response.connected) {
				await this.fetchTools();
			}

			return response.connected;
		} catch (error) {
			status.connected = false;
			status.connecting = false;
			status.error = error instanceof Error ? error.message : String(error);
			return false;
		}
	},

	async disconnect(): Promise<void> {
		try {
			await invoke('libreoffice_disconnect');
			status.connected = false;
			status.serverName = null;
			status.serverVersion = null;
			tools = [];
		} catch (error) {
			console.error('Failed to disconnect:', error);
			status.error = error instanceof Error ? error.message : String(error);
		}
	},

	async checkStatus(): Promise<void> {
		try {
			const response = await invoke<StatusResponse>('libreoffice_status');
			status.connected = response.connected;
			status.serverName = response.serverName;
			status.serverVersion = response.serverVersion;
		} catch (error) {
			status.connected = false;
			status.error = error instanceof Error ? error.message : String(error);
		}
	},

	async fetchTools(): Promise<MCPTool[]> {
		console.log('fetchTools called');
		try {
			const fetchedTools = await invoke<MCPTool[]>('libreoffice_list_tools');
			console.log('Fetched tools:', fetchedTools.length, 'tools');
			tools = fetchedTools;
			return fetchedTools;
		} catch (error) {
			console.error('Failed to fetch tools:', error);
			tools = [];
			return [];
		}
	},

	async callTool(toolName: string, args: Record<string, unknown>): Promise<ToolCallResult> {
		try {
			const result = await invoke<ToolCallResult>('libreoffice_call_tool', {
				tool_name: toolName,
				arguments: args
			});
			return result;
		} catch (error) {
			return {
				success: false,
				result: null,
				error: error instanceof Error ? error.message : String(error)
			};
		}
	},

	// Convenience methods
	async createDocument(
		filename: string,
		title?: string,
		docType?: string
	): Promise<ToolCallResult> {
		try {
			const result = await invoke<ToolCallResult>('libreoffice_create_document', {
				filename,
				title,
				doc_type: docType
			});
			return result;
		} catch (error) {
			return {
				success: false,
				result: null,
				error: error instanceof Error ? error.message : String(error)
			};
		}
	},

	async addText(text: string): Promise<ToolCallResult> {
		try {
			const result = await invoke<ToolCallResult>('libreoffice_add_text', { text });
			return result;
		} catch (error) {
			return {
				success: false,
				result: null,
				error: error instanceof Error ? error.message : String(error)
			};
		}
	},

	async saveDocument(path?: string): Promise<ToolCallResult> {
		try {
			const result = await invoke<ToolCallResult>('libreoffice_save_document', { path });
			return result;
		} catch (error) {
			return {
				success: false,
				result: null,
				error: error instanceof Error ? error.message : String(error)
			};
		}
	},

	setError(error: string) {
		status.error = error;
	},

	clearError() {
		status.error = undefined;
	}
};