<script lang="ts">
	import { onMount } from 'svelte';
	import type { Message } from '$lib/types/chat';
	import {
		renderMarkdown,
		copyToClipboard,
		extractCode,
		setupCodeCopyHandlers
	} from '$lib/utils/markdown';
	import { invoke } from '@tauri-apps/api/core';
	import { User, Bot, Copy, Check, Download } from '@lucide/svelte';

	// src/lib/components/ChatMessage.svelte 9-30
	interface Props {
		message: Message;
	}

	let { message }: Props = $props();

	let copied = $state(false);
	let contentContainer: HTMLDivElement;

	const renderedContent = $derived(renderMarkdown(message.content));
	const codeBlocks = $derived(extractCode(message.content));

	// Combine all code blocks into one string
	const allCode = $derived(codeBlocks.join('\n\n'));

	async function handleCopyAllCode() {
		const success = await copyToClipboard(allCode);
		if (success) {
			copied = true;
			setTimeout(() => (copied = false), 2000);
		}
	}

	async function handleSaveAllCode() {
		try {
			await invoke('save_code', { code: allCode });
		} catch (error) {
			console.error('Failed to save code:', error);
			alert('Failed to save file. Please try again.');
		}
	}

	// Setup event delegation for copy buttons (CSP-compliant)
	onMount(() => {
		if (contentContainer) {
			return setupCodeCopyHandlers(contentContainer);
		}
	});
</script>

<div
	class="flex gap-3 rounded-lg p-4 transition-colors {message.role === 'user'
		? 'bg-blue-50 dark:bg-blue-950/20'
		: 'bg-gray-50 dark:bg-gray-900/50'}"
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
	<div class="min-w-0 flex-1" bind:this={contentContainer}>
		<div class="mb-1 text-sm font-semibold text-gray-700 dark:text-gray-300">
			{message.role === 'user' ? 'You' : 'AI Assistant'}
		</div>

		<div
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

		<!-- Unified Code Actions -->
		{#if message.role === 'assistant' && codeBlocks.length > 0 && !message.isStreaming}
			<div class="mt-3 flex gap-2">
				<button
					type="button"
					onclick={handleCopyAllCode}
					class="flex items-center gap-1 rounded border border-gray-300 bg-white px-3 py-1.5 text-xs hover:bg-gray-100 dark:border-gray-700 dark:bg-gray-800 dark:hover:bg-gray-700"
					title="Copy all code from this message"
				>
					{#if copied}
						<Check class="h-3 w-3 text-green-600" />
						<span class="text-green-600">Copied!</span>
					{:else}
						<Copy class="h-3 w-3" />
						<span>Copy All Code</span>
					{/if}
				</button>
				<button
					type="button"
					onclick={handleSaveAllCode}
					class="flex items-center gap-1 rounded border border-gray-300 bg-white px-3 py-1.5 text-xs hover:bg-gray-100 dark:border-gray-700 dark:bg-gray-800 dark:hover:bg-gray-700"
					title="Save all code from this message to file"
				>
					<Download class="h-3 w-3" />
					<span>Save All Code</span>
				</button>
			</div>
		{/if}
	</div>
</div>

<style>
	/* Custom prose styling for code blocks */
	:global(.code-block) {
		margin-top: 1rem;
		margin-bottom: 1rem;
		overflow: hidden;
		border-radius: 0.5rem;
	}

	:global(.prose code) {
		border-radius: 0.25rem;
		padding-left: 0.375rem;
		padding-right: 0.375rem;
		padding-top: 0.125rem;
		padding-bottom: 0.125rem;
	}

	:global(.prose pre) {
		border-radius: 0.5rem;
	}

	:global(.prose p) {
		margin-top: 0.5rem;
		margin-bottom: 0.5rem;
	}

	:global(.prose ul) {
		margin-top: 0.5rem;
		margin-bottom: 0.5rem;
		padding-left: 0.5rem;
	}

	:global(.prose ol) {
		margin-top: 0.5rem;
		margin-bottom: 0.5rem;
		padding-left: 0.5rem;
	}

	:global(.prose li) {
		margin-left: 1.5rem;
	}
</style>
