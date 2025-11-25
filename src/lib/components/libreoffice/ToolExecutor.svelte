<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import type { MCPTool, ToolCallResult } from '$lib/types/libreoffice';

	interface Props {
		tool: MCPTool | null;
		disabled: boolean;
		onCallTool: (toolName: string, args: Record<string, unknown>) => Promise<ToolCallResult>;
	}

	let { tool, disabled, onCallTool }: Props = $props();

	let argsJson = $state('{}');
	let executing = $state(false);
	let result = $state<ToolCallResult | null>(null);
	let parseError = $state<string | null>(null);

	// Reset when tool changes
	$effect(() => {
		if (tool) {
			argsJson = '{}';
			result = null;
			parseError = null;
		}
	});

	async function handleExecute() {
		if (!tool) return;

		parseError = null;
		let args: Record<string, unknown>;

		try {
			args = JSON.parse(argsJson);
		} catch {
			parseError = 'Invalid JSON';
			return;
		}

		executing = true;
		result = null;

		try {
			result = await onCallTool(tool.name, args);
		} finally {
			executing = false;
		}
	}
</script>

<div class="rounded-lg border bg-white p-4 dark:border-gray-700 dark:bg-gray-800">
	{#if !tool}
		<div class="text-center text-gray-500 dark:text-gray-400">
			Select a tool from the list to execute it.
		</div>
	{:else}
		<h3 class="mb-2 font-semibold text-gray-900 dark:text-white">{tool.name}</h3>
		<p class="mb-4 text-sm text-gray-600 dark:text-gray-400">{tool.description}</p>

		<!-- Input Schema hint -->
		<div class="mb-3">
			<label class="mb-1 block text-sm text-gray-600 dark:text-gray-400">
				Input Schema
			</label>
			<pre class="max-h-32 overflow-auto rounded bg-gray-100 p-2 text-xs dark:bg-gray-900">
{JSON.stringify(tool.inputSchema, null, 2)}</pre>
		</div>

		<!-- Arguments input -->
		<div class="mb-3">
			<label class="mb-1 block text-sm text-gray-600 dark:text-gray-400">
				Arguments (JSON)
			</label>
			<textarea
				bind:value={argsJson}
				disabled={disabled || executing}
				rows={4}
				class="w-full rounded-md border bg-white px-3 py-2 font-mono text-sm dark:border-gray-600 dark:bg-gray-700 dark:text-white"
				placeholder="Enter JSON arguments"
			></textarea>
			{#if parseError}
				<p class="mt-1 text-xs text-red-500">{parseError}</p>
			{/if}
		</div>

		<!-- Execute button -->
		<Button
			onclick={handleExecute}
			disabled={disabled || executing}
			class="w-full"
		>
			{executing ? 'Executing...' : 'Execute Tool'}
		</Button>

		<!-- Result -->
		{#if result}
			<div class="mt-4">
				<label class="mb-1 block text-sm text-gray-600 dark:text-gray-400">Result</label>
				<div
					class={`rounded-md p-3 ${
						result.success
							? 'bg-green-100 dark:bg-green-900/30'
							: 'bg-red-100 dark:bg-red-900/30'
					}`}
				>
					{#if result.success}
						<pre class="overflow-auto text-xs text-green-800 dark:text-green-400">
{JSON.stringify(result.result, null, 2)}</pre>
					{:else}
						<p class="text-sm text-red-800 dark:text-red-400">{result.error}</p>
					{/if}
				</div>
			</div>
		{/if}
	{/if}
</div>