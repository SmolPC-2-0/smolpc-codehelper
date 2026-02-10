<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { chatsStore } from '$lib/stores/chats.svelte';
	import type { DeletedChatSnapshot } from '$lib/stores/chats.svelte';
	import type { Chat } from '$lib/types/chat';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { formatTimestamp, groupChatsByTime } from '$lib/utils/date';
	import {
		Archive,
		ArchiveRestore,
		ChevronDown,
		ChevronRight,
		Copy,
		Ellipsis,
		MessageSquarePlus,
		PanelLeftClose,
		Pin,
		PinOff,
		Pencil,
		Search,
		Trash2,
		Undo2
	} from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';

	interface Props {
		isOpen: boolean;
		onClose?: () => void;
	}

	let { isOpen = true, onClose }: Props = $props();

	let searchQuery = $state('');
	let showArchived = $state(false);
	let actionsMenuChatId = $state<string | null>(null);
	let editingChatId = $state<string | null>(null);
	let editingTitle = $state('');

	let showConfirm = $state(false);
	let pendingDeleteId: string | null = $state(null);
	let recentlyDeleted = $state<DeletedChatSnapshot | null>(null);
	let undoTimeoutId = $state<number | null>(null);

	const normalizedQuery = $derived(searchQuery.trim().toLowerCase());

	function chatMatchesQuery(chat: Chat, query: string): boolean {
		if (!query) return true;
		return (
			chat.title.toLowerCase().includes(query) ||
			chat.model.toLowerCase().includes(query) ||
			chat.messages.some((message) => message.content.toLowerCase().includes(query))
		);
	}

	const activeChats = $derived(
		chatsStore.sortedChats.filter((chat) => !chat.archived && chatMatchesQuery(chat, normalizedQuery))
	);

	const archivedChats = $derived(
		chatsStore.sortedChats.filter((chat) => chat.archived && chatMatchesQuery(chat, normalizedQuery))
	);

	const pinnedChats = $derived(activeChats.filter((chat) => chat.pinned));
	const regularChatGroups = $derived(groupChatsByTime(activeChats.filter((chat) => !chat.pinned)));
	const archivedChatGroups = $derived(groupChatsByTime(archivedChats));
	const shouldShowArchivedSection = $derived(showArchived || normalizedQuery.length > 0);

	function handleNewChat() {
		actionsMenuChatId = null;
		editingChatId = null;
		chatsStore.createChat(settingsStore.selectedModel);
	}

	function handleSelectChat(chatId: string) {
		if (editingChatId) return;
		actionsMenuChatId = null;
		chatsStore.setCurrentChat(chatId);
		if (window.innerWidth < 768 && onClose) {
			onClose();
		}
	}

	function toggleActionsMenu(chatId: string, event: MouseEvent) {
		event.preventDefault();
		event.stopPropagation();
		actionsMenuChatId = actionsMenuChatId === chatId ? null : chatId;
	}

	function startRename(chat: Chat) {
		editingChatId = chat.id;
		editingTitle = chat.title;
		actionsMenuChatId = null;
	}

	function commitRename() {
		if (!editingChatId) return;
		const nextTitle = editingTitle.trim();
		if (nextTitle.length > 0) {
			chatsStore.updateChatTitle(editingChatId, nextTitle);
		}
		editingChatId = null;
		editingTitle = '';
	}

	function cancelRename() {
		editingChatId = null;
		editingTitle = '';
	}

	function duplicateChat(chatId: string) {
		chatsStore.duplicateChat(chatId);
		actionsMenuChatId = null;
	}

	function togglePinned(chatId: string) {
		chatsStore.togglePinned(chatId);
		actionsMenuChatId = null;
	}

	function toggleArchived(chatId: string) {
		chatsStore.toggleArchived(chatId);
		actionsMenuChatId = null;
	}

	function requestDelete(id: string) {
		pendingDeleteId = id;
		showConfirm = true;
		actionsMenuChatId = null;
	}

	function confirmDelete() {
		if (pendingDeleteId) {
			const snapshot = chatsStore.deleteChat(pendingDeleteId);
			if (snapshot) {
				recentlyDeleted = snapshot;
				if (undoTimeoutId) {
					window.clearTimeout(undoTimeoutId);
				}
				undoTimeoutId = window.setTimeout(() => {
					recentlyDeleted = null;
					undoTimeoutId = null;
				}, 7000);
			}
		}
		pendingDeleteId = null;
		showConfirm = false;
	}

	function cancelDelete() {
		pendingDeleteId = null;
		showConfirm = false;
	}

	function undoDelete() {
		if (!recentlyDeleted) return;
		chatsStore.restoreDeletedChat(recentlyDeleted);
		recentlyDeleted = null;
		if (undoTimeoutId) {
			window.clearTimeout(undoTimeoutId);
			undoTimeoutId = null;
		}
	}

	function handleGlobalClick(event: MouseEvent) {
		const target = event.target as HTMLElement | null;
		if (!target) return;
		if (!target.closest('.sidebar__row-actions')) {
			actionsMenuChatId = null;
		}
	}

	onMount(() => {
		window.addEventListener('mousedown', handleGlobalClick);
	});

	onDestroy(() => {
		window.removeEventListener('mousedown', handleGlobalClick);
		if (undoTimeoutId) {
			window.clearTimeout(undoTimeoutId);
		}
	});
