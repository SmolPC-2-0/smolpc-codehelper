// Tauri v2 API
const { invoke } = window.__TAURI_INTERNALS__;

// ===== STATE MANAGEMENT =====
let chats = [];
let activeChat = null;
let contextEnabled = true;

// ===== INITIALIZATION =====
window.addEventListener("DOMContentLoaded", async () => {
  loadChatsFromStorage();
  setupEventListeners();
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

  // Add user message
  addMessage("user", content);
  input.value = "";
  autoResizeInput();

  // Update chat title if it's still "New Chat"
  if (activeChat.title === "New Chat") {
    activeChat.title = generateChatTitle(content);
    renderChatList();
  }

  // Show loading
  showLoading(true);
  document.getElementById("sendBtn").disabled = true;

  try {
    // Get context messages if enabled
    const context = contextEnabled ? getContextMessages() : [];

    // Prefer streaming for faster perceived latency; fallback when unavailable
    const { response, streamed } = await generateResponseStream(content, context);

    // If non-streaming fallback was used, append assistant message now
    if (!streamed) {
      addMessage("assistant", response);
    }

    showStatus("Response generated", "success");
  } catch (error) {
    showStatus(`Error: ${error}`, "error");
    addMessage("assistant", `❌ Error: ${error}`);
  } finally {
    showLoading(false);
    document.getElementById("sendBtn").disabled = false;
    input.focus();
  }
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

async function generateResponse(userMessage, context) {
  const model = document.getElementById("modelSelect").value;

  // Build the full prompt with context
  let fullPrompt = "";

  if (context.length > 0) {
    fullPrompt = "Previous conversation:\n\n";
    context.forEach((msg) => {
      const role = msg.role === "user" ? "Student" : "Assistant";
      fullPrompt += `${role}: ${msg.content}\n\n`;
    });
    fullPrompt += `Student: ${userMessage}`;
  } else {
    fullPrompt = userMessage;
  }

  // Call Rust backend
  return await invoke("generate_code", {
    prompt: fullPrompt,
    model: model,
  });
}

// Streaming generation using Tauri window events. Falls back to non-streaming
// when the event API is not available (maintains offline compatibility).
async function generateResponseStream(userMessage, context) {
  const model = document.getElementById("modelSelect").value;

  const hasTauriEvents = !!(
    window.__TAURI__ &&
    window.__TAURI__.event &&
    typeof window.__TAURI__.event.listen === "function"
  );
  if (!hasTauriEvents) {
    return await generateResponse(userMessage, context);
  }

  // Build the full prompt with context (same as non-streaming)
  let fullPrompt = "";
  if (context.length > 0) {
    fullPrompt = "Previous conversation:\n\n";
    context.forEach((msg) => {
      const role = msg.role === "user" ? "Student" : "Assistant";
      fullPrompt += `${role}: ${msg.content}\n\n`;
    });
    fullPrompt += `Student: ${userMessage}`;
  } else {
    fullPrompt = userMessage;
  }

  // Create an empty assistant message to fill with chunks
  addMessage("assistant", "");
  const assistantIndex = activeChat.messages.length - 1;

  const { event } = window.__TAURI__;
  const unlistenFns = [];
  const finalText = { value: "" };

  const unlistenChunk = await event.listen("gen_chunk", (e) => {
    const chunk = e && e.payload && e.payload.chunk ? e.payload.chunk : "";
    if (!chunk) return;
    finalText.value += chunk;
    activeChat.messages[assistantIndex].content = finalText.value;
    activeChat.lastUpdated = new Date().toISOString();
    saveChatsToStorage();
    renderMessages();
    scrollToBottom();
  });
  unlistenFns.push(unlistenChunk);

  const donePromise = new Promise((resolve, reject) => {
    event
      .listen("gen_done", () => resolve(finalText.value))
      .then((fn) => unlistenFns.push(fn));
    event
      .listen("gen_error", (e) => {
        const err =
          e && e.payload && e.payload.error ? e.payload.error : "Unknown error";
        reject(new Error(err));
      })
      .then((fn) => unlistenFns.push(fn));
  });

  // Start backend streaming after listeners are attached
  invoke("generate_code_stream", { prompt: fullPrompt, model: model }).catch(
    (err) => {
      console.error("Streaming invoke error:", err);
    }
  );

  try {
    return await donePromise;
  } finally {
    try {
      unlistenFns.forEach((fn) => typeof fn === "function" && fn());
    } catch (_) {}
  }
}

function renderMessages() {
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
