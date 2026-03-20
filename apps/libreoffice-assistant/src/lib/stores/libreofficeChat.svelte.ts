import {
  SOURCE_PARITY_CHAT_SESSION_SCHEMA_VERSION,
  type SourceParityChatMessage,
  type SourceParityChatRole,
  type SourceParityChatSessionPayloadV1,
  type SourceParityPersistedChatMessageV1
} from '../types/sourceParity';
import { loadFromStorage, removeFromStorage, saveToStorage } from '../utils/storage';
import { libreofficeController } from './libreofficeController.svelte';
import { libreofficeSettingsStore } from './libreofficeSettings.svelte';

const CHAT_SESSION_STORAGE_KEY = 'libreoffice_assistant_source_parity_chat_session_v1';

function newChatMessage(
  role: SourceParityChatRole,
  content: string,
  workflowOutcome?: string,
  isError = false
): SourceParityChatMessage {
  return {
    id: crypto.randomUUID(),
    role,
    content,
    timestamp: new Date(),
    workflowOutcome,
    isError
  };
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function normalizeRole(value: unknown): SourceParityChatRole | null {
  if (value === 'user' || value === 'assistant' || value === 'tool' || value === 'system') {
    return value;
  }

  return null;
}

function normalizeTimestamp(isoValue: unknown): Date {
  if (typeof isoValue !== 'string') {
    return new Date();
  }

  const parsed = new Date(isoValue);
  return Number.isNaN(parsed.getTime()) ? new Date() : parsed;
}

function normalizePersistedMessage(value: unknown): SourceParityChatMessage | null {
  if (!isObject(value)) {
    return null;
  }

  const role = normalizeRole(value.role);
  if (role === null || typeof value.content !== 'string') {
    return null;
  }

  const id =
    typeof value.id === 'string' && value.id.trim().length > 0 ? value.id : crypto.randomUUID();
  const workflowOutcome = typeof value.workflow_outcome === 'string' ? value.workflow_outcome : undefined;
  const isError = typeof value.is_error === 'boolean' ? value.is_error : undefined;

  return {
    id,
    role,
    content: value.content,
    timestamp: normalizeTimestamp(value.timestamp_iso),
    workflowOutcome,
    isError
  };
}

function toPersistedMessage(message: SourceParityChatMessage): SourceParityPersistedChatMessageV1 {
  return {
    id: message.id,
    role: message.role,
    content: message.content,
    timestamp_iso: message.timestamp.toISOString(),
    workflow_outcome: message.workflowOutcome,
    is_error: message.isError
  };
}

function buildPersistedSession(
  messages: SourceParityChatMessage[]
): SourceParityChatSessionPayloadV1 {
  return {
    schema_version: SOURCE_PARITY_CHAT_SESSION_SCHEMA_VERSION,
    saved_at_iso: new Date().toISOString(),
    messages: messages.map(toPersistedMessage)
  };
}

type RestoredSession = {
  messages: SourceParityChatMessage[];
  shouldRewriteStorage: boolean;
  shouldClearStorage: boolean;
};

function restoreMessagesFromPayload(raw: unknown): RestoredSession {
  if (raw === null) {
    return {
      messages: [],
      shouldRewriteStorage: false,
      shouldClearStorage: false
    };
  }

  if (!isObject(raw)) {
    return {
      messages: [],
      shouldRewriteStorage: false,
      shouldClearStorage: true
    };
  }

  if (raw.schema_version !== SOURCE_PARITY_CHAT_SESSION_SCHEMA_VERSION || !Array.isArray(raw.messages)) {
    return {
      messages: [],
      shouldRewriteStorage: false,
      shouldClearStorage: true
    };
  }

  const restoredMessages: SourceParityChatMessage[] = [];
  let droppedMalformedMessage = false;
  for (const rawMessage of raw.messages) {
    const normalizedMessage = normalizePersistedMessage(rawMessage);
    if (normalizedMessage === null) {
      droppedMalformedMessage = true;
      continue;
    }

    restoredMessages.push(normalizedMessage);
  }

  if (restoredMessages.length === 0 && raw.messages.length > 0) {
    return {
      messages: [],
      shouldRewriteStorage: false,
      shouldClearStorage: true
    };
  }

  return {
    messages: restoredMessages,
    shouldRewriteStorage: droppedMalformedMessage,
    shouldClearStorage: false
  };
}

function shouldCaptureToolTraceLine(line: string): boolean {
  return line.startsWith('Tool call ') || line.startsWith('Tool result ');
}

class LibreofficeChatStore {
  messages = $state<SourceParityChatMessage[]>([]);
  isGenerating = $state(false);
  currentStreamingMessage = $state('');

  constructor() {
    this.restoreSession();
  }

  get messageCount() {
    return this.messages.length;
  }

  addMessage(
    role: SourceParityChatRole,
    content: string,
    workflowOutcome?: string,
    isError = false
  ): void {
    this.messages = [...this.messages, newChatMessage(role, content, workflowOutcome, isError)];
    this.persistSession();
  }

  clearMessages(): void {
    this.messages = [];
    removeFromStorage(CHAT_SESSION_STORAGE_KEY);
  }

  private syncControllerFromSettings(): void {
    const settings = libreofficeSettingsStore.settings;
    libreofficeController.setSelectedModelId(settings.selected_model);
    libreofficeController.setMcpPythonPath(settings.python_path);
    libreofficeController.setWorkflowSystemPrompt(settings.system_prompt ?? '');
    libreofficeController.setWorkflowTemperature(settings.temperature);
    libreofficeController.setWorkflowMaxTokens(settings.max_tokens);
  }

  private appendToolTraceMessages(trace: string[]): void {
    const lines = trace.filter(shouldCaptureToolTraceLine);
    for (const line of lines) {
      this.addMessage('tool', line);
    }
  }

  private restoreSession(): void {
    const rawPayload = loadFromStorage<unknown>(CHAT_SESSION_STORAGE_KEY, null);
    const restored = restoreMessagesFromPayload(rawPayload);

    this.messages = restored.messages;

    if (restored.shouldClearStorage) {
      removeFromStorage(CHAT_SESSION_STORAGE_KEY);
      return;
    }

    if (restored.shouldRewriteStorage) {
      this.persistSession();
    }
  }

  private persistSession(): void {
    if (this.messages.length === 0) {
      removeFromStorage(CHAT_SESSION_STORAGE_KEY);
      return;
    }

    const payload = buildPersistedSession(this.messages);
    saveToStorage(CHAT_SESSION_STORAGE_KEY, payload);
  }

  async sendMessage(content: string): Promise<void> {
    const trimmed = content.trim();
    if (!trimmed || this.isGenerating) {
      return;
    }

    const workflowMode = libreofficeSettingsStore.settings.workflow_mode;
    const useToolFirst = workflowMode === 'tool_first';
    if (useToolFirst && !libreofficeController.selectedMcpTool.trim()) {
      this.addMessage(
        'assistant',
        'Tool-first mode requires a selected MCP tool in the Source-Parity Tools tab.',
        'failed_with_error',
        true
      );
      return;
    }

    this.addMessage('user', trimmed);
    this.isGenerating = true;
    this.currentStreamingMessage = 'Running MCP-assisted workflow...';

    try {
      this.syncControllerFromSettings();
      libreofficeController.setWorkflowPrompt(trimmed);

      if (useToolFirst) {
        await libreofficeController.runToolFirstWorkflow();
      } else {
        await libreofficeController.runMcpAssistedWorkflow();
      }

      this.appendToolTraceMessages(libreofficeController.workflowTrace);

      const finalResponse = libreofficeController.workflowFinalResponse.trim();
      if (finalResponse) {
        this.addMessage('assistant', finalResponse, libreofficeController.workflowOutcome);
      } else if (libreofficeController.commandError) {
        this.addMessage('assistant', libreofficeController.commandError, 'failed_with_error', true);
      } else {
        this.addMessage(
          'assistant',
          'Workflow completed without a final response.',
          'failed_with_error',
          true
        );
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.addMessage('assistant', `Workflow failed: ${message}`, 'failed_with_error', true);
    } finally {
      this.currentStreamingMessage = '';
      this.isGenerating = false;
    }
  }
}

export const libreofficeChatStore = new LibreofficeChatStore();
