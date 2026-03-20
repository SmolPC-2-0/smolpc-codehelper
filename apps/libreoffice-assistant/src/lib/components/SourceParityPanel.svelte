<script lang="ts">
  import type { McpStatus, McpTool, ModelDefinition, ToolResult } from '../types/libreoffice';
  import type { SourceParityDependencyItem } from '../types/sourceParity';
  import { libreofficeChatStore } from '../stores/libreofficeChat.svelte';
  import { libreofficeSettingsStore } from '../stores/libreofficeSettings.svelte';
  import SourceParityChatInput from './SourceParityChatInput.svelte';
  import SourceParityLoadingScreen from './SourceParityLoadingScreen.svelte';
  import SourceParityChatMessage from './SourceParityChatMessage.svelte';
  import SourceParitySettingsPage from './SourceParitySettingsPage.svelte';
  import SourceParityToolsPage from './SourceParityToolsPage.svelte';

  interface Props {
    models: ModelDefinition[];
    actionBusy: boolean;
    actionMessage: string | null;
    commandError: string | null;
    dependencyLoading: boolean;
    dependencyReady: boolean;
    dependencies: SourceParityDependencyItem[];
    mcpStatus: McpStatus | null;
    mcpTools: McpTool[];
    selectedMcpTool: string;
    mcpArguments: string;
    mcpToolResult: ToolResult | null;
    onRefreshDependencies: () => void;
    onEnsureEngineStarted: () => void;
    onStartMcpServer: () => void;
    onRefreshMcpStatus: () => void;
    onStopMcpServer: () => void;
    onLoadMcpTools: () => void;
    onCallSelectedMcpTool: () => void;
    onSelectedMcpToolChange: (toolName: string) => void;
    onMcpArgumentsChange: (nextValue: string) => void;
    onApplyToolArgumentTemplate: (toolName: string) => void;
  }

  let {
    models,
    actionBusy,
    actionMessage,
    commandError,
    dependencyLoading,
    dependencyReady,
    dependencies,
    mcpStatus,
    mcpTools,
    selectedMcpTool,
    mcpArguments,
    mcpToolResult,
    onRefreshDependencies,
    onEnsureEngineStarted,
    onStartMcpServer,
    onRefreshMcpStatus,
    onStopMcpServer,
    onLoadMcpTools,
    onCallSelectedMcpTool,
    onSelectedMcpToolChange,
    onMcpArgumentsChange,
    onApplyToolArgumentTemplate
  }: Props = $props();

  type View = 'chat' | 'tools' | 'settings';
  let currentView = $state<View>('chat');
  let messagesContainer = $state<HTMLDivElement | undefined>(undefined);
  let restoredSavedAtLabel = $derived.by(() => {
    const restoredSavedAtIso = libreofficeChatStore.sessionRestoreMetadata.restoredSavedAtIso;
    if (!restoredSavedAtIso) {
      return null;
    }

    const parsed = new Date(restoredSavedAtIso);
    return Number.isNaN(parsed.getTime()) ? null : parsed.toLocaleString();
  });

  $effect(() => {
    const messageCount = libreofficeChatStore.messageCount;
    const streamingHint = libreofficeChatStore.currentStreamingMessage;
    if (currentView !== 'chat' || !messagesContainer || (messageCount === 0 && !streamingHint)) {
      return;
    }

    setTimeout(() => {
      if (messagesContainer) {
        messagesContainer.scrollTop = messagesContainer.scrollHeight;
      }
    }, 40);
  });

  function handleSend(message: string): void {
    void libreofficeChatStore.sendMessage(message);
  }

  function handleStartNewSession(): void {
    if (typeof window !== 'undefined') {
      const confirmed = window.confirm(
        'Start a new source-parity session? This clears the saved chat history for this browser profile.'
      );
      if (!confirmed) {
        return;
      }
    }

    libreofficeChatStore.clearMessages();
  }
</script>

