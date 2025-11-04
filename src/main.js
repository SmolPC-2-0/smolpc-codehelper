// Tauri v2 API (access via injected globals)
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// ===== STATE MANAGEMENT =====
let chats = [];
let activeChat = null;
let contextEnabled = true;
let streamListenerRegistered = false;
let streamUnlisten = null;
let streamingActive = false;
let activeAssistantIndex = null;
let activeAssistantDomId = null;

// ===== INITIALIZATION =====
window.addEventListener("DOMContentLoaded", async () => {
  loadChatsFromStorage();
  setupEventListeners();
  await initStreamListener();
  checkOllamaStatus();

  // If no active chat, create one
  if (!activeChat) {
    createNewChat();
  } else {
    loadChat(activeChat.id);
  }
});

// ===== OLLAMA STATUS CHECK =====
async function checkOllamaStatus() {
  try {
    await invoke("check_ollama");
    showStatus("Ready", "success");
  } catch (error) {
    showStatus("⚠️ Ollama not running! Please start Ollama first.", "error");
  }
}

// ===== EVENT LISTENERS =====
function setupEventListeners() {
  // Sidebar toggle
  document
    .getElementById("sidebarToggle")
    .addEventListener("click", toggleSidebar);

  // New chat buttons
  document
    .getElementById("newChatBtn")
    .addEventListener("click", createNewChat);
  document
    .getElementById("newChatBtnHeader")
    .addEventListener("click", createNewChat);

  // Context toggle
  document
    .getElementById("contextToggle")
    .addEventListener("click", toggleContext);

  // Send message
  document.getElementById("sendBtn").addEventListener("click", sendMessage);

  // Input events
  const input = document.getElementById("messageInput");
  input.addEventListener("keydown", handleInputKeydown);
  input.addEventListener("input", autoResizeInput);

  // Example buttons
  document.querySelectorAll(".example-btn").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      const example = e.target.getAttribute("data-example");
      document.getElementById("messageInput").value = example;
      document.getElementById("messageInput").focus();
    });
  });
}

// ===== STREAMING EVENTS =====
async function initStreamListener() {
  if (streamListenerRegistered) return;

  try {
    // use the v2 API you imported
    streamUnlisten = await listen("llm_chunk", (event) => {
      if (!event || !event.payload) return;
      const { text = "", done = false } = event.payload;

      // micrologs: super helpful when debugging the “spinner never stops”
      console.debug("[llm_chunk]", { len: text.length, done });

      if (text) {
        appendToActiveAssistantMessage(text);
      }
      if (done) {
        finalizeAssistantMessage(true);
      }
    });

    streamListenerRegistered = true;
    console.debug("[stream] listener registered");
  } catch (error) {
    console.error("[stream] Failed to initialize stream listener:", error);
    streamListenerRegistered = false;
  }
}

function setStreamingState(isStreaming) {
  console.debug("[ui] setStreamingState", isStreaming);
  streamingActive = isStreaming;
  const input = document.getElementById("messageInput");
  const sendBtn = document.getElementById("sendBtn");

  if (input) {
    input.disabled = isStreaming;
  }
  if (sendBtn) {
    sendBtn.disabled = isStreaming;
  }

  showLoading(isStreaming);

  if (!isStreaming && input) {
    input.focus();
  }
}

