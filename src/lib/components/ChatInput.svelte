<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Send } from '@lucide/svelte';

	interface Props {
		onSend: (message: string) => void;
		disabled?: boolean;
		placeholder?: string;
	}

	let { onSend, disabled = false, placeholder = 'Ask a coding question...' }: Props = $props();

	let inputValue = $state('');
	let textarea: HTMLTextAreaElement;

	function handleSubmit() {
		const trimmed = inputValue.trim();
		if (trimmed && !disabled) {
			onSend(trimmed);
			inputValue = '';
			if (textarea) {
				textarea.style.height = 'auto';
			}
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSubmit();
		}
	}

	function handleInput() {
		if (textarea) {
			textarea.style.height = 'auto';
			textarea.style.height = textarea.scrollHeight + 'px';
		}
	}
</script>

<div class="flex items-end gap-2">
	<div class="relative flex-1">
		<textarea
			bind:this={textarea}
			bind:value={inputValue}
			onkeydown={handleKeydown}
			oninput={handleInput}
			{placeholder}
			{disabled}
			rows="1"
			class="w-full resize-none rounded-lg border border-gray-300 bg-white px-4 py-3 pr-12 text-sm text-gray-900 outline-none focus:border-blue-600 focus:ring-2 focus:ring-blue-600/20 disabled:bg-gray-100 disabled:text-gray-500 dark:border-gray-700 dark:bg-gray-900 dark:text-white dark:focus:border-blue-600 dark:disabled:bg-gray-800"
			style="max-height: 200px; overflow-y: auto;"
		></textarea>
	</div>
	<Button onclick={handleSubmit} {disabled} size="icon" class="h-12 w-12">
		<Send class="h-5 w-5" />
	</Button>
</div>

<style>
	textarea::-webkit-scrollbar {
		width: 8px;
	}

	textarea::-webkit-scrollbar-track {
		background: transparent;
	}

	textarea::-webkit-scrollbar-thumb {
		background: #cbd5e0;
		border-radius: 4px;
	}

	textarea::-webkit-scrollbar-thumb:hover {
		background: #a0aec0;
	}
</style>
