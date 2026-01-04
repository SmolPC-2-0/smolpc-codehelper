<script lang="ts">
	import { inferenceStore } from '$lib/stores/inference.svelte';
	import { Brain, Loader2 } from '@lucide/svelte';

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

<div class="flex items-center gap-2 rounded-lg border border-gray-200 bg-white px-3 py-2 dark:border-gray-700 dark:bg-gray-900">
	{#if isLoading}
		<Loader2 class="h-4 w-4 animate-spin text-gray-600 dark:text-gray-400" />
	{:else}
		<Brain class="h-4 w-4 text-gray-600 dark:text-gray-400" />
	{/if}
	<select
		value={inferenceStore.currentModel ?? ''}
		onchange={handleModelChange}
		disabled={isLoading || inferenceStore.isGenerating}
		class="flex-1 bg-transparent text-sm text-gray-700 outline-none disabled:opacity-50 dark:text-gray-300"
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
