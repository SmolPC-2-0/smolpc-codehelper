<script lang="ts">
  interface Props {
    onSend: (message: string) => void;
    disabled?: boolean;
  }

  let { onSend, disabled = false }: Props = $props();
  let inputValue = $state('');

  function handleSubmit(): void {
    const trimmed = inputValue.trim();
    if (!trimmed || disabled) {
      return;
    }

    onSend(trimmed);
    inputValue = '';
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      handleSubmit();
    }
  }
</script>

<div class="chat-input-container">
  <textarea
    bind:value={inputValue}
    onkeydown={handleKeydown}
    placeholder={disabled
      ? 'Workflow in progress...'
      : 'Type your message (Enter to send, Shift+Enter for newline)'}
    {disabled}
    rows="3"
  ></textarea>
  <button onclick={handleSubmit} disabled={disabled || !inputValue.trim()}>
    {disabled ? 'Running...' : 'Send'}
  </button>
</div>

<style>
  .chat-input-container {
    display: flex;
    gap: 0.75rem;
    padding: 1rem;
    border-top: 1px solid #334155;
    background: #0f172a;
  }

  textarea {
    flex: 1;
    padding: 0.75rem;
    border-radius: 8px;
    border: 1px solid #334155;
    background: #020617;
    color: #e2e8f0;
    resize: vertical;
    min-height: 64px;
    font: inherit;
  }

  textarea:focus {
    outline: none;
    border-color: #38bdf8;
  }

  textarea:disabled {
    opacity: 0.7;
    cursor: not-allowed;
  }

  button {
    align-self: flex-end;
    border: 1px solid #0ea5e9;
    border-radius: 8px;
    background: #0ea5e9;
    color: #082f49;
    font-weight: 700;
    padding: 0.7rem 1rem;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
</style>
