import type { Chat, Message } from '$lib/types/chat';
import type { AppMode } from '$lib/types/mode';
import { composerDraftStore } from '$lib/stores/composerDraft.svelte';
import { saveToStorage, loadFromStorage } from '$lib/utils/storage';

const STORAGE_KEY = 'smolpc_chats';
const CURRENT_CHAT_KEY = 'smolpc_current_chat'; // legacy single key — kept for migration
const MODE_CHAT_KEY = 'smolpc_mode_chats';

// Load initial state from localStorage
const initialChats = loadFromStorage<Chat[]>(STORAGE_KEY, []);

// Migrate: if legacy single currentChatId exists, seed it as the 'code' mode chat
const legacySingle = loadFromStorage<string | null>(CURRENT_CHAT_KEY, null);
const initialModeChats = loadFromStorage<Partial<Record<AppMode, string | null>>>(
	MODE_CHAT_KEY,
	legacySingle ? { code: legacySingle } : {}
);
if (legacySingle) {
	try {
		localStorage.removeItem(CURRENT_CHAT_KEY);
	} catch {
		/* best-effort */
	}
}

// Svelte 5 state using runes
let chats = $state<Chat[]>(initialChats);
let currentMode = $state<AppMode>('code');
let currentChatIdByMode = $state<Partial<Record<AppMode, string | null>>>(initialModeChats);

// Derived state
const currentChatId = $derived<string | null>(currentChatIdByMode[currentMode] ?? null);
const currentChat = $derived<Chat | null>(chats.find((chat) => chat.id === currentChatId) ?? null);
const sortedChats = $derived<Chat[]>([...chats].sort((a, b) => b.updatedAt - a.updatedAt));
const modeChats = $derived<Chat[]>(
	sortedChats.filter((chat) => (chat.mode ?? 'code') === currentMode)
);

export interface DeletedChatSnapshot {
	chat: Chat;
	index: number;
	wasCurrent: boolean;
}

function cloneChat(chat: Chat): Chat {
	return {
		...chat,
		messages: chat.messages.map((message) => ({ ...message }))
	};
}

function persistModeChats() {
	saveToStorage(MODE_CHAT_KEY, currentChatIdByMode);
}

function draftKeyForChat(chat: Chat): string {
	return `${chat.mode ?? 'code'}:${chat.id}`;
}

