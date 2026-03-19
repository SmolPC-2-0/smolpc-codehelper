import type { SourceParityChatMessage, SourceParityChatRole } from '../types/sourceParity';
import { libreofficeController } from './libreofficeController.svelte';
import { libreofficeSettingsStore } from './libreofficeSettings.svelte';

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

function shouldCaptureToolTraceLine(line: string): boolean {
  return line.startsWith('Tool call ') || line.startsWith('Tool result ');
}

class LibreofficeChatStore {
  messages = $state<SourceParityChatMessage[]>([]);
  isGenerating = $state(false);
  currentStreamingMessage = $state('');

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
  }

  clearMessages(): void {
    this.messages = [];
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
