<script lang="ts">
	import { chatsStore } from '$lib/stores/chats.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { groupChatsByTime } from '$lib/utils/date';
	import { MessageSquarePlus, Trash2, Settings } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';

	interface Props {
		isOpen: boolean;
		onClose?: () => void;
	}

	let { isOpen = true, onClose }: Props = $props();

	const chatGroups = $derived(groupChatsByTime(chatsStore.sortedChats));

	function handleNewChat() {
		chatsStore.createChat(settingsStore.selectedModel);
	}

	function handleSelectChat(chatId: string) {
		chatsStore.setCurrentChat(chatId);
		if (window.innerWidth < 768 && onClose) {
			onClose();
		}
	}

	function handleDeleteChat(chatId: string, event: MouseEvent) {
		event.preventDefault();
		event.stopPropagation();
		if (confirm('Delete this chat?')) {
			chatsStore.deleteChat(chatId);
		}
	}
</script>

<aside
	class="flex h-full w-64 flex-col border-r border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950"
	class:hidden={!isOpen}
>
	<!-- Header -->
	<div class="border-b border-gray-200 p-4 dark:border-gray-800">
		<h1 class="text-xl font-bold text-gray-900 dark:text-white">SmolPC Helper</h1>
		<p class="text-sm text-gray-600 dark:text-gray-400">AI Coding Assistant</p>
	</div>

	<!-- New Chat Button -->
	<div class="p-4">
		<Button onclick={handleNewChat} class="w-full">
			<MessageSquarePlus class="mr-2 h-4 w-4" />
			New Chat
		</Button>
	</div>

	<!-- Chat List -->
	<div class="flex-1 overflow-y-auto px-2">
		{#each chatGroups as group (group.label)}
			<div class="mb-4">
				<h3 class="mb-2 px-2 text-xs font-semibold uppercase text-gray-500 dark:text-gray-400">
					{group.label}
				</h3>
				{#each group.chats as chat (chat.id)}
					<div
						class="group relative mb-1 flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm transition-colors hover:bg-gray-100 dark:hover:bg-gray-900 {chatsStore.currentChatId === chat.id ? 'bg-gray-100 dark:bg-gray-900' : ''}"
					>
						<button
							type="button"
							onclick={() => handleSelectChat(chat.id)}
							class="flex-1 truncate text-left text-gray-700 dark:text-gray-300"
						>
							{chat.title}
						</button>
						<button
							type="button"
							onclick={(e) => handleDeleteChat(chat.id, e)}
							class="ml-2 rounded p-1 opacity-0 transition-opacity hover:bg-red-100 dark:hover:bg-red-900 group-hover:opacity-100"
							aria-label="Delete chat"
						>
							<Trash2 class="h-3 w-3 text-red-600 dark:text-red-400" />
						</button>
					</div>
				{/each}
			</div>
		{/each}

		{#if chatsStore.chats.length === 0}
			<div class="px-4 py-8 text-center text-sm text-gray-500 dark:text-gray-400">
				<p>No chats yet</p>
				<p>Click "New Chat" to start</p>
			</div>
		{/if}
	</div>

	<!-- Footer with Settings -->
	<div class="border-t border-gray-200 p-4 dark:border-gray-800">
		<Button variant="outline" class="w-full">
			<Settings class="mr-2 h-4 w-4" />
			Settings
		</Button>
	</div>
</aside>
