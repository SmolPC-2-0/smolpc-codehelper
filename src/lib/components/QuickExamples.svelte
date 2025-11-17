<script lang="ts">
	import { QUICK_EXAMPLES } from '$lib/types/examples';
	import { Lightbulb, X } from '@lucide/svelte';

	interface Props {
		onSelectExample: (prompt: string) => void;
		onClose?: () => void;
	}

	let { onSelectExample, onClose }: Props = $props();

	function handleSelect(prompt: string) {
		onSelectExample(prompt);
		if (onClose) onClose();
	}
</script>

<div class="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900">
	<div class="mb-3 flex items-center justify-between">
		<div class="flex items-center gap-2">
			<Lightbulb class="h-5 w-5 text-yellow-600 dark:text-yellow-400" />
			<h3 class="text-sm font-semibold text-gray-900 dark:text-white">Quick Examples</h3>
		</div>
		{#if onClose}
			<button
				onclick={onClose}
				class="rounded p-1 hover:bg-gray-100 dark:hover:bg-gray-800"
				aria-label="Close examples"
			>
				<X class="h-4 w-4 text-gray-600 dark:text-gray-400" />
			</button>
		{/if}
	</div>

	<div class="grid gap-2 sm:grid-cols-2">
		{#each QUICK_EXAMPLES as example (example.id)}
			<button
				onclick={() => handleSelect(example.prompt)}
				class="group rounded-lg border border-gray-200 p-3 text-left transition-all hover:border-blue-600 hover:bg-blue-50 dark:border-gray-700 dark:hover:border-blue-600 dark:hover:bg-blue-950/20"
			>
				<div class="mb-1 text-sm font-semibold text-gray-900 dark:text-white">
					{example.title}
				</div>
				<div class="line-clamp-2 text-xs text-gray-600 dark:text-gray-400">
					{example.prompt}
				</div>
			</button>
		{/each}
	</div>
</div>
