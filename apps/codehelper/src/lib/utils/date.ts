import type { Chat, ChatGroup, TimeGroup } from '$lib/types/chat';

/**
 * Get time group for a timestamp
 */
export function getTimeGroup(timestamp: number): TimeGroup {
	const now = Date.now();
	const diff = now - timestamp;

	const oneDay = 24 * 60 * 60 * 1000;
	const oneWeek = 7 * oneDay;

	// Today (last 24 hours)
	if (diff < oneDay) {
		const today = new Date().setHours(0, 0, 0, 0);
		const chatDate = new Date(timestamp).setHours(0, 0, 0, 0);
		if (today === chatDate) return 'today';
	}

	// Yesterday
	const yesterday = new Date(now - oneDay).setHours(0, 0, 0, 0);
	const chatDate = new Date(timestamp).setHours(0, 0, 0, 0);
	if (yesterday === chatDate) return 'yesterday';

	// Last 7 days
	if (diff < oneWeek) return 'lastWeek';

	// Older
	return 'older';
}

/**
 * Get label for time group
 */
export function getTimeGroupLabel(group: TimeGroup): string {
	switch (group) {
		case 'today':
			return 'Today';
		case 'yesterday':
			return 'Yesterday';
		case 'lastWeek':
			return 'Last 7 Days';
		case 'older':
			return 'Older';
	}
}

/**
 * Group chats by time period
 */
export function groupChatsByTime(chats: Chat[]): ChatGroup[] {
	const groups: Record<TimeGroup, Chat[]> = {
		today: [],
		yesterday: [],
		lastWeek: [],
		older: []
	};

	// Sort chats by updatedAt descending
	const sortedChats = [...chats].sort((a, b) => b.updatedAt - a.updatedAt);

	// Group chats
	for (const chat of sortedChats) {
		const group = getTimeGroup(chat.updatedAt);
		groups[group].push(chat);
	}

	// Convert to array format, filtering out empty groups
	const result: ChatGroup[] = [];
	const groupOrder: TimeGroup[] = ['today', 'yesterday', 'lastWeek', 'older'];

	for (const group of groupOrder) {
		if (groups[group].length > 0) {
			result.push({
				label: getTimeGroupLabel(group),
				chats: groups[group]
			});
		}
	}

	return result;
}

/**
 * Format timestamp to readable string
 */
export function formatTimestamp(timestamp: number): string {
	const date = new Date(timestamp);
	const now = new Date();

	// If today, show time
	if (date.toDateString() === now.toDateString()) {
		return date.toLocaleTimeString('en-US', {
			hour: 'numeric',
			minute: '2-digit',
			hour12: true
		});
	}

	// If this year, show month and day
	if (date.getFullYear() === now.getFullYear()) {
		return date.toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric'
		});
	}

	// Otherwise show full date
	return date.toLocaleDateString('en-US', {
		year: 'numeric',
		month: 'short',
		day: 'numeric'
	});
}
