<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import type { ToolCallResult } from '$lib/types/libreoffice';

	interface Props {
		disabled: boolean;
		onCreateDocument: (filename: string, title?: string, docType?: string) => Promise<ToolCallResult>;
	}

	let { disabled, onCreateDocument }: Props = $props();

	let filename = $state('');
	let title = $state('');
	let docType = $state<'text' | 'spreadsheet' | 'presentation'>('text');
	let creating = $state(false);
	let result = $state<ToolCallResult | null>(null);

	async function handleCreate() {
		if (!filename.trim()) return;

		creating = true;
		result = null;

		try {
			result = await onCreateDocument(
				filename.trim(),
				title.trim() || undefined,
				docType
			);

			if (result.success) {
				filename = '';
				title = '';
			}
		} finally {
			creating = false;
		}
	}

	function getFileExtension(type: string): string {
		switch (type) {
			case 'text': return '.odt';
			case 'spreadsheet': return '.ods';
			case 'presentation': return '.odp';
			default: return '.odt';
		}
	}
</script>

<div class="rounded-lg border bg-white p-4 dark:border-gray-700 dark:bg-gray-800">
	<h3 class="mb-4 font-semibold text-gray-900 dark:text-white">Create Document</h3>

	<div class="space-y-3">
		<!-- Document type -->
		<div>
			<label class="mb-1 block text-sm text-gray-600 dark:text-gray-400">Type</label>
			<select
				bind:value={docType}
				disabled={disabled || creating}
				class="w-full rounded-md border bg-white px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-700 dark:text-white"
			>
				<option value="text">Text Document (.odt)</option>
				<option value="spreadsheet">Spreadsheet (.ods)</option>
				<option value="presentation">Presentation (.odp)</option>
			</select>
		</div>

		<!-- Filename -->
		<div>
			<label class="mb-1 block text-sm text-gray-600 dark:text-gray-400">Filename</label>
			<div class="flex">
				<input
					type="text"
					bind:value={filename}
					disabled={disabled || creating}
					placeholder="my-document"
					class="flex-1 rounded-l-md border bg-white px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-700 dark:text-white"
				/>
				<span class="flex items-center rounded-r-md border border-l-0 bg-gray-100 px-3 text-sm text-gray-500 dark:border-gray-600 dark:bg-gray-600 dark:text-gray-300">
					{getFileExtension(docType)}
				</span>
			</div>
		</div>

		<!-- Title (optional) -->
		<div>
			<label class="mb-1 block text-sm text-gray-600 dark:text-gray-400">Title (optional)</label>
			<input
				type="text"
				bind:value={title}
				disabled={disabled || creating}
				placeholder="Document Title"
				class="w-full rounded-md border bg-white px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-700 dark:text-white"
			/>
		</div>

		<!-- Create button -->
		<Button
			onclick={handleCreate}
			disabled={disabled || creating || !filename.trim()}
			class="w-full"
		>
			{creating ? 'Creating...' : 'Create Document'}
		</Button>

		<!-- Result message -->
		{#if result}
			<div
				class={`rounded-md p-2 text-sm ${
					result.success
						? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400'
						: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400'
				}`}
			>
				{result.success ? 'Document created successfully!' : result.error}
			</div>
		{/if}
	</div>
</div>