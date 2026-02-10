<script lang="ts">
	import { inferenceStore } from '$lib/stores/inference.svelte';
	import { BrainCircuit, Loader2 } from '@lucide/svelte';

	let isLoading = $state(false);

	async function handleModelChange(event: Event) {
		const target = event.target as HTMLSelectElement;
		const modelId = target.value;

		// Don't reload if same model
		if (modelId === inferenceStore.currentModel) return;

		isLoading = true;
		try {
			await inferenceStore.loadModel(modelId);
		} finally {
			isLoading = false;
		}
	}
</script>

<div class="model-selector">
	{#if isLoading}
		<Loader2 class="model-selector__icon model-selector__icon--loading" />
	{:else}
		<BrainCircuit class="model-selector__icon" />
	{/if}
	<select
		value={inferenceStore.currentModel ?? ''}
		onchange={handleModelChange}
		disabled={isLoading || inferenceStore.isGenerating}
		class="model-selector__control"
		aria-label="Select inference model"
	>
		{#if inferenceStore.availableModels.length > 0}
			{#each inferenceStore.availableModels as model}
				<option value={model.id}>
					{model.name} ({model.size})
				</option>
			{/each}
		{:else}
			<option value="">No models found</option>
		{/if}
	</select>
</div>

<style>
	.model-selector {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		min-width: 17rem;
		padding: 0.45rem 0.68rem;
		border-radius: var(--radius-xl);
		border: 1px solid color-mix(in srgb, var(--color-border) 88%, transparent);
		background: color-mix(in srgb, var(--color-card) 96%, transparent);
		box-shadow: var(--shadow-soft);
	}

	:global(.model-selector__icon) {
		width: 0.95rem;
		height: 0.95rem;
		color: var(--color-muted-foreground);
		flex-shrink: 0;
	}

	:global(.model-selector__icon--loading) {
		animation: spin 1s linear infinite;
	}

	.model-selector__control {
		flex: 1;
		font-size: 0.82rem;
		background: transparent;
		color: var(--color-foreground);
		outline: none;
		border: none;
		appearance: none;
		padding-right: 0.4rem;
	}

	.model-selector__control:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	@media (max-width: 768px) {
		.model-selector {
			min-width: 14.5rem;
			width: 100%;
		}
	}
</style>
