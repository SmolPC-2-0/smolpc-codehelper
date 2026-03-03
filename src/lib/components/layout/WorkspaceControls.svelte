<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import ContextToggle from '$lib/components/ContextToggle.svelte';
	import ThemeSelector from '$lib/components/ThemeSelector.svelte';
	import { chatsStore } from '$lib/stores/chats.svelte';

	const currentChat = $derived(chatsStore.currentChat);
	const currentWorkspacePath = $derived(currentChat?.workspacePath ?? null);

	function workspaceDisplayName(path: string): string {
		const segments = path.split(/[\\/]/).filter(Boolean);
		return segments[segments.length - 1] ?? path;
	}

	async function handleChooseWorkspace() {
		if (!currentChat) return;
		try {
			const selected = await invoke<string | null>('pick_workspace_folder');
			if (selected) {
				chatsStore.setWorkspacePath(currentChat.id, selected);
			}
		} catch (error) {
			console.error('Failed to choose workspace folder:', error);
			alert('Failed to choose workspace folder. Please try again.');
		}
	}

	function handleClearWorkspace() {
		if (!currentChat) return;
		chatsStore.setWorkspacePath(currentChat.id, null);
	}
</script>

<section class="workspace-controls" aria-label="Session controls">
	<div class="workspace-controls__row">
		<ContextToggle />
		<div class="workspace-controls__workspace">
			<button
				type="button"
				onclick={handleChooseWorkspace}
				class="workspace-controls__workspace-button"
				title="Select workspace folder for this chat"
				disabled={!currentChat}
			>
				{currentWorkspacePath ? 'Change Workspace' : 'Choose Workspace'}
			</button>

			{#if currentWorkspacePath}
				<button
					type="button"
					onclick={handleClearWorkspace}
					class="workspace-controls__workspace-clear"
					title="Clear workspace for this chat"
				>
					Clear
				</button>
				<span class="workspace-controls__workspace-path" title={currentWorkspacePath}>
					{workspaceDisplayName(currentWorkspacePath)}
				</span>
			{:else}
				<span class="workspace-controls__workspace-empty">No workspace selected</span>
			{/if}
		</div>
	</div>
	<div class="workspace-controls__row workspace-controls__row--compact">
		<ThemeSelector />
	</div>
</section>

<style>
	.workspace-controls {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		padding: 0.8rem 1rem;
		border-bottom: 1px solid var(--outline-soft);
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--surface-widget) 98%, black),
				var(--surface-subtle)
			),
			var(--surface-subtle);
		backdrop-filter: blur(10px);
	}

	.workspace-controls__row {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.6rem;
		min-width: 0;
	}

	.workspace-controls__row--compact {
		margin-left: auto;
	}

	.workspace-controls__workspace {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		min-width: 0;
	}

	.workspace-controls__workspace-button,
	.workspace-controls__workspace-clear {
		border: 1px solid var(--outline-soft);
		border-radius: var(--radius-md);
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
		color: var(--color-muted-foreground);
		font-size: 0.7rem;
		font-weight: 620;
		cursor: pointer;
		transition:
			color var(--motion-fast),
			border-color var(--motion-fast),
			background var(--motion-fast);
	}

	.workspace-controls__workspace-button {
		padding: 0.34rem 0.56rem;
	}

	.workspace-controls__workspace-clear {
		padding: 0.34rem 0.5rem;
	}

	.workspace-controls__workspace-button:hover,
	.workspace-controls__workspace-clear:hover {
		color: var(--color-foreground);
		border-color: var(--outline-strong);
		background: var(--surface-active);
	}

	.workspace-controls__workspace-button:disabled {
		opacity: 0.55;
		cursor: not-allowed;
	}

	.workspace-controls__workspace-path {
		max-width: 11.5rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 0.69rem;
		color: var(--color-muted-foreground);
	}

	.workspace-controls__workspace-empty {
		font-size: 0.69rem;
		color: var(--color-muted-foreground);
	}

	@media (max-width: 768px) {
		.workspace-controls {
			padding: 0.7rem 0.8rem;
		}

		.workspace-controls__row--compact {
			margin-left: 0;
			width: 100%;
		}

		.workspace-controls__workspace-path {
			max-width: 8.5rem;
		}
	}
</style>
