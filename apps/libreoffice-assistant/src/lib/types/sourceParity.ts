export type SourceParityChatRole = 'user' | 'assistant' | 'tool' | 'system';

export type SourceParityTheme = 'dark' | 'light';

export type SourceParityWorkflowMode = 'mcp_assisted' | 'tool_first';

export interface SourceParityChatMessage {
  id: string;
  role: SourceParityChatRole;
  content: string;
  timestamp: Date;
  workflowOutcome?: string;
  isError?: boolean;
}

export interface SourceParitySettings {
  selected_model: string;
  python_path: string;
  documents_path: string;
  libreoffice_path: string | null;
  theme: SourceParityTheme;
  system_prompt?: string;
  temperature: number;
  max_tokens: number;
  workflow_mode: SourceParityWorkflowMode;
}

export const DEFAULT_SOURCE_PARITY_SETTINGS: SourceParitySettings = {
  selected_model: 'qwen3-4b-instruct-2507',
  python_path: 'python',
  documents_path: '~/Documents',
  libreoffice_path: null,
  theme: 'dark',
  system_prompt: '',
  temperature: 0.0,
  max_tokens: 64,
  workflow_mode: 'mcp_assisted'
};
