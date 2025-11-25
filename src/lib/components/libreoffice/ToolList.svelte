<script lang="ts">
	import type { MCPTool } from '$lib/types/libreoffice';

	interface Props {
		tools: MCPTool[];
		selectedTool: MCPTool | null;
		onSelectTool: (tool: MCPTool) => void;
	}

	let { tools, selectedTool, onSelectTool }: Props = $props();
	let searchQuery = $state('');

	let filteredTools = $derived(
		tools.filter(
			(tool) =>
				tool.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
				tool.description.toLowerCase().includes(searchQuery.toLowerCase())
		)
	);
</script>

<div class="flex h-full flex-col">
	<!-- Search -->
	<div class="border-b p-3 dark:border-gray-700">
		<input
			type="text"
			placeholder="Search tools..."
			bind:value={searchQuery}
			class="w-full rounded-md border bg-white px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-white"
		/>
	</div>

	<!-- Tool list -->
	<div class="flex-1 overflow-y-auto">
		{#if filteredTools.length === 0}
			<div class="p-4 text-center text-sm text-gray-500">
				{tools.length === 0 ? 'No tools available. Connect first.' : 'No matching tools.'}
			</div>
		{:else}
			{#each filteredTools as tool (tool.name)}
				<button
					class={`w-full border-b p-3 text-left transition-colors hover:bg-gray-50 dark:border-gray-700 dark:hover:bg-gray-800 ${
						selectedTool?.name === tool.name ? 'bg-blue-50 dark:bg-blue-900/20' : ''
					}`}
					onclick={() => onSelectTool(tool)}
				>
					<div class="font-medium text-sm text-gray-900 dark:text-white">
						{tool.name}
					</div>
					<div class="text-xs text-gray-500 dark:text-gray-400 line-clamp-2">
						{tool.description}
					</div>
				</button>
			{/each}
		{/if}
	</div>

	<!-- Tool count -->
	<div class="border-t p-2 text-center text-xs text-gray-500 dark:border-gray-700">
		{filteredTools.length} / {tools.length} tools
	</div>
</div>