function startAssistantMessage() {
  if (!activeChat) return;

  // If we’re already streaming into a bubble, don’t create another
  if (activeAssistantIndex !== null && activeAssistantDomId) return;
  console.debug(
    "[ui] startAssistantMessage -> index",
    activeAssistantIndex,
    "domId",
    activeAssistantDomId
  );

  // 1) Update state
  addMessage("assistant", ""); // pushes into activeChat.messages and persists
  activeAssistantIndex = activeChat.messages.length - 1;

  // 2) Create a stable DOM node for this assistant message (exactly once)
  activeAssistantDomId = `msg-${activeChat.id}-${activeAssistantIndex}`;

  const container = document.getElementById("messagesContainer");
  const bubble = document.createElement("div");
  bubble.className = "message assistant";
  bubble.dataset.mid = activeAssistantDomId;
  bubble.innerHTML = `
    <div class="message-avatar">🤖</div>
    <div class="message-content">
      <div class="message-header">
        <span class="message-role">AI Assistant</span>
        <span class="message-time">${formatTime(
          new Date().toISOString()
        )}</span>
      </div>
      <div class="message-text"></div>
      <div class="message-actions">
        <button class="message-action-btn" onclick="copyMessage(${activeAssistantIndex})">📋 Copy</button>
        <button class="message-action-btn" onclick="saveMessage(${activeAssistantIndex})">💾 Save</button>
      </div>
    </div>
  `;
  container.appendChild(bubble);
  scrollToBottom();
}

function appendToActiveAssistantMessage(text) {
  if (!activeChat) return;
  if (activeAssistantIndex === null) {
    // If we somehow get a chunk with no active bubble, create it once.
    startAssistantMessage();
  }
  console.debug("[ui] append", {
    chars: text.length,
    index: activeAssistantIndex,
  });

  const message = activeChat.messages[activeAssistantIndex];
  if (!message) return;

  // Update state
  message.content = (message.content || "") + text;
  activeChat.lastUpdated = new Date().toISOString();

  // Update only the DOM node for this live assistant message
  const container = document.getElementById("messagesContainer");
  let node = null;

  if (activeAssistantDomId) {
    node = container.querySelector(
      `.message.assistant[data-mid="${activeAssistantDomId}"] .message-text`
    );
  }
  if (activeAssistantIndex === null) {
    startAssistantMessage();
  }
  // Fallback (shouldn’t happen): find last assistant’s .message-text
  if (!node) {
    const nodes = container.querySelectorAll(
      ".message.assistant .message-text"
    );
    node = nodes[nodes.length - 1];
  }

  if (node) {
    // Streaming-time: keep it fast — append plain text only
    node.textContent += text;
  }

  // Keep scroll pinned
  scrollToBottom();

  // Persist (cheap enough; you can throttle later)
  saveChatsToStorage();
}

