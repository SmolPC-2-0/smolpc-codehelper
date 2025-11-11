<script lang="ts">
	import type { Message } from '$lib/types/chat';
	import { renderMarkdown, copyToClipboard, extractCode } from '$lib/utils/markdown';
	import { invoke } from '@tauri-apps/api/core';
	import { User, Bot, Copy, Check, Download } from '@lucide/svelte';
	import { onMount } from 'svelte';

	interface Props {
		message: Message;
	}

	let { message }: Props = $props();

	let messageElement: HTMLDivElement;
	let copiedCode: string | null = $state(null);

	const renderedContent = $derived(renderMarkdown(message.content));
	const codeBlocks = $derived(extractCode(message.content));

	async function handleCopyCode(code: string) {
		const success = await copyToClipboard(code);
		if (success) {
			copiedCode = code;
			setTimeout(() => (copiedCode = null), 2000);
		}
	}

	async function handleSaveCode(code: string) {
		try {
			await invoke('save_code', { code });
		} catch (error) {
			console.error('Failed to save code:', error);
			alert('Failed to save file. Please try again.');
		}
	}

	onMount(() => {
		// Add click handlers to dynamically created copy buttons
		if (messageElement) {
			const copyButtons = messageElement.querySelectorAll('.copy-code-btn');
			copyButtons.forEach((btn) => {
				btn.addEventListener('click', (e) => {
					const code = (e.target as HTMLElement).getAttribute('data-code');
					if (code) handleCopyCode(code);
				});
			});
		}
	});
</script>

<div
	class="flex gap-3 rounded-lg p-4 transition-colors"
	class:bg-blue-50={message.role === 'user'}
	class:dark:bg-blue-950/20={message.role === 'user'}
	class:bg-gray-50={message.role === 'assistant'}
	class:dark:bg-gray-900/50={message.role === 'assistant'}
>
	<!-- Avatar -->
	<div class="flex-shrink-0">
		<div
			class="flex h-8 w-8 items-center justify-center rounded-full"
			class:bg-blue-600={message.role === 'user'}
			class:bg-green-600={message.role === 'assistant'}
		>
			{#if message.role === 'user'}
				<User class="h-5 w-5 text-white" />
			{:else}
				<Bot class="h-5 w-5 text-white" />
			{/if}
		</div>
	</div>

	<!-- Content -->
	<div class="flex-1 overflow-hidden">
		<div class="mb-1 text-sm font-semibold text-gray-700 dark:text-gray-300">
			{message.role === 'user' ? 'You' : 'AI Assistant'}
		</div>

		<div
			bind:this={messageElement}
			class="prose prose-sm dark:prose-invert max-w-none break-words text-gray-800 dark:text-gray-200"
		>
			{@html renderedContent}
		</div>

		{#if message.isStreaming}
			<div class="mt-2 flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
				<span class="inline-block h-2 w-2 animate-pulse rounded-full bg-green-600"></span>
				Generating...
			</div>
		{/if}

		<!-- Code Actions -->
		{#if message.role === 'assistant' && codeBlocks.length > 0 && !message.isStreaming}
			<div class="mt-3 flex flex-wrap gap-2">
				{#each codeBlocks as code, index}
					<div class="flex gap-1">
						<button
							onclick={() => handleCopyCode(code)}
							class="flex items-center gap-1 rounded border border-gray-300 bg-white px-2 py-1 text-xs hover:bg-gray-100 dark:border-gray-700 dark:bg-gray-800 dark:hover:bg-gray-700"
							title="Copy code block {index + 1}"
						>
							{#if copiedCode === code}
								<Check class="h-3 w-3 text-green-600" />
								<span>Copied!</span>
							{:else}
								<Copy class="h-3 w-3" />
								<span>Copy Code {codeBlocks.length > 1 ? index + 1 : ''}</span>
							{/if}
						</button>
						<button
							onclick={() => handleSaveCode(code)}
							class="flex items-center gap-1 rounded border border-gray-300 bg-white px-2 py-1 text-xs hover:bg-gray-100 dark:border-gray-700 dark:bg-gray-800 dark:hover:bg-gray-700"
							title="Save code block {index + 1} to file"
						>
							<Download class="h-3 w-3" />
							<span>Save</span>
						</button>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>

<style>
	/* Custom prose styling for code blocks */
	:global(.code-block) {
		@apply my-4 overflow-hidden rounded-lg;
	}

	:global(.prose code) {
		@apply rounded px-1 py-0.5;
	}

	:global(.prose pre) {
		@apply rounded-lg;
	}

	:global(.prose p) {
		@apply my-2;
	}

	:global(.prose ul) {
		@apply my-2;
	}

	:global(.prose ol) {
		@apply my-2;
	}
</style>
