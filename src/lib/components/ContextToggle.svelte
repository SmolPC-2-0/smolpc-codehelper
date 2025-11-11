<script lang="ts">
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { MessageSquare, MessageSquareOff } from '@lucide/svelte';

	function handleToggle() {
		settingsStore.toggleContext();
	}
</script>

<button
	onclick={handleToggle}
	class="flex items-center gap-2 rounded-lg border px-3 py-2 text-sm transition-colors"
	class:border-green-600={settingsStore.contextEnabled}
	class:bg-green-50={settingsStore.contextEnabled}
	class:dark:bg-green-950/20={settingsStore.contextEnabled}
	class:border-gray-300={!settingsStore.contextEnabled}
	class:bg-gray-50={!settingsStore.contextEnabled}
	class:dark:border-gray-700={!settingsStore.contextEnabled}
	class:dark:bg-gray-900={!settingsStore.contextEnabled}
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
