<script lang="ts">
	import type { OllamaStatus } from '$lib/types/ollama';

	interface Props {
		status: OllamaStatus;
	}

	let { status }: Props = $props();
</script>

<div class="flex items-center gap-2 rounded-md border px-3 py-2">
	<div
		class={`h-3 w-3 rounded-full ${status.connected ? 'bg-green-500' : 'bg-red-500'} ${status.checking ? 'animate-pulse' : ''}`}
	></div>
	<span class="text-sm font-medium">
		{#if status.checking}
			Checking Ollama...
		{:else if status.connected}
			Ollama Connected
		{:else}
			Ollama Disconnected
		{/if}
	</span>
	{#if status.error}
		<span class="text-xs text-muted-foreground">({status.error})</span>
	{/if}
</div>
