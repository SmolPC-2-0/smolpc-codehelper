<script lang="ts">
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { ollamaStore } from '$lib/stores/ollama.svelte';
	import { Brain } from '@lucide/svelte';

	function handleModelChange(event: Event) {
		const target = event.target as HTMLSelectElement;
		settingsStore.setModel(target.value);
	}
</script>

<div class="flex items-center gap-2 rounded-lg border border-gray-200 bg-white px-3 py-2 dark:border-gray-700 dark:bg-gray-900">
	<Brain class="h-4 w-4 text-gray-600 dark:text-gray-400" />
	<select
		value={settingsStore.selectedModel}
		onchange={handleModelChange}
		class="flex-1 bg-transparent text-sm text-gray-700 outline-none dark:text-gray-300"
	>
		{#if ollamaStore.availableModels.length > 0}
			{#each ollamaStore.availableModels as model}
				<option value={model}>
					{model}
				</option>
			{/each}
		{:else}
			<option value={settingsStore.selectedModel}>
				{settingsStore.selectedModel} (No models detected)
			</option>
		{/if}
	</select>
</div>