function finalizeAssistantMessage(success = true) {
  console.debug("[ui] finalizeAssistantMessage", {
    success,
    index: activeAssistantIndex,
    domId: activeAssistantDomId,
  });

  // Parse & render markdown exactly once at the end
  if (activeChat && activeAssistantIndex !== null) {
    const msg = activeChat.messages[activeAssistantIndex] || { content: "" };
    const cleaned = (msg.content || "").replace(/\n{3,}/g, "\n\n");

    const container = document.getElementById("messagesContainer");
    let node = null;
    if (activeAssistantDomId) {
      node = container.querySelector(
        `.message.assistant[data-mid="${activeAssistantDomId}"] .message-text`
      );
    }
    if (!node) {
      const nodes = container.querySelectorAll(
        ".message.assistant .message-text"
      );
      node = nodes[nodes.length - 1];
    }

    if (node) {
      if (typeof marked !== "undefined") {
        node.innerHTML = marked.parse(cleaned);
      } else {
        let formatted = escapeHtml(cleaned)
          .replace(
            /```(\w+)?\n([\s\S]*?)```/g,
            (m, lang, code) => `<pre><code>${code.trim()}</code></pre>`
          )
          .replace(/`([^`]+)`/g, "<code>$1</code>")
          .replace(/\n/g, "<br>");
        node.innerHTML = formatted;
      }
    }

    activeChat.lastUpdated = new Date().toISOString();
    saveChatsToStorage();
  }

  // Optional: refresh sidebar timestamps AFTER streaming completes
  renderChatList();

  // Reset streaming state
  activeAssistantIndex = null;
  activeAssistantDomId = null;
  setStreamingState(false);

  if (success) showStatus("Response generated", "success");
}

// ===== SIDEBAR FUNCTIONS =====
function toggleSidebar() {
  const sidebar = document.getElementById("sidebar");
  sidebar.classList.toggle("collapsed");
}

function renderChatList() {
  const chatList = document.getElementById("chatList");
  chatList.innerHTML = "";

  if (chats.length === 0) {
    chatList.innerHTML =
      '<div style="padding: 20px; text-align: center; color: #94a3b8;">No chats yet. Start a new conversation!</div>';
    return;
  }

  // Group chats by date
  const groups = groupChatsByDate(chats);

  Object.keys(groups).forEach((groupName) => {
    const groupDiv = document.createElement("div");
    groupDiv.className = "chat-group";

    const groupTitle = document.createElement("div");
    groupTitle.className = "chat-group-title";
    groupTitle.textContent = groupName;
    groupDiv.appendChild(groupTitle);

    groups[groupName].forEach((chat) => {
      const chatItem = createChatItem(chat);
      groupDiv.appendChild(chatItem);
    });

    chatList.appendChild(groupDiv);
  });
}

function createChatItem(chat) {
  const item = document.createElement("div");
  item.className = "chat-item";
  item.dataset.chatId = chat.id;

  if (activeChat && activeChat.id === chat.id) {
    item.classList.add("active");
  }

  item.innerHTML = `
        <span class="chat-item-icon">💬</span>
        <div class="chat-item-content">
            <div class="chat-item-title" data-chat-id="${chat.id}">${escapeHtml(
    chat.title
  )}</div>
            <div class="chat-item-time">${formatTime(chat.lastUpdated)}</div>
        </div>
        <div class="chat-item-actions">
            <button class="chat-action-btn" onclick="renameChat('${
              chat.id
            }')" title="Rename">✏️</button>
            <button class="chat-action-btn" onclick="deleteChat('${
              chat.id
            }')" title="Delete">🗑️</button>
        </div>
    `;

  item.addEventListener("click", (e) => {
    if (!e.target.classList.contains("chat-action-btn")) {
      loadChat(chat.id);
    }
  });

  return item;
}

function groupChatsByDate(chats) {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  const lastWeek = new Date(today);
  lastWeek.setDate(lastWeek.getDate() - 7);

  const groups = {
    Today: [],
    Yesterday: [],
    "Last 7 Days": [],
    Older: [],
  };

  chats.forEach((chat) => {
    const chatDate = new Date(chat.lastUpdated);
    const chatDay = new Date(
      chatDate.getFullYear(),
      chatDate.getMonth(),
      chatDate.getDate()
    );

    if (chatDay.getTime() === today.getTime()) {
      groups["Today"].push(chat);
    } else if (chatDay.getTime() === yesterday.getTime()) {
      groups["Yesterday"].push(chat);
    } else if (chatDay >= lastWeek) {
      groups["Last 7 Days"].push(chat);
    } else {
      groups["Older"].push(chat);
    }
  });

  // Remove empty groups
  Object.keys(groups).forEach((key) => {
    if (groups[key].length === 0) {
      delete groups[key];
    }
  });

  return groups;
}

// ===== CHAT MANAGEMENT =====
function createNewChat() {
  const chat = {
    id: generateId(),
    title: "New Chat",
    created: new Date().toISOString(),
    lastUpdated: new Date().toISOString(),
    messages: [],
  };

  chats.unshift(chat);
  activeChat = chat;
  activeAssistantIndex = null;
  streamingActive = false;
  setStreamingState(false);

  saveChatsToStorage();
  renderChatList();
  renderMessages();

  // Focus input
  document.getElementById("messageInput").focus();

  showStatus("New chat started", "success");
}

function loadChat(chatId) {
  const chat = chats.find((c) => c.id === chatId);
  if (!chat) return;

  activeChat = chat;
  activeAssistantIndex = null;
  streamingActive = false;
  setStreamingState(false);
  saveChatsToStorage();
  renderChatList();
  renderMessages();

  // Scroll to bottom
  setTimeout(scrollToBottom, 100);
}

function deleteChat(chatId) {
  if (!confirm("Delete this chat?")) return;

  chats = chats.filter((c) => c.id !== chatId);

  if (activeChat && activeChat.id === chatId) {
    if (chats.length > 0) {
      loadChat(chats[0].id);
    } else {
      createNewChat();
    }
  }

  saveChatsToStorage();
  renderChatList();
  showStatus("Chat deleted", "success");
}

function renameChat(chatId) {
  const chat = chats.find((c) => c.id === chatId);
  if (!chat) return;

  const titleElement = document.querySelector(
    `.chat-item-title[data-chat-id="${chatId}"]`
  );
  const currentTitle = chat.title;

  titleElement.classList.add("editing");
  titleElement.innerHTML = `<input type="text" class="chat-item-input" value="${escapeHtml(
    currentTitle
  )}" />`;

  const input = titleElement.querySelector("input");
  input.focus();
  input.select();

  const finishRename = () => {
    const newTitle = input.value.trim() || currentTitle;
    chat.title = newTitle;
    chat.lastUpdated = new Date().toISOString();
    saveChatsToStorage();
    renderChatList();
  };

  input.addEventListener("blur", finishRename);
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      finishRename();
    } else if (e.key === "Escape") {
      renderChatList();
    }
  });
}

