<script lang="ts">
	import type { InferenceStatus } from '$lib/types/inference';

	interface Props {
		status: InferenceStatus;
	}

	let { status }: Props = $props();
</script>

<div class="flex items-center gap-2 rounded-md border px-3 py-2">
	<div
		class={`h-3 w-3 rounded-full ${status.isLoaded ? 'bg-green-500' : status.isGenerating ? 'bg-yellow-500 animate-pulse' : 'bg-gray-400'}`}
	></div>
	<span class="text-sm font-medium">
		{#if status.isGenerating}
			Generating...
		{:else if status.isLoaded}
			{status.currentModel ?? 'Model Ready'}
		{:else}
			No Model Loaded
		{/if}
	</span>
	{#if status.error}
		<span class="text-xs text-red-500">({status.error})</span>
	{/if}
</div>
