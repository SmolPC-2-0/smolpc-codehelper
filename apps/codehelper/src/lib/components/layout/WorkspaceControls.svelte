<script lang="ts">
	import type { Snippet } from 'svelte';
	import { CircleHelp } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import ContextToggle from '$lib/components/ContextToggle.svelte';
	import ThemeSelector from '$lib/components/ThemeSelector.svelte';

	interface Props {
		leadingContent?: Snippet;
		helpOpen?: boolean;
		onToggleHelp?: () => void;
	}

	let { leadingContent, helpOpen = false, onToggleHelp }: Props = $props();
</script>

<section class="workspace-controls" aria-label="Session controls">
	<div class="workspace-controls__row">
		{#if leadingContent}
			{@render leadingContent()}
		{/if}
		<ContextToggle />
	</div>
	<div class="workspace-controls__row workspace-controls__row--compact">
		<ThemeSelector />
		<Button
			variant="ghost"
			size="sm"
			onclick={() => onToggleHelp?.()}
			class={`workspace-controls__help ${helpOpen ? 'workspace-controls__help--active' : ''}`}
			aria-label="Open mode help"
			title="Open mode help"
		>
			<CircleHelp class="h-4 w-4" />
			<span>Help</span>
		</Button>
	</div>
</section>

<style>
	.workspace-controls {
		position: relative;
		z-index: 10;
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

	:global(.workspace-controls__help) {
		display: inline-flex;
		align-items: center;
		gap: 0.38rem;
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
		box-shadow: var(--glow-subtle);
	}

	:global(.workspace-controls__help--active) {
		border-color: var(--outline-strong);
		background: var(--surface-active);
	}

	@media (max-width: 768px) {
		.workspace-controls {
			padding: 0.7rem 0.8rem;
		}

		.workspace-controls__row--compact {
			margin-left: 0;
			width: 100%;
		}
	}
</style>