// Store object with methods
export const chatsStore = {
	// Getters
	get chats() {
		return chats;
	},
	get currentChatId() {
		return currentChatId;
	},
	get currentChat() {
		return currentChat;
	},
	get sortedChats() {
		return sortedChats;
	},
	get modeChats() {
		return modeChats;
	},

	// Actions

	setMode(mode: AppMode) {
		currentMode = mode;
	},

	createChat(model: string, mode?: AppMode): Chat {
		const chatMode = mode ?? currentMode;
		const newChat: Chat = {
			id: crypto.randomUUID(),
			title: 'New Chat',
			messages: [],
			createdAt: Date.now(),
			updatedAt: Date.now(),
			model,
			mode: chatMode,
			pinned: false,
			archived: false
		};
		chats = [...chats, newChat];
		currentChatIdByMode = { ...currentChatIdByMode, [chatMode]: newChat.id };
		persistModeChats();
		this.persist();
		return newChat;
	},

	// Callers must ensure currentMode matches the chat's mode — the Sidebar
	// already filters by mode so this is safe from the UI, but programmatic
	// callers should call setMode() first if switching modes.
	setCurrentChat(id: string) {
		const chat = chats.find((c) => c.id === id);
		if (chat) {
			const chatMode = chat.mode ?? 'code';
			currentChatIdByMode = { ...currentChatIdByMode, [chatMode]: id };
			persistModeChats();
		}
	},

	addMessage(chatId: string, message: Message) {
		const chat = chats.find((c) => c.id === chatId);
		if (chat) {
			chat.messages = [...chat.messages, message];
			chat.updatedAt = Date.now();

			// Auto-generate title from first user message
			if (chat.messages.length === 1 && message.role === 'user') {
				chat.title = message.content.slice(0, 50) + (message.content.length > 50 ? '...' : '');
			}

			this.persist();
		}
	},

	updateMessage(chatId: string, messageId: string, updates: Partial<Message>) {
		const chat = chats.find((c) => c.id === chatId);
		if (chat) {
			const message = chat.messages.find((m) => m.id === messageId);
			if (message) {
				Object.assign(message, updates);
				chat.updatedAt = Date.now();
				this.persist();
			}
		}
	},

	deleteChat(id: string): DeletedChatSnapshot | null {
		const index = chats.findIndex((chat) => chat.id === id);
		if (index !== -1) {
			const chatToDelete = chats[index];
			const chatMode = chatToDelete.mode ?? 'code';
			const snapshot: DeletedChatSnapshot = {
				chat: cloneChat(chatToDelete),
				index,
				wasCurrent: currentChatIdByMode[chatMode] === id
			};
			composerDraftStore.clearDraft(draftKeyForChat(chatToDelete));

			chats = chats.filter((chat) => chat.id !== id);

			// If deleted was current for its mode, pick next in same mode
			if (currentChatIdByMode[chatMode] === id) {
				const next = chats.find((c) => (c.mode ?? 'code') === chatMode && !c.archived);
				currentChatIdByMode = { ...currentChatIdByMode, [chatMode]: next?.id ?? null };
				persistModeChats();
			}

			this.persist();
			return snapshot;
		}

		return null;
	},

	restoreDeletedChat(snapshot: DeletedChatSnapshot) {
		const safeIndex = Math.max(0, Math.min(snapshot.index, chats.length));
		chats = [...chats.slice(0, safeIndex), snapshot.chat, ...chats.slice(safeIndex)];

		const chatMode = snapshot.chat.mode ?? 'code';
		if (
			snapshot.wasCurrent ||
			!currentChatIdByMode[chatMode] ||
			!chats.some((chat) => chat.id === currentChatIdByMode[chatMode])
		) {
			currentChatIdByMode = { ...currentChatIdByMode, [chatMode]: snapshot.chat.id };
		}

		persistModeChats();
		this.persist();
	},

	updateChatTitle(id: string, title: string) {
		const chat = chats.find((c) => c.id === id);
		if (chat) {
			chat.title = title;
			chat.updatedAt = Date.now();
			this.persist();
		}
	},

	togglePinned(id: string) {
		const chat = chats.find((c) => c.id === id);
		if (!chat) return;

		chat.pinned = !chat.pinned;
		chat.updatedAt = Date.now();
		this.persist();
	},

	toggleArchived(id: string) {
		const chat = chats.find((c) => c.id === id);
		if (!chat) return;

		chat.archived = !chat.archived;
		chat.pinned = chat.archived ? false : chat.pinned;
		chat.updatedAt = Date.now();

		const chatMode = chat.mode ?? 'code';
		if (chat.archived && currentChatIdByMode[chatMode] === id) {
			const nextChat =
				chats.find(
					(candidate) =>
						candidate.id !== id && (candidate.mode ?? 'code') === chatMode && !candidate.archived
				) ?? null;
			currentChatIdByMode = { ...currentChatIdByMode, [chatMode]: nextChat?.id ?? null };
			persistModeChats();
		}
		if (chat.archived) {
			composerDraftStore.clearDraft(draftKeyForChat(chat));
		}

		this.persist();
	},

	duplicateChat(id: string): Chat | null {
		const source = chats.find((chat) => chat.id === id);
		if (!source) return null;

		const chatMode = source.mode ?? 'code';
		const duplicate: Chat = {
			...cloneChat(source),
			id: crypto.randomUUID(),
			title: `${source.title} (Copy)`,
			createdAt: Date.now(),
			updatedAt: Date.now(),
			mode: chatMode,
			pinned: false,
			archived: false,
			messages: source.messages.map((message) => ({
				...message,
				id: crypto.randomUUID(),
				isStreaming: false
			}))
		};

		chats = [...chats, duplicate];
		currentChatIdByMode = { ...currentChatIdByMode, [chatMode]: duplicate.id };
		persistModeChats();
		this.persist();
		return duplicate;
	},

	clearAllChats() {
		for (const chat of chats) {
			composerDraftStore.clearDraft(draftKeyForChat(chat));
		}
		chats = [];
		currentChatIdByMode = {};
		persistModeChats();
		this.persist();
	},

	finalizeStaleStreamingMessages() {
		let changed = false;

		for (const chat of chats) {
			let chatChanged = false;
			for (const message of chat.messages) {
				if (!message.isStreaming) {
					continue;
				}

				message.isStreaming = false;
				if (!message.content.trim()) {
					message.content = 'Generation interrupted before completion.';
				}
				chatChanged = true;
			}

			if (chatChanged) {
				chat.updatedAt = Date.now();
				changed = true;
			}
		}

		if (changed) {
			this.persist();
		}
	},

	persist() {
		saveToStorage(STORAGE_KEY, chats);
	}
};
