<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  type AssistantResponse = {
    reply: string;
    explain?: string;
    undoable?: boolean;
    plan: any;
    tool_results: any[];
  };

  type Message = {
    role: "user" | "assistant";
    text: string;
    explain?: string;
    undoable?: boolean;
  };

  const SUGGESTIONS = [
    "Draw a red circle",
    "Increase brightness",
    "Blur the image",
    "Draw a blue heart",
    "Blur the top half",
    "Brighten the bottom half",
  ];

  let messages = $state<Message[]>([]);
  let input = $state("");
  let isSending = $state(false);
  let isConnected = $state(false);
  let imageInfo = $state("");
  let showDevTools = $state(false);
  let chatEl = $state<HTMLElement | undefined>(undefined);
  let textareaEl = $state<HTMLTextAreaElement | undefined>(undefined);

  // Dev tools state
  let llmStatus = $state("Unknown");
  let gimpStatus = $state("Unknown");
  let llmTestResult = $state("");
  let toolsListResult = $state("");
  let actionLog = $state<string[]>([]);

  function logAction(msg: string) {
    actionLog = [msg, ...actionLog].slice(0, 20);
  }

  // Auto-scroll chat to bottom whenever messages update
  $effect(() => {
    messages; // track reactive dependency
    if (chatEl) chatEl.scrollTop = chatEl.scrollHeight;
  });

  onMount(async () => {
    await checkConnection();
  });

  async function checkConnection() {
    try {
      await invoke("mcp_list_tools");
      isConnected = true;
      // Try to read the open image name
      try {
        const res = await invoke<any>("mcp_call_tool", {
          name: "call_api",
          arguments: {
            api_path: "exec",
            args: ["pyGObject-console", [
              "from gi.repository import Gimp",
              "imgs = Gimp.get_images()",
              "print(imgs[0].get_name() if imgs else '__none__')"
            ]],
            kwargs: {}
          }
        });
        const txt = JSON.stringify(res);
        const match = txt.match(/([^/\\"]+\.(xcf|png|jpe?g|tiff?|bmp|gif|webp))/i);
        imageInfo = match ? match[1] : "Image open";
      } catch {
        imageInfo = "No image open";
      }
    } catch {
      isConnected = false;
      imageInfo = "";
    }
  }

  async function sendChat(text?: string) {
    const trimmed = (text ?? input).trim();
    if (!trimmed || isSending) return;
    input = "";
    resetTextareaHeight();
    messages = [...messages, { role: "user", text: trimmed }];
    isSending = true;

    try {
      const result = await invoke<AssistantResponse>("assistant_request", { prompt: trimmed });
      messages = [...messages, {
        role: "assistant",
        text: result.reply || "Done.",
        explain: result.explain,
        undoable: result.undoable ?? false
      }];
      isConnected = true;
      await checkConnection();
    } catch (e) {
      messages = [...messages, { role: "assistant", text: "Error: " + String(e) }];
      isConnected = false;
    } finally {
      isSending = false;
    }
  }

  async function undoLast() {
    try {
      await invoke("macro_undo");
      messages = [...messages, { role: "assistant", text: "↩ Last change undone." }];
    } catch (e) {
      messages = [...messages, { role: "assistant", text: "Undo failed: " + String(e) }];
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void sendChat();
    }
  }

  function growTextarea() {
    if (!textareaEl) return;
    textareaEl.style.height = "auto";
    textareaEl.style.height = Math.min(textareaEl.scrollHeight, 120) + "px";
  }

  function resetTextareaHeight() {
    if (!textareaEl) return;
    textareaEl.style.height = "auto";
  }

  // Dev tools helpers
  async function testLlm() {
    llmStatus = "Checking…";
    try {
      const result = await invoke<string>("test_llm");
      llmTestResult = result;
      llmStatus = "Connected";
    } catch (e) {
      llmTestResult = String(e);
      llmStatus = "Error";
    }
  }

  async function listTools() {
    gimpStatus = "Checking…";
    try {
      const result = await invoke<any>("mcp_list_tools");
      toolsListResult = JSON.stringify(result, null, 2);
      gimpStatus = "Connected";
    } catch (e) {
      toolsListResult = String(e);
      gimpStatus = "Disconnected";
    }
  }

  async function runDrawTestLine() {
    logAction("Draw test line…");
    try {
      await invoke("macro_draw_line", { x1: 50, y1: 50, x2: 200, y2: 200 });
      logAction("✅ Line OK");
    } catch (e) { logAction("❌ " + String(e)); }
  }

  async function runCropSquare() {
    logAction("Crop square…");
    try {
      await invoke("macro_crop_square");
      logAction("✅ Crop OK");
    } catch (e) { logAction("❌ " + String(e)); }
  }

  async function runResize1024() {
    logAction("Resize to 1024w…");
    try {
      await invoke("macro_resize", { width: 1024 });
      logAction("✅ Resize OK");
    } catch (e) { logAction("❌ " + String(e)); }
  }
</script>

<div class="app">
  <!-- ── Header ── -->
  <header>
    <div class="header-left">
      <span class="app-icon">🎨</span>
      <div class="header-titles">
        <span class="app-name">GIMP Assistant</span>
        <span class="app-sub">AI-powered image editing</span>
      </div>
    </div>
    <div class="header-right">
      <div class="status-pill" class:connected={isConnected}>
        <span class="status-dot"></span>
        <span class="status-label">
          {isConnected ? (imageInfo || "GIMP connected") : "GIMP offline"}
        </span>
      </div>
      <button
        class="icon-btn"
        onclick={() => (showDevTools = !showDevTools)}
        title="Developer tools"
        class:active={showDevTools}
      >⚙</button>
    </div>
  </header>

  <!-- ── Main ── -->
  <div class="main">
    <!-- Chat -->
    <div class="chat" bind:this={chatEl}>

      <!-- Empty state with suggestion chips -->
      {#if messages.length === 0}
        <div class="empty-state">
          <div class="empty-icon">✦</div>
          <p class="empty-title">What would you like to do?</p>
          <p class="empty-sub">Type a command or try one of these:</p>
          <div class="chips">
            {#each SUGGESTIONS as s}
              <button class="chip" onclick={() => sendChat(s)}>{s}</button>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Messages -->
      {#each messages as msg}
        <div class="row {msg.role}">
          {#if msg.role === "assistant"}
            <div class="avatar">✦</div>
          {/if}
          <div class="col">
            <div class="bubble {msg.role}">{msg.text}</div>
            {#if msg.explain}
              <div class="tip">
                <span class="tip-icon">💡</span>
                <span class="tip-text">{msg.explain}</span>
              </div>
            {/if}
            {#if msg.undoable}
              <button class="undo-btn" onclick={undoLast}>↩ Undo</button>
            {/if}
          </div>
        </div>
      {/each}

      <!-- Typing indicator -->
      {#if isSending}
        <div class="row assistant">
          <div class="avatar">✦</div>
          <div class="col">
            <div class="bubble assistant typing">
              <span></span><span></span><span></span>
            </div>
          </div>
        </div>
      {/if}
    </div>

    <!-- Input bar -->
    <div class="input-bar">
      <textarea
        bind:this={textareaEl}
        bind:value={input}
        onkeydown={handleKeydown}
        oninput={growTextarea}
        placeholder="Ask me to edit your image… (Enter to send)"
        disabled={isSending}
        rows="1"
      ></textarea>
      <button
        class="send-btn"
        onclick={() => sendChat()}
        disabled={isSending || !input.trim()}
        title="Send"
      >↑</button>
    </div>
  </div>

  <!-- ── Dev tools overlay ── -->
  {#if showDevTools}
    <aside class="devtools">
      <div class="devtools-header">
        <span>Developer Tools</span>
        <button class="icon-btn" onclick={() => (showDevTools = false)}>✕</button>
      </div>

      <details class="dev-section">
        <summary>LLM · <em>{llmStatus}</em></summary>
        <button class="dev-btn" onclick={testLlm}>Test Connection</button>
        {#if llmTestResult}<pre>{llmTestResult}</pre>{/if}
      </details>

      <details class="dev-section">
        <summary>GIMP MCP · <em>{gimpStatus}</em></summary>
        <button class="dev-btn" onclick={listTools}>Refresh Tools</button>
        {#if toolsListResult}<pre>{toolsListResult}</pre>{/if}
      </details>

      <details class="dev-section" open>
        <summary>Quick Actions</summary>
        <div class="dev-actions">
          <button class="dev-btn" onclick={runDrawTestLine}>✏️ Line</button>
          <button class="dev-btn" onclick={runCropSquare}>✂️ Crop</button>
          <button class="dev-btn" onclick={runResize1024}>📐 1024w</button>
        </div>
        {#if actionLog.length > 0}
          <pre class="action-log">{actionLog.join("\n")}</pre>
        {/if}
      </details>
    </aside>
  {/if}
</div>

<style>
  /* ── Reset / Base ── */
  :global(*, *::before, *::after) { box-sizing: border-box; }
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    background: #f0f0f5;
    -webkit-font-smoothing: antialiased;
  }

  /* ── App shell ── */
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: #fff;
    position: relative;
    overflow: hidden;
  }

  /* ── Header ── */
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid #e8e8ec;
    background: #fff;
    flex-shrink: 0;
    gap: 10px;
  }
  .header-left {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .app-icon { font-size: 22px; line-height: 1; }
  .header-titles {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .app-name {
    font-size: 15px;
    font-weight: 600;
    color: #1a1a1a;
    line-height: 1.2;
  }
  .app-sub {
    font-size: 11px;
    color: #999;
    line-height: 1.2;
  }
  .header-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  /* Status pill */
  .status-pill {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-radius: 20px;
    background: #f5f5f5;
    border: 1px solid #e0e0e0;
    font-size: 12px;
    color: #888;
    transition: background 0.2s, color 0.2s;
  }
  .status-pill.connected {
    background: #f0faf3;
    border-color: #b8e6c6;
    color: #2a7a45;
  }
  .status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #ccc;
    flex-shrink: 0;
    transition: background 0.2s;
  }
  .status-pill.connected .status-dot { background: #34c759; }
  .status-label { white-space: nowrap; max-width: 140px; overflow: hidden; text-overflow: ellipsis; }

  /* Icon button */
  .icon-btn {
    width: 32px;
    height: 32px;
    border: 1px solid #e0e0e0;
    border-radius: 8px;
    background: #fafafa;
    font-size: 15px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #666;
    transition: background 0.15s, color 0.15s;
    flex-shrink: 0;
  }
  .icon-btn:hover { background: #f0f0f0; color: #333; }
  .icon-btn.active { background: #007aff; border-color: #007aff; color: #fff; }

  /* ── Main layout ── */
  .main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* ── Chat area ── */
  .chat {
    flex: 1;
    overflow-y: auto;
    padding: 20px 16px 8px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    scroll-behavior: smooth;
  }

  /* Empty state */
  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 40px 20px;
    color: #888;
  }
  .empty-icon {
    font-size: 36px;
    color: #007aff;
    margin-bottom: 12px;
    opacity: 0.6;
  }
  .empty-title {
    font-size: 17px;
    font-weight: 600;
    color: #333;
    margin: 0 0 6px;
  }
  .empty-sub {
    font-size: 13px;
    color: #999;
    margin: 0 0 18px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    justify-content: center;
    max-width: 360px;
  }
  .chip {
    padding: 7px 14px;
    border-radius: 20px;
    border: 1px solid #d0d0d8;
    background: #fff;
    font-size: 13px;
    color: #444;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s, color 0.15s;
    white-space: nowrap;
  }
  .chip:hover {
    background: #007aff;
    border-color: #007aff;
    color: #fff;
  }

  /* Message rows */
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

  /* Bubbles */
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

  /* Typing indicator */
  .bubble.typing {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 12px 16px;
  }
  .bubble.typing span {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #999;
    display: inline-block;
    animation: bounce 1.2s infinite;
  }
  .bubble.typing span:nth-child(2) { animation-delay: 0.2s; }
  .bubble.typing span:nth-child(3) { animation-delay: 0.4s; }
  @keyframes bounce {
    0%, 60%, 100% { transform: translateY(0); opacity: 0.5; }
    30% { transform: translateY(-5px); opacity: 1; }
  }

  /* Explain tip */
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

  /* Undo button */
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

  /* ── Input bar ── */
  .input-bar {
    display: flex;
    align-items: flex-end;
    gap: 10px;
    padding: 12px 16px;
    border-top: 1px solid #e8e8ec;
    background: #fff;
  }
  textarea {
    flex: 1;
    border: 1px solid #d8d8e0;
    border-radius: 14px;
    padding: 10px 14px;
    font-size: 14px;
    font-family: inherit;
    resize: none;
    line-height: 1.5;
    outline: none;
    transition: border-color 0.15s;
    background: #fafafa;
    max-height: 120px;
    overflow-y: auto;
  }
  textarea:focus { border-color: #007aff; background: #fff; }
  textarea:disabled { opacity: 0.5; }

  .send-btn {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    border: none;
    background: #007aff;
    color: #fff;
    font-size: 18px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: background 0.15s, opacity 0.15s;
    line-height: 1;
  }
  .send-btn:hover:not(:disabled) { background: #0060df; }
  .send-btn:disabled { opacity: 0.35; cursor: default; }

  /* ── Dev tools overlay ── */
  .devtools {
    position: absolute;
    top: 57px;
    right: 0;
    bottom: 0;
    width: 280px;
    background: #fff;
    border-left: 1px solid #e0e0e8;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    z-index: 10;
    box-shadow: -4px 0 16px rgba(0,0,0,0.06);
  }
  .devtools-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 14px;
    border-bottom: 1px solid #eee;
    font-size: 13px;
    font-weight: 600;
    color: #333;
    flex-shrink: 0;
  }
  .dev-section {
    border-bottom: 1px solid #f0f0f0;
    font-size: 12px;
  }
  .dev-section > summary {
    padding: 10px 14px;
    cursor: pointer;
    user-select: none;
    color: #444;
    font-weight: 500;
    list-style: none;
  }
  .dev-section > summary::-webkit-details-marker { display: none; }
  .dev-section > summary::before { content: "▶ "; font-size: 9px; }
  .dev-section[open] > summary::before { content: "▼ "; }
  .dev-section em { font-style: normal; color: #888; font-weight: 400; }
  .dev-section > :not(summary) { padding: 0 14px 10px; }
  .dev-btn {
    margin: 4px 0;
    padding: 5px 10px;
    font-size: 12px;
    border-radius: 6px;
    border: 1px solid #d0d0d8;
    background: #fafafa;
    cursor: pointer;
    transition: background 0.1s;
  }
  .dev-btn:hover { background: #f0f0f0; }
  .dev-actions { display: flex; gap: 6px; flex-wrap: wrap; padding-top: 4px; }
  pre {
    font-size: 10px;
    background: #f4f4f8;
    border-radius: 6px;
    padding: 8px;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
    margin: 6px 0 0;
    color: #333;
  }
  .action-log { margin: 8px 0 0; }
</style>
