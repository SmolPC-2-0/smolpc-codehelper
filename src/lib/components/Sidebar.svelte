<script lang="ts">
	import { chatsStore } from '$lib/stores/chats.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { groupChatsByTime } from '$lib/utils/date';
	import { MessageSquarePlus, Trash2 } from '@lucide/svelte';
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

	// In-app confirmation modal state
	let showConfirm = $state(false);
	let pendingDeleteId: string | null = $state(null);

	function requestDelete(id: string) {
		pendingDeleteId = id;
		showConfirm = true;
	}

	function confirmDelete() {
		if (pendingDeleteId) {
			chatsStore.deleteChat(pendingDeleteId);
		}
		pendingDeleteId = null;
		showConfirm = false;
	}

	function cancelDelete() {
		pendingDeleteId = null;
		showConfirm = false;
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
				<h3 class="mb-2 px-2 text-xs font-semibold text-gray-500 uppercase dark:text-gray-400">
					{group.label}
				</h3>
				{#each group.chats as chat (chat.id)}
					<div
						class="group relative mb-1 flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm transition-colors hover:bg-gray-100 dark:hover:bg-gray-900 {chatsStore.currentChatId ===
						chat.id
							? 'bg-gray-100 dark:bg-gray-900'
							: ''}"
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
							onclick={(e: MouseEvent) => {
								// prevent bubbling to the parent select handler
								e.preventDefault();
								e.stopPropagation();
								// open in-app confirmation modal (avoids window.confirm)
								requestDelete(chat.id);
							}}
							data-chat-id={chat.id}
							class="ml-2 rounded p-1 opacity-0 transition-opacity group-hover:opacity-100 hover:bg-red-100 dark:hover:bg-red-900"
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
</aside>

{#if showConfirm}
	<!-- Simple in-app confirmation modal -->
	<div class="fixed inset-0 z-50 flex items-center justify-center">
		<div
			class="absolute inset-0 bg-black/40"
			onclick={cancelDelete}
			tabindex="0"
			role="button"
			onkeydown={(e: KeyboardEvent) => {
				if (e.key === 'Escape') cancelDelete();
			}}
		></div>
		<div class="relative z-10 mx-4 w-full max-w-sm rounded bg-white p-4 shadow-lg dark:bg-gray-900">
			<h4 class="mb-2 text-lg font-semibold text-gray-900 dark:text-gray-100">Delete chat</h4>
			<p class="mb-4 text-sm text-gray-700 dark:text-gray-300">
				Are you sure you want to delete this chat? This action cannot be undone.
			</p>
			<div class="flex justify-end gap-2">
				<Button variant="outline" onclick={cancelDelete}>No</Button>
				<Button variant="destructive" onclick={confirmDelete}>Yes, delete</Button>
			</div>
		</div>
	</div>
{/if}
