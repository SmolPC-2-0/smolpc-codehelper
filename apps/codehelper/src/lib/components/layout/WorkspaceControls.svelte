<script lang="ts">
	import type { Snippet } from 'svelte';
	import ContextToggle from '$lib/components/ContextToggle.svelte';
	import ThemeSelector from '$lib/components/ThemeSelector.svelte';

	interface Props {
		leadingContent?: Snippet;
	}

	let { leadingContent }: Props = $props();
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
