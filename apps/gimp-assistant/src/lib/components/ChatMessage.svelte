<script lang="ts">
  type Message = {
    role: "user" | "assistant";
    text: string;
    explain?: string;
    undoable?: boolean;
    isStreaming?: boolean;
  };

  let { msg, onUndo }: { msg: Message; onUndo: () => void } = $props();
</script>

<div class="row {msg.role}">
  {#if msg.role === "assistant"}
    <div class="avatar">✦</div>
  {/if}
  <div class="col">
    <div class="bubble {msg.role}">
      {msg.text}{#if msg.isStreaming}<span class="cursor">|</span>{/if}
    </div>
    {#if msg.explain}
      <div class="tip">
        <span class="tip-icon">💡</span>
        <span class="tip-text">{msg.explain}</span>
      </div>
    {/if}
    {#if msg.undoable}
      <button class="undo-btn" onclick={onUndo}>↩ Undo</button>
    {/if}
  </div>
</div>

<style>
  .row {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    max-width: 100%;
  }
  .row.user { flex-direction: row-reverse; }

  .avatar {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: #007aff;
    color: #fff;
    font-size: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    margin-bottom: 2px;
  }

  .col {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    max-width: 72%;
    gap: 6px;
  }
  .row.user .col { align-items: flex-end; }

  .bubble {
    padding: 10px 14px;
    border-radius: 18px;
    font-size: 14px;
    line-height: 1.5;
    word-break: break-word;
    white-space: pre-wrap;
  }
  .bubble.user {
    background: #007aff;
    color: #fff;
    border-bottom-right-radius: 5px;
  }
  .bubble.assistant {
    background: #f0f0f5;
    color: #1a1a1a;
    border-bottom-left-radius: 5px;
  }

  .cursor {
    animation: blink 0.8s step-end infinite;
    font-weight: 300;
    color: #007aff;
  }
  @keyframes blink {
    0%, 100% { opacity: 1; }
    50% { opacity: 0; }
  }

  .tip {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    background: #fffbea;
    border-left: 3px solid #f5c842;
    border-radius: 8px;
    padding: 8px 12px;
    max-width: 100%;
  }
  .tip-icon { font-size: 13px; flex-shrink: 0; margin-top: 2px; }
  .tip-text { font-size: 12px; color: #5a4a00; line-height: 1.5; margin: 0; }

  .undo-btn {
    font-size: 12px;
    padding: 4px 12px;
    border-radius: 12px;
    border: 1px solid rgba(0, 122, 255, 0.3);
    background: rgba(0, 122, 255, 0.06);
    color: #007aff;
    cursor: pointer;
    transition: background 0.15s;
  }
  .undo-btn:hover { background: rgba(0, 122, 255, 0.14); }
</style>
