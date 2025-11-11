<script lang="ts">
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { MessageSquare, MessageSquareOff } from '@lucide/svelte';

	function handleToggle() {
		settingsStore.toggleContext();
	}
</script>

<button
	onclick={handleToggle}
	class="flex items-center gap-2 rounded-lg border px-3 py-2 text-sm transition-colors {settingsStore.contextEnabled ? 'border-green-600 bg-green-50 dark:bg-green-950/20' : 'border-gray-300 bg-gray-50 dark:border-gray-700 dark:bg-gray-900'}"
	title={settingsStore.contextEnabled
		? 'Context enabled - AI remembers conversation'
		: 'Context disabled - Each message is independent'}
>
	{#if settingsStore.contextEnabled}
		<MessageSquare class="h-4 w-4 text-green-600 dark:text-green-400" />
		<span class="font-medium text-green-700 dark:text-green-300">Context On</span>
	{:else}
		<MessageSquareOff class="h-4 w-4 text-gray-600 dark:text-gray-400" />
		<span class="font-medium text-gray-700 dark:text-gray-300">Context Off</span>
	{/if}
</button>
