<script lang="ts">
  import { invoke, Channel } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import ChatMessage from "$lib/components/ChatMessage.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import DevTools from "$lib/components/DevTools.svelte";

  type AssistantResponse = {
    reply: string;
    explain?: string;
    undoable?: boolean;
    streamed?: boolean;
  };

  type Message = {
    role: "user" | "assistant";
    text: string;
    explain?: string;
    undoable?: boolean;
    isStreaming?: boolean;
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
  let isStreaming = $state(false);
  let isConnected = $state(false);
  let imageInfo = $state("");
  let engineStatus = $state<"unknown" | "ready" | "offline">("unknown");
  let showDevTools = $state(false);
  let chatEl = $state<HTMLElement | undefined>(undefined);
  let textareaEl = $state<HTMLTextAreaElement | undefined>(undefined);

  // Auto-scroll chat to bottom whenever messages update
  $effect(() => {
    messages; // track reactive dependency
    if (chatEl) chatEl.scrollTop = chatEl.scrollHeight;
  });

  onMount(async () => {
    await checkConnection();
    await checkEngineHealth();
  });

  async function checkConnection() {
    try {
      await invoke("mcp_list_tools");
      isConnected = true;
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

  async function checkEngineHealth() {
    try {
      const healthy = await invoke<boolean>("engine_health");
      engineStatus = healthy ? "ready" : "offline";
    } catch {
      engineStatus = "offline";
    }
  }

  async function sendChat(text?: string) {
    const trimmed = (text ?? input).trim();
    if (!trimmed || isSending) return;
    input = "";
    resetTextareaHeight();
    messages = [...messages, { role: "user", text: trimmed }];
    isSending = true;

    const streamIdx = messages.length;
    messages = [...messages, { role: "assistant", text: "", isStreaming: true }];

    const channel = new Channel<string>();
    let accumulated = "";

    channel.onmessage = (token: string) => {
      accumulated += token;
      isStreaming = true;
      messages = messages.map((m, i) =>
        i === streamIdx ? { ...m, text: accumulated } : m
      );
    };

    try {
      const result = await invoke<AssistantResponse>("assistant_chat_stream", {
        prompt: trimmed,
        onToken: channel,
      });

      const finalText = result.streamed
        ? accumulated
        : (result.reply || accumulated || "Done.");

      messages = messages.map((m, i) =>
        i === streamIdx
          ? {
              ...m,
              text: finalText,
              explain: result.explain,
              undoable: result.undoable ?? false,
              isStreaming: false,
            }
          : m
      );

      isConnected = true;
      await checkConnection();
    } catch (e) {
      messages = messages.map((m, i) =>
        i === streamIdx
          ? { ...m, text: "Error: " + String(e), isStreaming: false }
          : m
      );
      isConnected = false;
    } finally {
      isSending = false;
      isStreaming = false;
    }
  }

  async function cancelGeneration() {
    try {
      await invoke("engine_cancel");
    } catch {
      // ignore cancel errors
    }
  }

  async function undoLast() {
    try {
      await invoke("macro_undo");
      messages = [...messages, { role: "assistant", text: "\u21a9 Last change undone." }];
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
</script>

<div class="app">
  <!-- Header -->
  <header>
    <div class="header-left">
      <span class="app-icon">🎨</span>
      <div class="header-titles">
        <span class="app-name">GIMP Assistant</span>
        <span class="app-sub">AI-powered image editing</span>
      </div>
    </div>
    <div class="header-right">
      <StatusBar {engineStatus} {isConnected} {imageInfo} />
      <button
        class="icon-btn"
        onclick={() => (showDevTools = !showDevTools)}
        title="Developer tools"
        class:active={showDevTools}
      >⚙</button>
    </div>
  </header>

  <!-- Main -->
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
        <ChatMessage {msg} onUndo={undoLast} />
      {/each}

      <!-- Typing indicator -->
      {#if isSending && !isStreaming && messages[messages.length - 1]?.text === ""}
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
        placeholder="Ask me to edit your image... (Enter to send)"
        disabled={isSending}
        rows="1"
      ></textarea>
      {#if isSending && isStreaming}
        <button
          class="stop-btn"
          onclick={cancelGeneration}
          title="Stop generation"
        >■</button>
      {:else}
        <button
          class="send-btn"
          onclick={() => sendChat()}
          disabled={isSending || !input.trim()}
          title="Send"
        >↑</button>
      {/if}
    </div>
  </div>

  <!-- Dev tools overlay -->
  {#if showDevTools}
    <DevTools onClose={() => (showDevTools = false)} />
  {/if}
</div>

<style>
  /* Reset / Base */
  :global(*, *::before, *::after) { box-sizing: border-box; }
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    background: #f0f0f5;
    -webkit-font-smoothing: antialiased;
  }

  /* App shell */
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: #fff;
    position: relative;
    overflow: hidden;
  }

  /* Header */
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

  /* Main layout */
  .main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* Chat area */
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

  /* Typing indicator (kept in page since it's not a reusable message) */
  .row {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    max-width: 100%;
  }
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
  .bubble {
    padding: 10px 14px;
    border-radius: 18px;
    font-size: 14px;
    line-height: 1.5;
    word-break: break-word;
    white-space: pre-wrap;
  }
  .bubble.assistant {
    background: #f0f0f5;
    color: #1a1a1a;
    border-bottom-left-radius: 5px;
  }
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

  /* Input bar */
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

  .stop-btn {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    border: none;
    background: #ef4444;
    color: #fff;
    font-size: 14px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: background 0.15s;
    line-height: 1;
  }
  .stop-btn:hover { background: #dc2626; }
</style>