<section class="panel source-parity">
  <div class="source-parity__header">
    <div>
      <h2>Source-Parity Workspace</h2>
      <p class="muted">Chat/settings migration slice from external LibreOffice app (engine-only)</p>
    </div>
    <div class="source-parity__tabs">
      <button
        type="button"
        class={currentView === 'chat' ? 'active' : ''}
        onclick={() => (currentView = 'chat')}
      >
        Chat
      </button>
      <button
        type="button"
        class={currentView === 'settings' ? 'active' : ''}
        onclick={() => (currentView = 'settings')}
      >
        Settings
      </button>
      <button
        type="button"
        class={currentView === 'tools' ? 'active' : ''}
        onclick={() => (currentView = 'tools')}
      >
        Tools
      </button>
    </div>
  </div>

  {#if currentView === 'settings'}
    <SourceParitySettingsPage {models} onClose={() => (currentView = 'chat')} />
  {:else if !dependencyReady}
    <SourceParityLoadingScreen
      loading={dependencyLoading}
      {dependencies}
      {actionBusy}
      onRefreshChecks={onRefreshDependencies}
      {onEnsureEngineStarted}
      {onStartMcpServer}
    />
  {:else if currentView === 'tools'}
    <SourceParityToolsPage
      {actionBusy}
      {actionMessage}
      {commandError}
      {mcpStatus}
      {mcpTools}
      {selectedMcpTool}
      {mcpArguments}
      {mcpToolResult}
      workflowMode={libreofficeSettingsStore.settings.workflow_mode}
      onRefreshMcpStatus={onRefreshMcpStatus}
      onStartMcpServer={onStartMcpServer}
      onStopMcpServer={onStopMcpServer}
      onLoadMcpTools={onLoadMcpTools}
      onCallSelectedMcpTool={onCallSelectedMcpTool}
      onSelectedMcpToolChange={onSelectedMcpToolChange}
      onMcpArgumentsChange={onMcpArgumentsChange}
      onApplyToolArgumentTemplate={onApplyToolArgumentTemplate}
    />
  {:else}
    <div class="source-parity__chat">
      {#if libreofficeChatStore.sessionRestoreMetadata.resetDueToCorruptPayload}
        <div class="session-banner session-banner--warning" role="status">
          Prior saved session data was malformed and has been reset for safety.
        </div>
      {/if}

      {#if libreofficeChatStore.sessionRestoreMetadata.restoreHappened}
        <div class="session-banner session-banner--info" role="status">
          <strong>Resumed previous session.</strong>
          Restored {libreofficeChatStore.sessionRestoreMetadata.restoredCount} message{libreofficeChatStore.sessionRestoreMetadata
            .restoredCount === 1
            ? ''
            : 's'}
          {#if restoredSavedAtLabel}
            (saved {restoredSavedAtLabel}).
          {/if}
        </div>
      {/if}

      <div class="source-parity__messages" bind:this={messagesContainer}>
        {#if libreofficeChatStore.messages.length === 0}
          <div class="welcome">
            <h3>Welcome to LibreOffice Chat</h3>
            <p>Ask to create, edit, and inspect documents through MCP tools.</p>
            <ul>
              <li>"List text documents in my Documents folder"</li>
              <li>"Summarize my latest writer document"</li>
              <li>"Create a short report draft about local-first AI in schools"</li>
            </ul>
          </div>
        {/if}

        {#each libreofficeChatStore.messages as message (message.id)}
          <SourceParityChatMessage {message} />
        {/each}

        {#if libreofficeChatStore.currentStreamingMessage}
          <div class="streaming-hint">
            {libreofficeChatStore.currentStreamingMessage}
          </div>
        {/if}
      </div>

      <div class="source-parity__actions">
        <button
          type="button"
          class="danger"
          onclick={handleStartNewSession}
          disabled={libreofficeChatStore.isGenerating}
        >
          Start New Session
        </button>
      </div>

      {#if libreofficeSettingsStore.settings.workflow_mode === 'tool_first' && !selectedMcpTool.trim()}
        <p class="tool-first-hint">
          Tool-first mode is enabled. Select an MCP tool in the Tools tab before sending.
        </p>
      {/if}

      <SourceParityChatInput
        onSend={handleSend}
        disabled={actionBusy || libreofficeChatStore.isGenerating}
      />
    </div>
  {/if}
</section>

<style>
  .source-parity {
    border-color: #1d4ed8;
    background: linear-gradient(180deg, #0b1220 0%, #0f172a 100%);
    color: #e2e8f0;
  }

  .source-parity__header {
    display: flex;
    justify-content: space-between;
    gap: 0.75rem;
    align-items: flex-start;
    margin-bottom: 0.9rem;
  }

  .source-parity__tabs {
    display: flex;
    gap: 0.5rem;
  }

  .source-parity__tabs button {
    border: 1px solid #334155;
    border-radius: 999px;
    background: #0f172a;
    color: #cbd5e1;
    padding: 0.45rem 0.75rem;
  }

  .source-parity__tabs button.active {
    border-color: #0ea5e9;
    background: #0ea5e9;
    color: #082f49;
  }

  .source-parity__chat {
    border: 1px solid #334155;
    border-radius: 10px;
    overflow: hidden;
  }

  .source-parity__messages {
    max-height: 420px;
    overflow-y: auto;
    padding: 1rem;
    background: #020617;
  }

  .session-banner {
    margin: 0.75rem 0.75rem 0;
    border-radius: 8px;
    padding: 0.65rem 0.8rem;
    font-size: 0.88rem;
    line-height: 1.4;
  }

  .session-banner--info {
    border: 1px solid #075985;
    background: #082f49;
    color: #bae6fd;
  }

  .session-banner--warning {
    border: 1px solid #f59e0b;
    background: #451a03;
    color: #fde68a;
  }

  .source-parity__actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.6rem;
    padding: 0.65rem 1rem;
    border-top: 1px solid #334155;
    border-bottom: 1px solid #334155;
    background: #111827;
  }

  .danger {
    border: 1px solid #ef4444;
    border-radius: 8px;
    background: #7f1d1d;
    color: #fee2e2;
    padding: 0.5rem 0.8rem;
    font-weight: 700;
  }

  .welcome {
    border: 1px solid #334155;
    border-radius: 10px;
    background: #111827;
    padding: 1rem;
    margin-bottom: 1rem;
  }

  .welcome h3 {
    margin: 0 0 0.5rem;
    color: #7dd3fc;
  }

  .welcome p {
    margin: 0 0 0.7rem;
    color: #cbd5e1;
  }

  .welcome ul {
    margin: 0;
    padding-left: 1.1rem;
  }

  .welcome li {
    margin-top: 0.35rem;
    color: #94a3b8;
  }

  .streaming-hint {
    margin-top: 0.4rem;
    color: #7dd3fc;
    font-weight: 700;
  }

  .tool-first-hint {
    margin: 0;
    padding: 0.75rem 1rem;
    border-top: 1px solid #334155;
    background: #172554;
    color: #bfdbfe;
    font-size: 0.88rem;
  }

  @media (max-width: 640px) {
    .source-parity__header {
      flex-direction: column;
    }
  }
</style>
