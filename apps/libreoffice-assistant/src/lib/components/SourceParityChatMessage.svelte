<script lang="ts">
  import type { SourceParityChatMessage } from '../types/sourceParity';

  interface Props {
    message: SourceParityChatMessage;
  }

  let { message }: Props = $props();

  function roleLabel(role: SourceParityChatMessage['role']): string {
    if (role === 'user') {
      return 'You';
    }
    if (role === 'assistant') {
      return 'Assistant';
    }
    if (role === 'tool') {
      return 'Tool';
    }
    return 'System';
  }
</script>

<article class="message {message.role} {message.isError ? 'error' : ''}">
  <div class="message-header">
    <span class="role">{roleLabel(message.role)}</span>
    <span class="timestamp">{message.timestamp.toLocaleTimeString()}</span>
  </div>
  <div class="message-content">{message.content}</div>
  {#if message.workflowOutcome}
    <p class="meta">Outcome: <code>{message.workflowOutcome}</code></p>
  {/if}
</article>

<style>
  .message {
    border: 1px solid #334155;
    border-radius: 10px;
    margin-bottom: 0.85rem;
    padding: 0.85rem;
    background: #0f172a;
  }

  .message.user {
    background: #0b3a53;
    border-color: #0ea5e9;
    margin-left: 1.5rem;
  }

  .message.assistant {
    background: #111827;
    margin-right: 1.5rem;
  }

  .message.tool {
    background: #1f2937;
    border-style: dashed;
  }

  .message.system {
    background: #1e293b;
  }

  .message.error {
    border-color: #dc2626;
    background: #450a0a;
  }

  .message-header {
    display: flex;
    justify-content: space-between;
    margin-bottom: 0.5rem;
    font-size: 0.82rem;
  }

  .role {
    font-weight: 700;
    color: #bae6fd;
  }

  .timestamp {
    color: #94a3b8;
  }

  .message-content {
    white-space: pre-wrap;
    word-break: break-word;
    color: #e2e8f0;
    line-height: 1.45;
  }

  .meta {
    margin: 0.65rem 0 0;
    color: #94a3b8;
    font-size: 0.8rem;
  }
</style>
