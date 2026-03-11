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

  let messages: Message[] = [
    {
      role: "assistant",
      text: "Hi! I'm your GIMP AI assistant. How can I help you?"
    }
  ];
  
  let input = "";
  let isSending = false;
  let imageInfo = 'No image';
  let isConnected = false;
  let showDevTools = false;

  // Status variables for dev tools
  let llmStatus = "Disconnected";
  let gimpStatus = "Disconnected";
  let llmTestResult = "";
  let toolsListResult = "";
  let toolCallResult = "";
  let actionLog: string[] = [];
  let planRunResult: string | null = null;

  function logAction(msg: string) {
    actionLog = [msg, ...actionLog].slice(0, 20);
  }

  async function testLlm() {
    try {
      llmStatus = "Checking...";
      const result = await invoke<string>("test_llm");
      llmTestResult = result;
      llmStatus = "Connected";
    } catch (e) {
      console.error(e);
      llmTestResult = String(e);
      llmStatus = "Error";
    }
  }

  async function listTools() {
    try {
      gimpStatus = "Checking MCP...";
      const result = await invoke<any>("mcp_list_tools");
      toolsListResult = JSON.stringify(result, null, 2);
      gimpStatus = "Connected";
    } catch (e) {
      console.error(e);
      toolsListResult = String(e);
      gimpStatus = "Disconnected";
    }
  }

  async function runDrawTestLine() {
    try {
      logAction("Running: Draw test line...");
      await invoke("macro_draw_line", { x1: 50, y1: 50, x2: 200, y2: 200 });
      logAction("✅ Draw line OK");
    } catch (e) {
      logAction("❌ Draw line failed: " + String(e));
    }
  }

  async function runCropSquare() {
    try {
      logAction("Running: Crop square...");
      await invoke("macro_crop_square");
      logAction("✅ Crop square OK");
    } catch (e) {
      logAction("❌ Crop square failed: " + String(e));
    }
  }

  async function runResize1024() {
    try {
      logAction("Running: Resize width to 1024...");
      await invoke("macro_resize", { width: 1024 });
      logAction("✅ Resize OK");
    } catch (e) {
      logAction("❌ Resize failed: " + String(e));
    }
  }

  async function sendChat() {
    const trimmed = input.trim();
    if (!trimmed || isSending) return;
    input = "";
    messages = [...messages, { role: "user", text: trimmed }];
    isSending = true;

    try {
      const result = await invoke<AssistantResponse>("assistant_request", { prompt: trimmed });
      messages = [...messages, { role: "assistant", text: result.reply || "Done.", explain: result.explain, undoable: result.undoable ?? false }];
      isConnected = true;
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
</script>

<div class="container">
  <header class="header">
    <h1>GIMP AI Assistant</h1>
    <button class="dev-toggle" on:click={() => (showDevTools = !showDevTools)}>
      {showDevTools ? "✕" : "⋯"}
    </button>
  </header>

  <div class="status-bar">
    <div class="status-item">
      <div class="status-dot" class:connected={isConnected}></div>
      <span>{imageInfo}</span>
    </div>
  </div>

  <main class="main-layout">
    <section class="chat-section">
      <div class="chat-container">
        {#each messages as msg}
          <div class="message {msg.role}">
            <div class="message-content">{msg.text}</div>
            {#if msg.explain}
              <div class="explain-box">
                <span class="explain-icon">💡</span>
                <p class="explain-text">{msg.explain}</p>
              </div>
            {/if}
            {#if msg.undoable}
              <button class="undo-btn" on:click={undoLast}>↩ Undo</button>
            {/if}
          </div>
        {/each}
      </div>

      <div class="input-container">
        <textarea
          placeholder="Type a message..."
          bind:value={input}
          on:keydown={handleKeydown}
          disabled={isSending}
        ></textarea>
        <button on:click={sendChat} disabled={isSending || !input.trim()}>
          {isSending ? "..." : "↑"}
        </button>
      </div>
    </section>

    {#if showDevTools}
      <aside class="sidebar">
        <div class="sidebar-section">
          <h2>Developer Tools</h2>
          
          <details class="dev-panel">
            <summary>LLM Status: {llmStatus}</summary>
            <button class="dev-button" on:click={testLlm}>Test Connection</button>
            {#if llmTestResult}<pre>{llmTestResult}</pre>{/if}
          </details>

          <details class="dev-panel">
            <summary>GIMP Status: {gimpStatus}</summary>
            <button class="dev-button" on:click={listTools}>Refresh Tools</button>
            {#if toolsListResult}<pre>{toolsListResult}</pre>{/if}
          </details>

          <details class="dev-panel" open>
            <summary>Quick Actions</summary>
            <div class="button-grid">
              <button class="dev-button" on:click={runDrawTestLine}>✏️ Line</button>
              <button class="dev-button" on:click={runCropSquare}>✂️ Crop</button>
              <button class="dev-button" on:click={runResize1024}>📐 Resize</button>
            </div>
            {#if actionLog.length > 0}
              <pre class="log">{actionLog.join("\n")}</pre>
            {/if}
          </details>
        </div>
      </aside>
    {/if}
  </main>
</div>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, sans-serif;
    background: #f5f5f7;
  }
  .container { display: flex; flex-direction: column; height: 100vh; background: white; }
  .header { padding: 16px; border-bottom: 1px solid #ddd; display: flex; justify-content: space-between; align-items: center; }
  .main-layout { display: flex; flex: 1; overflow: hidden; }
  .chat-section { flex: 1; display: flex; flex-direction: column; }
  .chat-container { flex: 1; overflow-y: auto; padding: 20px; display: flex; flex-direction: column; gap: 10px; }
  .sidebar { width: 300px; border-left: 1px solid #ddd; background: #fafafa; padding: 15px; overflow-y: auto; }
  .message.user { align-self: flex-end; background: #007aff; color: white; border-radius: 15px; padding: 10px; }
  .message.assistant { align-self: flex-start; background: #e9e9eb; border-radius: 15px; padding: 10px; }
  .input-container { padding: 20px; border-top: 1px solid #ddd; display: flex; gap: 10px; }
  textarea { flex: 1; border-radius: 10px; padding: 10px; border: 1px solid #ccc; resize: none; }
  .dev-panel { margin-bottom: 10px; padding: 5px; border-bottom: 1px solid #eee; }
  .dev-button { margin-top: 5px; cursor: pointer; }
  .button-grid { display: flex; gap: 5px; flex-wrap: wrap; }
  pre { font-size: 10px; background: #eee; padding: 5px; border-radius: 5px; }
  .undo-btn { display: block; margin-top: 6px; font-size: 11px; background: rgba(255,255,255,0.25); border: 1px solid rgba(255,255,255,0.4); border-radius: 8px; padding: 3px 10px; cursor: pointer; color: inherit; }
  .undo-btn:hover { background: rgba(255,255,255,0.45); }
  .explain-box { display: flex; gap: 8px; align-items: flex-start; margin-top: 8px; background: #fffbea; border-left: 3px solid #f5c842; border-radius: 6px; padding: 8px 10px; }
  .explain-icon { font-size: 14px; flex-shrink: 0; margin-top: 1px; }
  .explain-text { margin: 0; font-size: 12px; color: #5a4a00; line-height: 1.5; }
  .status-dot { width: 10px; height: 10px; border-radius: 50%; background: red; display: inline-block; }
  .status-dot.connected { background: green; }
</style>