// ===== MESSAGE HANDLING =====
async function sendMessage() {
  const input = document.getElementById("messageInput");
  const content = input.value.trim();

  if (!content) return;

  addMessage("user", content);
  input.value = "";
  autoResizeInput();

  if (activeChat.title === "New Chat") {
    activeChat.title = generateChatTitle(content);
    renderChatList();
  }

  const context = contextEnabled ? getContextMessages() : [];
  const model = document.getElementById("modelSelect").value;
  const fullPrompt = buildPromptFromContext(content, context);

  setStreamingState(true);
  try {
    await initStreamListener();
    await sendMessageStreaming(model, fullPrompt, null, null);
  } catch (error) {
    console.error("Failed to generate response:", error);
    const message =
      typeof error === "string" ? error : error?.message || String(error);
    if (activeAssistantIndex === null) {
      startAssistantMessage();
    }
    appendToActiveAssistantMessage(`Error: ${message}`);
    finalizeAssistantMessage(false);
    showStatus(`Error: ${message}`, "error");
  }

  input.focus();
}

function addMessage(role, content) {
  const message = {
    role: role,
    content: content,
    timestamp: new Date().toISOString(),
  };

  activeChat.messages.push(message);
  activeChat.lastUpdated = new Date().toISOString();

  saveChatsToStorage();
  renderMessages();
  scrollToBottom();
}

function getContextMessages() {
  if (!activeChat || !activeChat.messages) return [];

  // Return all messages for full context
  return activeChat.messages.map((msg) => ({
    role: msg.role,
    content: msg.content,
  }));
}

function buildPromptFromContext(userMessage, context) {
  if (!context || context.length === 0) {
    return userMessage;
  }

  let fullPrompt = "Previous conversation:\n\n";
  context.forEach((msg) => {
    const role = msg.role === "user" ? "Student" : "Assistant";
    fullPrompt += `${role}: ${msg.content}\n\n`;
  });
  fullPrompt += `Student: ${userMessage}`;
  return fullPrompt;
}

async function generateResponse(userMessage, context) {
  const model = document.getElementById("modelSelect").value;
  const fullPrompt = buildPromptFromContext(userMessage, context);

  return await invoke("generate_code_stream", {
    prompt: fullPrompt,
    model: model,
  });
}

