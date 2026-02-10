<script lang="ts">
	import ModelSelector from '$lib/components/ModelSelector.svelte';
	import ContextToggle from '$lib/components/ContextToggle.svelte';
	import ThemeSelector from '$lib/components/ThemeSelector.svelte';
</script>

<section class="workspace-controls" aria-label="Session controls">
	<div class="workspace-controls__row">
		<ModelSelector />
		<ContextToggle />
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
		border-bottom: 1px solid var(--color-border);
		background: color-mix(in srgb, var(--color-card) 90%, transparent);
		backdrop-filter: blur(8px);
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
