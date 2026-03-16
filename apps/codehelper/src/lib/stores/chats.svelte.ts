import type { Chat, Message } from '$lib/types/chat';
import { APP_MODES, type AppMode } from '$lib/types/mode';
import { saveToStorage, loadFromStorage } from '$lib/utils/storage';

const STORAGE_KEY = 'smolpc_unified_chats_v1';
const CURRENT_CHAT_KEY = 'smolpc_unified_current_chat_by_mode_v1';

type CurrentChatByMode = Record<AppMode, string | null>;

function emptyCurrentChatByMode(): CurrentChatByMode {
	return {
		code: null,
		gimp: null,
		blender: null,
		writer: null,
		calc: null,
		impress: null
	};
}

function isAppMode(value: unknown): value is AppMode {
	return typeof value === 'string' && APP_MODES.includes(value as AppMode);
}

function sanitizeMessage(candidate: unknown): Message | null {
	if (!candidate || typeof candidate !== 'object') {
		return null;
	}

	const value = candidate as Partial<Message>;
	if (
		typeof value.id !== 'string' ||
		(value.role !== 'user' && value.role !== 'assistant') ||
		typeof value.content !== 'string' ||
		typeof value.timestamp !== 'number'
	) {
		return null;
	}

	return {
		id: value.id,
		role: value.role,
		content: value.content,
		timestamp: value.timestamp,
		isStreaming: value.isStreaming === true
	};
}

function sanitizeChat(candidate: unknown): Chat | null {
	if (!candidate || typeof candidate !== 'object') {
		return null;
	}

	const value = candidate as Partial<Chat>;
	if (
		typeof value.id !== 'string' ||
		typeof value.title !== 'string' ||
		!Array.isArray(value.messages) ||
		typeof value.createdAt !== 'number' ||
		typeof value.updatedAt !== 'number' ||
		typeof value.model !== 'string'
	) {
		return null;
	}

	const messages = value.messages
		.map((message) => sanitizeMessage(message))
		.filter((message): message is Message => message !== null);

	return {
		id: value.id,
		mode: isAppMode(value.mode) ? value.mode : 'code',
		title: value.title,
		messages,
		createdAt: value.createdAt,
		updatedAt: value.updatedAt,
		model: value.model,
		pinned: value.pinned === true,
		archived: value.archived === true
	};
}

function sanitizeChats(candidate: unknown): Chat[] {
	if (!Array.isArray(candidate)) {
		return [];
	}

	return candidate.map((chat) => sanitizeChat(chat)).filter((chat): chat is Chat => chat !== null);
}

function sanitizeCurrentChatByMode(candidate: unknown): CurrentChatByMode {
	const next = emptyCurrentChatByMode();
	if (!candidate || typeof candidate !== 'object') {
		return next;
	}

	for (const mode of APP_MODES) {
		const current = (candidate as Record<string, unknown>)[mode];
		next[mode] = typeof current === 'string' ? current : null;
	}

	return next;
}

const initialChats = sanitizeChats(loadFromStorage<unknown>(STORAGE_KEY, []));
const initialCurrentChatIdByMode = sanitizeCurrentChatByMode(
	loadFromStorage<unknown>(CURRENT_CHAT_KEY, emptyCurrentChatByMode())
);

let chats = $state<Chat[]>(initialChats);
let currentChatIdByMode = $state<CurrentChatByMode>(initialCurrentChatIdByMode);

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

function getChatsForModeInternal(mode: AppMode): Chat[] {
	return chats
		.filter((chat) => chat.mode === mode)
		.sort((left, right) => right.updatedAt - left.updatedAt);
}

function resolveCurrentChatId(mode: AppMode): string | null {
	const currentId = currentChatIdByMode[mode];
	if (!currentId) {
		return null;
	}

	return chats.some((chat) => chat.id === currentId && chat.mode === mode) ? currentId : null;
}

