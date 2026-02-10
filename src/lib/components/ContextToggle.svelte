<script lang="ts">
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { MessageSquareCode, MessageSquareOff } from '@lucide/svelte';

	function handleToggle() {
		settingsStore.toggleContext();
	}
</script>

<button
	onclick={handleToggle}
	class={`context-toggle ${settingsStore.contextEnabled ? 'context-toggle--enabled' : 'context-toggle--disabled'}`}
	title={settingsStore.contextEnabled
		? 'Context enabled - AI remembers conversation'
		: 'Context disabled - Each message is independent'}
	aria-label={settingsStore.contextEnabled ? 'Disable context memory' : 'Enable context memory'}
>
	{#if settingsStore.contextEnabled}
		<MessageSquareCode class="h-4 w-4" />
		<span>Context On</span>
	{:else}
		<MessageSquareOff class="h-4 w-4" />
		<span>Context Off</span>
	{/if}
</button>

<style>
	.context-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		padding: 0.45rem 0.68rem;
		border-radius: var(--radius-xl);
		border: 1px solid color-mix(in srgb, var(--color-border) 88%, transparent);
		font-size: 0.81rem;
		font-weight: 650;
		cursor: pointer;
		box-shadow: var(--shadow-soft);
		transition:
			transform var(--motion-fast),
			border-color var(--motion-fast),
			background var(--motion-fast);
	}

	.context-toggle:hover {
		transform: translateY(-1px);
	}

	.context-toggle--enabled {
		color: color-mix(in srgb, var(--color-success) 85%, var(--color-foreground));
		border-color: color-mix(in srgb, var(--color-success) 58%, var(--color-border));
		background: color-mix(in srgb, var(--color-success) 11%, transparent);
	}

	.context-toggle--disabled {
		color: var(--color-muted-foreground);
		background: color-mix(in srgb, var(--color-card) 96%, transparent);
	}
</style>