async function sendMessageStreaming(model, prompt, system, options) {
  startAssistantMessage();

  if (!streamListenerRegistered) {
    console.warn(
      "[stream] listener not registered; falling back to non-streaming"
    );
    const text = await invoke("generate_code", {
      prompt,
      model,
      options: options ?? null,
    });
    appendToActiveAssistantMessage(text);
    finalizeAssistantMessage(true);
    return;
  }

  try {
    await invoke("generate_code_stream", {
      req: {
        model,
        prompt,
        system: system ?? null,
        options: options ?? null,
      },
    });
  } catch (error) {
    console.error("Streaming invocation failed:", error);
    const fallbackText = await invoke("generate_code", {
      prompt,
      model,
      options: options ?? null,
    });
    appendToActiveAssistantMessage(fallbackText);
    finalizeAssistantMessage(true);
  }
}

function renderMessages() {
  if (streamingActive) return;
  const container = document.getElementById("messagesContainer");
  const welcomeMsg = document.getElementById("welcomeMessage");

  if (!activeChat || activeChat.messages.length === 0) {
    container.innerHTML = "";
    welcomeMsg.style.display = "flex";
    return;
  }

  welcomeMsg.style.display = "none";
  container.innerHTML = "";

  activeChat.messages.forEach((msg, index) => {
    const messageDiv = createMessageElement(msg, index);
    container.appendChild(messageDiv);
  });
}

function createMessageElement(message, index) {
  const div = document.createElement("div");
  div.className = `message ${message.role}`;

  const avatar = message.role === "user" ? "👤" : "🤖";
  const roleName = message.role === "user" ? "You" : "AI Assistant";

  div.innerHTML = `
        <div class="message-avatar">${avatar}</div>
        <div class="message-content">
            <div class="message-header">
                <span class="message-role">${roleName}</span>
                <span class="message-time">${formatTime(
                  message.timestamp
                )}</span>
            </div>
            <div class="message-text">${formatMessageContent(
              message.content
            )}</div>
            ${
              message.role === "assistant"
                ? `
                <div class="message-actions">
                    <button class="message-action-btn" onclick="copyMessage(${index})">📋 Copy</button>
                    <button class="message-action-btn" onclick="saveMessage(${index})">💾 Save</button>
                </div>
            `
                : ""
            }
        </div>
    `;

  return div;
}