export const chatsStore = {
	get chats() {
		return chats;
	},
	get currentChatIdByMode() {
		return currentChatIdByMode;
	},

	getChatsForMode(mode: AppMode): Chat[] {
		return getChatsForModeInternal(mode);
	},

	getCurrentChatIdForMode(mode: AppMode): string | null {
		return resolveCurrentChatId(mode);
	},

	getCurrentChatForMode(mode: AppMode): Chat | null {
		const currentId = resolveCurrentChatId(mode);
		return chats.find((chat) => chat.id === currentId && chat.mode === mode) ?? null;
	},

	createChat(mode: AppMode, model: string): Chat {
		const newChat: Chat = {
			id: crypto.randomUUID(),
			mode,
			title: 'New Chat',
			messages: [],
			createdAt: Date.now(),
			updatedAt: Date.now(),
			model,
			pinned: false,
			archived: false
		};

		chats = [...chats, newChat];
		currentChatIdByMode = {
			...currentChatIdByMode,
			[mode]: newChat.id
		};
		this.persist();
		return newChat;
	},

	setCurrentChat(mode: AppMode, id: string) {
		if (!chats.some((chat) => chat.id === id && chat.mode === mode)) {
			return;
		}

		currentChatIdByMode = {
			...currentChatIdByMode,
			[mode]: id
		};
		this.persist();
	},

	addMessage(chatId: string, message: Message) {
		const chat = chats.find((candidate) => candidate.id === chatId);
		if (!chat) {
			return;
		}

		chat.messages = [...chat.messages, message];
		chat.updatedAt = Date.now();

		if (chat.messages.length === 1 && message.role === 'user') {
			chat.title = message.content.slice(0, 50) + (message.content.length > 50 ? '...' : '');
		}

		this.persist();
	},

	updateMessage(chatId: string, messageId: string, updates: Partial<Message>) {
		const chat = chats.find((candidate) => candidate.id === chatId);
		if (!chat) {
			return;
		}

		const message = chat.messages.find((candidate) => candidate.id === messageId);
		if (!message) {
			return;
		}

		Object.assign(message, updates);
		chat.updatedAt = Date.now();
		this.persist();
	},

	deleteChat(id: string): DeletedChatSnapshot | null {
		const index = chats.findIndex((chat) => chat.id === id);
		if (index === -1) {
			return null;
		}

		const chatToDelete = chats[index];
		const wasCurrent = resolveCurrentChatId(chatToDelete.mode) === id;
		const snapshot: DeletedChatSnapshot = {
			chat: cloneChat(chatToDelete),
			index,
			wasCurrent
		};

		chats = chats.filter((chat) => chat.id !== id);

		if (wasCurrent) {
			const nextChat =
				getChatsForModeInternal(chatToDelete.mode).find((chat) => !chat.archived) ??
				getChatsForModeInternal(chatToDelete.mode)[0] ??
				null;
			currentChatIdByMode = {
				...currentChatIdByMode,
				[chatToDelete.mode]: nextChat?.id ?? null
			};
		}

		this.persist();
		return snapshot;
	},

	restoreDeletedChat(snapshot: DeletedChatSnapshot) {
		const safeIndex = Math.max(0, Math.min(snapshot.index, chats.length));
		chats = [...chats.slice(0, safeIndex), snapshot.chat, ...chats.slice(safeIndex)];

		const currentId = resolveCurrentChatId(snapshot.chat.mode);
		if (snapshot.wasCurrent || !currentId) {
			currentChatIdByMode = {
				...currentChatIdByMode,
				[snapshot.chat.mode]: snapshot.chat.id
			};
		}

		this.persist();
	},

	updateChatTitle(id: string, title: string) {
		const chat = chats.find((candidate) => candidate.id === id);
		if (!chat) {
			return;
		}

		chat.title = title;
		chat.updatedAt = Date.now();
		this.persist();
	},

	togglePinned(id: string) {
		const chat = chats.find((candidate) => candidate.id === id);
		if (!chat) {
			return;
		}

		chat.pinned = !chat.pinned;
		chat.updatedAt = Date.now();
		this.persist();
	},

	toggleArchived(id: string) {
		const chat = chats.find((candidate) => candidate.id === id);
		if (!chat) {
			return;
		}

		chat.archived = !chat.archived;
		chat.pinned = chat.archived ? false : chat.pinned;
		chat.updatedAt = Date.now();

		if (chat.archived && resolveCurrentChatId(chat.mode) === id) {
			const nextChat =
				getChatsForModeInternal(chat.mode).find(
					(candidate) => candidate.id !== id && !candidate.archived
				) ??
				getChatsForModeInternal(chat.mode).find((candidate) => candidate.id !== id) ??
				null;
			currentChatIdByMode = {
				...currentChatIdByMode,
				[chat.mode]: nextChat?.id ?? null
			};
		}

		this.persist();
	},

	duplicateChat(id: string): Chat | null {
		const source = chats.find((chat) => chat.id === id);
		if (!source) {
			return null;
		}

		const duplicate: Chat = {
			...cloneChat(source),
			id: crypto.randomUUID(),
			title: `${source.title} (Copy)`,
			createdAt: Date.now(),
			updatedAt: Date.now(),
			pinned: false,
			archived: false,
			messages: source.messages.map((message) => ({
				...message,
				id: crypto.randomUUID(),
				isStreaming: false
			}))
		};

		chats = [...chats, duplicate];
		currentChatIdByMode = {
			...currentChatIdByMode,
			[duplicate.mode]: duplicate.id
		};
		this.persist();
		return duplicate;
	},

	clearAllChats() {
		chats = [];
		currentChatIdByMode = emptyCurrentChatByMode();
		this.persist();
	},

	persist() {
		saveToStorage(STORAGE_KEY, chats);
		saveToStorage(CURRENT_CHAT_KEY, currentChatIdByMode);
	}
};
