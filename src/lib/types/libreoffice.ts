/**
 * LibreOffice MCP integration types
 */

/** Connection status from backend */
export interface LibreOfficeStatus {
	connected: boolean;
	connecting: boolean;
	serverName: string | null;
	serverVersion: string | null;
	error?: string;
}

/** MCP Tool definition */
export interface MCPTool {
	name: string;
	description: string;
	inputSchema: Record<string, unknown>;
}

/** Tool call result from backend */
export interface ToolCallResult {
	success: boolean;
	result: unknown | null;
	error: string | null;
}

/** Status response from libreoffice_status command */
export interface StatusResponse {
	connected: boolean;
	serverName: string | null;
	serverVersion: string | null;
}

/** Document types supported by LibreOffice */
export type DocumentType = 'text' | 'spreadsheet' | 'presentation';

/** Parameters for creating a document */
export interface CreateDocumentParams {
	filename: string;
	title?: string;
	docType?: DocumentType;
}

/** Parameters for adding text */
export interface AddTextParams {
	text: string;
}

/** Parameters for saving a document */
export interface SaveDocumentParams {
	path?: string;
}

/** Parameters for generic tool call */
export interface ToolCallParams {
	toolName: string;
	arguments: Record<string, unknown>;
}