function formatMessageContent(content) {
  // Use marked.js to parse markdown
  if (typeof marked !== "undefined") {
    // Configure marked
    marked.setOptions({
      breaks: true, // Convert \n to <br>
      gfm: true, // GitHub Flavored Markdown
      headerIds: false,
      mangle: false,
    });

    // Clean up excessive blank lines before parsing
    // Replace 3+ newlines with just 2 (one blank line max)
    let cleaned = content.replace(/\n{3,}/g, "\n\n");

    return marked.parse(cleaned);
  }

  // Fallback if marked isn't loaded
  let formatted = escapeHtml(content);
  formatted = formatted.replace(
    /```(\w+)?\n([\s\S]*?)```/g,
    (match, lang, code) => {
      return `<pre><code>${code.trim()}</code></pre>`;
    }
  );
  formatted = formatted.replace(/`([^`]+)`/g, "<code>$1</code>");
  formatted = formatted.replace(/\n/g, "<br>");
  return formatted;
}

// ===== MESSAGE ACTIONS =====
async function copyMessage(index) {
  const message = activeChat.messages[index];
  try {
    await navigator.clipboard.writeText(message.content);
    showStatus("Copied to clipboard!", "success");
  } catch (error) {
    showStatus("Failed to copy", "error");
  }
}

async function saveMessage(index) {
  const message = activeChat.messages[index];
  try {
    await invoke("save_code", { content: message.content });
    showStatus("Saved successfully!", "success");
  } catch (error) {
    showStatus(`Failed to save: ${error}`, "error");
  }
}

// ===== CONTEXT TOGGLE =====
function toggleContext() {
  contextEnabled = !contextEnabled;

  const toggle = document
    .getElementById("contextToggle")
    .querySelector(".toggle-switch");
  const indicator = document.getElementById("contextIndicator");

  if (contextEnabled) {
    toggle.classList.add("active");
    indicator.classList.remove("off");
    indicator.querySelector(".indicator-text").textContent =
      "Context ON - I remember this conversation";
    indicator.querySelector(".indicator-icon").textContent = "💡";
  } else {
    toggle.classList.remove("active");
    indicator.classList.add("off");
    indicator.querySelector(".indicator-text").textContent =
      "Context OFF - Each question is fresh";
    indicator.querySelector(".indicator-icon").textContent = "⚡";
  }

  saveSettings();
  showStatus(`Context ${contextEnabled ? "enabled" : "disabled"}`, "success");
}

// ===== UI HELPERS =====
function showLoading(show) {
  const loading = document.getElementById("loadingIndicator");
  if (show) {
    loading.classList.remove("hidden");
    scrollToBottom();
  } else {
    loading.classList.add("hidden");
  }
}

function showStatus(message, type = "") {
  const statusBar = document.getElementById("statusBar");
  const statusText = document.getElementById("statusText");

  statusText.textContent = message;
  statusBar.className = "status-bar";
  if (type) statusBar.classList.add(type);

  // Auto-clear after 3 seconds
  setTimeout(() => {
    if (statusText.textContent === message) {
      statusText.textContent = "Ready";
      statusBar.className = "status-bar";
    }
  }, 3000);
}

function scrollToBottom() {
  const container = document.getElementById("chatContainer");
  container.scrollTop = container.scrollHeight;
}

function autoResizeInput() {
  const input = document.getElementById("messageInput");
  input.style.height = "auto";
  input.style.height = Math.min(input.scrollHeight, 200) + "px";
}

function handleInputKeydown(e) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  }
}

// ===== STORAGE FUNCTIONS =====
function saveChatsToStorage() {
  const data = {
    chats: chats,
    activeChat: activeChat ? activeChat.id : null,
    contextEnabled: contextEnabled,
  };
  localStorage.setItem("smolpc_chats", JSON.stringify(data));
}

function loadChatsFromStorage() {
  const stored = localStorage.getItem("smolpc_chats");
  if (!stored) return;

  try {
    const data = JSON.parse(stored);
    chats = data.chats || [];
    contextEnabled =
      data.contextEnabled !== undefined ? data.contextEnabled : true;

    if (data.activeChat) {
      activeChat = chats.find((c) => c.id === data.activeChat);
    }

    renderChatList();

    // Update context toggle UI
    const toggle = document
      .getElementById("contextToggle")
      .querySelector(".toggle-switch");
    if (contextEnabled) {
      toggle.classList.add("active");
    } else {
      toggle.classList.remove("active");
    }
  } catch (error) {
    console.error("Failed to load chats:", error);
  }
}

function saveSettings() {
  saveChatsToStorage();
}

// ===== UTILITY FUNCTIONS =====
function generateId() {
  return "chat_" + Date.now() + "_" + Math.random().toString(36).substr(2, 9);
}

function generateChatTitle(firstMessage) {
  // Take first 50 chars of first message
  let title = firstMessage.substring(0, 50);
  if (firstMessage.length > 50) {
    title += "...";
  }
  return title;
}

function formatTime(timestamp) {
  const date = new Date(timestamp);
  const now = new Date();

  const diff = now - date;
  const minutes = Math.floor(diff / 60000);
  const hours = Math.floor(diff / 3600000);
  const days = Math.floor(diff / 86400000);

  if (minutes < 1) return "Just now";
  if (minutes < 60) return `${minutes}m ago`;
  if (hours < 24) return `${hours}h ago`;
  if (days < 7) return `${days}d ago`;

  return date.toLocaleDateString();
}

function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

// Make functions available globally for onclick handlers
window.deleteChat = deleteChat;
window.renameChat = renameChat;
window.copyMessage = copyMessage;
window.saveMessage = saveMessage;
