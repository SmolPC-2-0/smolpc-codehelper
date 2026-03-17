<script lang="ts">
	import ContextToggle from '$lib/components/ContextToggle.svelte';
	import ThemeSelector from '$lib/components/ThemeSelector.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Wrench } from '@lucide/svelte';

	interface Props {
		showContextControls?: boolean;
		setupNeedsAttention?: boolean;
		onOpenSetup?: () => void;
	}

	let { showContextControls = true, setupNeedsAttention = false, onOpenSetup }: Props = $props();
</script>

<section class="workspace-controls" aria-label="Session controls">
	{#if showContextControls}
		<div class="workspace-controls__row">
			<ContextToggle />
		</div>
	{/if}
	<div class="workspace-controls__row workspace-controls__row--compact">
		<Button variant="outline" class="workspace-controls__setup" onclick={() => onOpenSetup?.()}>
			<Wrench class="h-4 w-4" />
			<span>Setup</span>
			{#if setupNeedsAttention}
				<span class="workspace-controls__badge">Needs attention</span>
			{/if}
		</Button>
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

	:global(.workspace-controls__setup) {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
	}

	.workspace-controls__badge {
		display: inline-flex;
		align-items: center;
		padding: 0.12rem 0.42rem;
		border-radius: 999px;
		font-size: 0.65rem;
		font-weight: 700;
		color: color-mix(in srgb, var(--color-primary) 72%, var(--color-foreground));
		background: color-mix(in srgb, var(--brand-soft) 74%, transparent);
		border: 1px solid color-mix(in srgb, var(--color-primary) 18%, transparent);
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
