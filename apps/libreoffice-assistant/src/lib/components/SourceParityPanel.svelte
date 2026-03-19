<script lang="ts">
  import type { ModelDefinition } from '../types/libreoffice';
  import { libreofficeChatStore } from '../stores/libreofficeChat.svelte';
  import SourceParityChatInput from './SourceParityChatInput.svelte';
  import SourceParityChatMessage from './SourceParityChatMessage.svelte';
  import SourceParitySettingsPage from './SourceParitySettingsPage.svelte';

  interface Props {
    models: ModelDefinition[];
    actionBusy: boolean;
  }

  let { models, actionBusy }: Props = $props();

  type View = 'chat' | 'settings';
  let currentView = $state<View>('chat');
  let messagesContainer = $state<HTMLDivElement | undefined>(undefined);

  $effect(() => {
    const messageCount = libreofficeChatStore.messageCount;
    const streamingHint = libreofficeChatStore.currentStreamingMessage;
    if (!messagesContainer || (messageCount === 0 && !streamingHint)) {
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
    </div>
  </div>

  {#if currentView === 'settings'}
    <SourceParitySettingsPage {models} onClose={() => (currentView = 'chat')} />
  {:else}
    <div class="source-parity__chat">
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
          class="secondary"
          onclick={() => libreofficeChatStore.clearMessages()}
          disabled={libreofficeChatStore.isGenerating}
        >
          Clear Chat
        </button>
      </div>

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

  .source-parity__actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.6rem;
    padding: 0.65rem 1rem;
    border-top: 1px solid #334155;
    border-bottom: 1px solid #334155;
    background: #111827;
  }

  .secondary {
    border: 1px solid #334155;
    border-radius: 8px;
    background: #0f172a;
    color: #e2e8f0;
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

  @media (max-width: 640px) {
    .source-parity__header {
      flex-direction: column;
    }
  }
</style>