</script>

<aside class="sidebar" class:hidden={!isOpen}>
	<div class="sidebar__header">
		<div class="sidebar__header-row">
			<div>
				<h1>SmolPC Helper</h1>
				<p>Offline coding assistant workspace</p>
			</div>
			{#if onClose}
				<Button
					variant="ghost"
					size="icon"
					onclick={onClose}
					class="sidebar__collapse"
					aria-label="Collapse sidebar"
					title="Collapse sidebar (Ctrl/Cmd + \\)"
				>
					<PanelLeftClose class="h-4 w-4" />
				</Button>
			{/if}
		</div>
	</div>

	<div class="sidebar__action">
		<Button onclick={handleNewChat} class="sidebar__new-chat">
			<MessageSquarePlus class="mr-2 h-4 w-4" />
			New Chat
		</Button>

		<div class="sidebar__search-wrap">
			<Search class="sidebar__search-icon h-3.5 w-3.5" />
			<input
				type="search"
				bind:value={searchQuery}
				class="sidebar__search-input"
				placeholder="Search chats, model, content"
				aria-label="Search chats"
			/>
		</div>
	</div>

	<div class="sidebar__scroll">
		{#if pinnedChats.length > 0}
			<section class="sidebar__group">
				<h3 class="sidebar__group-title">Pinned</h3>
				{#each pinnedChats as chat (chat.id)}
					<div
						class={`sidebar__chat-row ${chatsStore.currentChatId === chat.id ? 'sidebar__chat-row--active' : ''}`}
					>
						{#if editingChatId === chat.id}
							<input
								bind:value={editingTitle}
								class="sidebar__rename-input"
								onblur={commitRename}
								onkeydown={(event: KeyboardEvent) => {
									if (event.key === 'Enter') {
										event.preventDefault();
										commitRename();
									} else if (event.key === 'Escape') {
										event.preventDefault();
										cancelRename();
									}
								}}
							/>
						{:else}
							<button type="button" onclick={() => handleSelectChat(chat.id)} class="sidebar__chat-button">
								<span class="sidebar__chat-title">{chat.title}</span>
								<span class="sidebar__chat-meta">{formatTimestamp(chat.updatedAt)}</span>
							</button>
						{/if}

						<div class="sidebar__row-actions">
							<button
								type="button"
								onclick={() => togglePinned(chat.id)}
								class="sidebar__icon-btn"
								aria-label="Unpin chat"
								title="Unpin"
							>
								<PinOff class="h-3.5 w-3.5" />
							</button>
							<button
								type="button"
								onclick={(event: MouseEvent) => toggleActionsMenu(chat.id, event)}
								class="sidebar__icon-btn"
								aria-label="Chat actions"
								title="More actions"
							>
								<Ellipsis class="h-3.5 w-3.5" />
							</button>
							{#if actionsMenuChatId === chat.id}
								<div class="sidebar__menu">
									<button type="button" onclick={() => startRename(chat)}><Pencil class="h-3.5 w-3.5" /> Rename</button>
									<button type="button" onclick={() => duplicateChat(chat.id)}><Copy class="h-3.5 w-3.5" /> Duplicate</button>
									<button type="button" onclick={() => toggleArchived(chat.id)}><Archive class="h-3.5 w-3.5" /> Archive</button>
									<button type="button" class="sidebar__menu-danger" onclick={() => requestDelete(chat.id)}>
										<Trash2 class="h-3.5 w-3.5" /> Delete
									</button>
								</div>
							{/if}
						</div>
					</div>
				{/each}
			</section>
		{/if}

		{#each regularChatGroups as group (group.label)}
			<section class="sidebar__group">
				<h3 class="sidebar__group-title">{group.label}</h3>
				{#each group.chats as chat (chat.id)}
					<div
						class={`sidebar__chat-row ${chatsStore.currentChatId === chat.id ? 'sidebar__chat-row--active' : ''}`}
					>
						{#if editingChatId === chat.id}
							<input
								bind:value={editingTitle}
								class="sidebar__rename-input"
								onblur={commitRename}
								onkeydown={(event: KeyboardEvent) => {
									if (event.key === 'Enter') {
										event.preventDefault();
										commitRename();
									} else if (event.key === 'Escape') {
										event.preventDefault();
										cancelRename();
									}
								}}
							/>
						{:else}
							<button type="button" onclick={() => handleSelectChat(chat.id)} class="sidebar__chat-button">
								<span class="sidebar__chat-title">{chat.title}</span>
								<span class="sidebar__chat-meta">{formatTimestamp(chat.updatedAt)}</span>
							</button>
						{/if}

						<div class="sidebar__row-actions">
							<button
								type="button"
								onclick={() => togglePinned(chat.id)}
								class="sidebar__icon-btn"
								aria-label="Pin chat"
								title="Pin"
							>
								<Pin class="h-3.5 w-3.5" />
							</button>
							<button
								type="button"
								onclick={(event: MouseEvent) => toggleActionsMenu(chat.id, event)}
								class="sidebar__icon-btn"
								aria-label="Chat actions"
								title="More actions"
							>
								<Ellipsis class="h-3.5 w-3.5" />
							</button>
							{#if actionsMenuChatId === chat.id}
								<div class="sidebar__menu">
									<button type="button" onclick={() => startRename(chat)}><Pencil class="h-3.5 w-3.5" /> Rename</button>
									<button type="button" onclick={() => duplicateChat(chat.id)}><Copy class="h-3.5 w-3.5" /> Duplicate</button>
									<button type="button" onclick={() => toggleArchived(chat.id)}><Archive class="h-3.5 w-3.5" /> Archive</button>
									<button type="button" class="sidebar__menu-danger" onclick={() => requestDelete(chat.id)}>
										<Trash2 class="h-3.5 w-3.5" /> Delete
									</button>
								</div>
							{/if}
						</div>
					</div>
				{/each}
			</section>
		{/each}

		{#if archivedChats.length > 0}
			<section class="sidebar__group sidebar__group--archived">
				<button
					type="button"
					onclick={() => (showArchived = !showArchived)}
					class="sidebar__archived-toggle"
					aria-label={showArchived ? 'Hide archived chats' : 'Show archived chats'}
				>
					{#if showArchived}
						<ChevronDown class="h-3.5 w-3.5" />
					{:else}
						<ChevronRight class="h-3.5 w-3.5" />
					{/if}
					<span>Archived ({archivedChats.length})</span>
				</button>

				{#if shouldShowArchivedSection}
					{#each archivedChatGroups as group (group.label)}
						<div class="sidebar__archived-block">
							<h4>{group.label}</h4>
							{#each group.chats as chat (chat.id)}
								<div class="sidebar__chat-row sidebar__chat-row--archived">
									<button
										type="button"
										onclick={() => handleSelectChat(chat.id)}
										class="sidebar__chat-button"
									>
										<span class="sidebar__chat-title">{chat.title}</span>
										<span class="sidebar__chat-meta">{formatTimestamp(chat.updatedAt)}</span>
									</button>
									<div class="sidebar__row-actions">
										<button
											type="button"
											onclick={() => toggleArchived(chat.id)}
											class="sidebar__icon-btn"
											aria-label="Restore chat"
											title="Restore"
										>
											<ArchiveRestore class="h-3.5 w-3.5" />
										</button>
									</div>
								</div>
							{/each}
						</div>
					{/each}
				{/if}
			</section>
		{/if}

		{#if activeChats.length === 0 && archivedChats.length === 0}
			<div class="sidebar__empty">
				{#if normalizedQuery}
					<p>No results for "{searchQuery}"</p>
					<p>Try fewer words or check archived chats</p>
				{:else}
					<p>No chats yet</p>
					<p>Click "New Chat" to start</p>
				{/if}
			</div>
		{/if}
	</div>
</aside>

{#if recentlyDeleted}
	<div class="sidebar-undo">
		<div class="sidebar-undo__text">
			<span>Chat deleted.</span>
			<strong>{recentlyDeleted.chat.title}</strong>
		</div>
		<Button variant="outline" onclick={undoDelete} class="sidebar-undo__button">
			<Undo2 class="mr-2 h-3.5 w-3.5" />
			Undo
		</Button>
	</div>
{/if}

{#if showConfirm}
	<div class="sidebar-modal">
		<div
			class="sidebar-modal__backdrop"
			onclick={cancelDelete}
			tabindex="0"
			role="button"
			onkeydown={(event: KeyboardEvent) => {
				if (event.key === 'Escape') cancelDelete();
			}}
		></div>
		<div class="sidebar-modal__card">
			<h4>Delete chat</h4>
			<p>Are you sure you want to delete this chat? This action cannot be undone.</p>
			<div class="sidebar-modal__actions">
				<Button variant="outline" onclick={cancelDelete}>No</Button>
				<Button variant="destructive" onclick={confirmDelete}>Yes, delete</Button>
			</div>
		</div>
	</div>
{/if}

<style>
	.sidebar {
		width: 18.25rem;
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		border-right: 1px solid var(--color-border);
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--color-primary) 7%, transparent),
				color-mix(in srgb, var(--color-card) 94%, transparent) 32%
			),
			var(--surface-subtle);
	}

	.sidebar__header {
		padding: 1rem;
		border-bottom: 1px solid var(--color-border);
	}

	.sidebar__header-row {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 0.5rem;
	}

	.sidebar__header h1 {
		font-size: 1.06rem;
		font-weight: 700;
	}

	.sidebar__header p {
		margin-top: 0.3rem;
		font-size: 0.77rem;
		color: var(--color-muted-foreground);
	}

	:global(.sidebar__collapse) {
		flex-shrink: 0;
		border: 1px solid color-mix(in srgb, var(--color-border) 85%, transparent);
		background: color-mix(in srgb, var(--color-card) 94%, transparent);
		box-shadow: var(--shadow-soft);
	}

	.sidebar__action {
		padding: 0.9rem 0.85rem 0.75rem;
		display: grid;
		gap: 0.6rem;
	}

	:global(.sidebar__new-chat) {
		width: 100%;
		height: 2.35rem;
		border-radius: var(--radius-lg);
		background: var(--color-primary);
		box-shadow: var(--shadow-soft);
	}

	.sidebar__search-wrap {
		position: relative;
	}

	.sidebar__search-icon {
		position: absolute;
		left: 0.6rem;
		top: 50%;
		transform: translateY(-50%);
		color: var(--color-muted-foreground);
	}

	.sidebar__search-input {
		width: 100%;
		height: 2.1rem;
		border-radius: var(--radius-lg);
		border: 1px solid var(--color-border);
		padding: 0.45rem 0.55rem 0.45rem 1.9rem;
		font-size: 0.76rem;
		background: color-mix(in srgb, var(--color-card) 96%, transparent);
		color: var(--color-foreground);
		outline: none;
		transition:
			border-color var(--motion-fast),
			box-shadow var(--motion-fast),
			background var(--motion-fast);
	}

	.sidebar__search-input:focus {
		border-color: color-mix(in srgb, var(--color-primary) 55%, var(--color-border));
		box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-primary) 17%, transparent);
	}

	.sidebar__scroll {
		flex: 1;
		overflow-y: auto;
		padding: 0.3rem 0.5rem 0.9rem;
	}

	.sidebar__group {
		margin-bottom: 0.85rem;
	}

	.sidebar__group-title {
		padding: 0.3rem 0.55rem;
		font-size: 0.64rem;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		font-weight: 700;
		color: var(--color-muted-foreground);
	}

	.sidebar__chat-row {
		display: flex;
		align-items: stretch;
		gap: 0.3rem;
		position: relative;
		border-radius: var(--radius-lg);
		padding: 0.2rem;
		margin-bottom: 0.14rem;
		border: 1px solid transparent;
		transition:
			border-color var(--motion-fast),
			background var(--motion-fast);
	}

	.sidebar__chat-row:hover {
		background: color-mix(in srgb, var(--color-accent) 42%, transparent);
	}

	.sidebar__chat-row--active {
		border-color: color-mix(in srgb, var(--color-primary) 46%, transparent);
		background:
			linear-gradient(
				140deg,
				color-mix(in srgb, var(--color-primary) 10%, transparent),
				color-mix(in srgb, var(--color-card) 94%, transparent)
			),
			var(--surface-elevated);
		box-shadow: var(--shadow-soft);
	}

	.sidebar__chat-row--active::before {
		content: '';
		position: absolute;
		left: 0.18rem;
		top: 0.42rem;
		bottom: 0.42rem;
		width: 2px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--color-primary) 84%, white);
	}

	.sidebar__chat-row--archived {
		opacity: 0.92;
	}

	.sidebar__chat-button {
		flex: 1;
		min-width: 0;
		display: grid;
		gap: 0.12rem;
		border: 0;
		background: transparent;
		padding: 0.4rem 0.5rem;
		text-align: left;
		border-radius: var(--radius-md);
		color: inherit;
		cursor: pointer;
	}

	.sidebar__chat-title {
		font-size: 0.79rem;
		font-weight: 600;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.sidebar__chat-meta {
		font-size: 0.66rem;
		color: var(--color-muted-foreground);
	}

	.sidebar__rename-input {
		flex: 1;
		border: 1px solid color-mix(in srgb, var(--color-primary) 50%, var(--color-border));
		border-radius: var(--radius-md);
		padding: 0.35rem 0.45rem;
		font-size: 0.77rem;
		outline: none;
		background: color-mix(in srgb, var(--color-card) 96%, transparent);
	}

	.sidebar__row-actions {
		display: flex;
		align-items: center;
		gap: 0.2rem;
		position: relative;
	}

	.sidebar__icon-btn {
		width: 1.62rem;
		height: 1.62rem;
		border: 0;
		border-radius: var(--radius-md);
		color: var(--color-muted-foreground);
		background: transparent;
		cursor: pointer;
		transition:
			background var(--motion-fast),
			color var(--motion-fast),
			transform var(--motion-fast);
	}

	.sidebar__icon-btn:hover {
		background: color-mix(in srgb, var(--color-accent) 42%, transparent);
		color: var(--color-foreground);
		transform: translateY(-1px);
	}

	.sidebar__menu {
		position: absolute;
		top: calc(100% + 0.2rem);
		right: 0;
		min-width: 9.5rem;
		z-index: 10;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-lg);
		background: color-mix(in srgb, var(--color-card) 99%, transparent);
		box-shadow: var(--shadow-soft);
		padding: 0.28rem;
		display: grid;
		gap: 0.12rem;
	}

	.sidebar__menu button {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		width: 100%;
		border: 0;
		background: transparent;
		padding: 0.35rem 0.42rem;
		border-radius: var(--radius-md);
		font-size: 0.73rem;
		text-align: left;
		color: var(--color-foreground);
		cursor: pointer;
	}

	.sidebar__menu button:hover {
		background: color-mix(in srgb, var(--color-accent) 38%, transparent);
	}

	.sidebar__menu-danger {
		color: color-mix(in srgb, var(--color-destructive) 82%, var(--color-foreground)) !important;
	}

	.sidebar__group--archived {
		margin-top: 0.45rem;
	}

	.sidebar__archived-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.34rem;
		padding: 0.25rem 0.55rem;
		border: 0;
		background: transparent;
		font-size: 0.68rem;
		font-weight: 700;
		color: var(--color-muted-foreground);
		cursor: pointer;
	}

	.sidebar__archived-toggle:hover {
		color: var(--color-foreground);
	}

	.sidebar__archived-block {
		margin-top: 0.4rem;
	}

	.sidebar__archived-block h4 {
		padding: 0.18rem 0.55rem;
		font-size: 0.61rem;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		font-weight: 700;
		color: var(--color-muted-foreground);
	}

	.sidebar__empty {
		padding: 1.2rem;
		text-align: center;
		font-size: 0.8rem;
		color: var(--color-muted-foreground);
		display: grid;
		gap: 0.2rem;
	}

	.sidebar-modal {
		position: fixed;
		inset: 0;
		z-index: 60;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.sidebar-undo {
		position: fixed;
		left: 1rem;
		bottom: 1rem;
		z-index: 70;
		display: inline-flex;
		align-items: center;
		gap: 0.55rem;
		padding: 0.5rem 0.6rem;
		border: 1px solid color-mix(in srgb, var(--color-primary) 36%, var(--color-border));
		border-radius: var(--radius-lg);
		background: color-mix(in srgb, var(--color-card) 97%, transparent);
		box-shadow: var(--shadow-soft);
	}

	.sidebar-undo__text {
		display: inline-flex;
		align-items: baseline;
		gap: 0.35rem;
		font-size: 0.76rem;
	}

	.sidebar-undo__text span {
		color: var(--color-muted-foreground);
	}

	.sidebar-undo__text strong {
		max-width: 11rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	:global(.sidebar-undo__button) {
		height: 1.95rem;
		font-size: 0.72rem;
	}

	.sidebar-modal__backdrop {
		position: absolute;
		inset: 0;
		background: rgb(2 8 23 / 42%);
	}

	.sidebar-modal__card {
		position: relative;
		z-index: 1;
		width: min(92vw, 25rem);
		border-radius: var(--radius-xl);
		border: 1px solid var(--color-border);
		background: var(--color-card);
		padding: 1rem;
		box-shadow: var(--shadow-strong);
	}

	.sidebar-modal__card h4 {
		font-size: 1rem;
		font-weight: 700;
		margin-bottom: 0.4rem;
	}

	.sidebar-modal__card p {
		font-size: 0.82rem;
		color: var(--color-muted-foreground);
		margin-bottom: 0.9rem;
	}

	.sidebar-modal__actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.45rem;
	}

	@media (max-width: 900px) {
		.sidebar {
			width: min(18rem, 86vw);
		}

		.sidebar-undo {
			left: 0.7rem;
			right: 0.7rem;
			bottom: 0.8rem;
			justify-content: space-between;
		}
	}
</style>
