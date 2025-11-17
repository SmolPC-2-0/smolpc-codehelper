import type { Chat, Message } from '$lib/types/chat';
import { saveToStorage, loadFromStorage } from '$lib/utils/storage';

const STORAGE_KEY = 'smolpc_chats';
const CURRENT_CHAT_KEY = 'smolpc_current_chat';

// Load initial state from localStorage
const initialChats = loadFromStorage<Chat[]>(STORAGE_KEY, []);
const initialCurrentId = loadFromStorage<string | null>(CURRENT_CHAT_KEY, null);

// Svelte 5 state using runes
let chats = $state<Chat[]>(initialChats);
let currentChatId = $state<string | null>(initialCurrentId);

// Derived state
const currentChat = $derived<Chat | null>(
	chats.find((chat) => chat.id === currentChatId) ?? null
);

const sortedChats = $derived<Chat[]>(
	[...chats].sort((a, b) => b.updatedAt - a.updatedAt)
);

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

	// Actions
	createChat(model: string): Chat {
		const newChat: Chat = {
			id: crypto.randomUUID(),
			title: 'New Chat',
			messages: [],
			createdAt: Date.now(),
			updatedAt: Date.now(),
			model
		};
		chats = [...chats, newChat];
		currentChatId = newChat.id;
		this.persist();
		return newChat;
	},

	setCurrentChat(id: string) {
		if (chats.some((chat) => chat.id === id)) {
			currentChatId = id;
			saveToStorage(CURRENT_CHAT_KEY, id);
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

	deleteChat(id: string) {
		const index = chats.findIndex((chat) => chat.id === id);
		if (index !== -1) {
			chats = chats.filter((chat) => chat.id !== id);
			if (currentChatId === id) {
				currentChatId = chats.length > 0 ? chats[0].id : null;
				saveToStorage(CURRENT_CHAT_KEY, currentChatId);
			}
			this.persist();
		}
	},

	updateChatTitle(id: string, title: string) {
		const chat = chats.find((c) => c.id === id);
		if (chat) {
			chat.title = title;
			chat.updatedAt = Date.now();
			this.persist();
		}
	},

	clearAllChats() {
		chats = [];
		currentChatId = null;
		this.persist();
		saveToStorage(CURRENT_CHAT_KEY, null);
	},

	persist() {
		saveToStorage(STORAGE_KEY, chats);
	}
};
