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
	import { Bot, Check, Copy, Download, GitBranchPlus, RefreshCw, User, Waypoints } from '@lucide/svelte';

	// src/lib/components/ChatMessage.svelte 9-30
	interface Props {
		message: Message;
		canRegenerate?: boolean;
		showAssistantActions?: boolean;
		onRegenerate?: () => void;
		onContinue?: () => void;
		onBranchFromHere?: () => void;
	}

	let {
		message,
		canRegenerate = false,
		showAssistantActions = true,
		onRegenerate = () => {},
		onContinue = () => {},
		onBranchFromHere = () => {}
	}: Props = $props();

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

<article class={`chat-message ${message.role === 'user' ? 'chat-message--user' : 'chat-message--assistant'}`}>
	<div class="chat-message__avatar">
		{#if message.role === 'user'}
			<User class="h-4 w-4" />
		{:else}
			<Bot class="h-4 w-4" />
		{/if}
	</div>

	<div class="chat-message__body" bind:this={contentContainer}>
		<header class="chat-message__meta">
			<span class="chat-message__role">{message.role === 'user' ? 'You' : 'Assistant'}</span>
			<span class="chat-message__time">{new Date(message.timestamp).toLocaleTimeString([], {
				hour: '2-digit',
				minute: '2-digit'
			})}</span>
		</header>

		<div class="chat-message__content prose prose-sm max-w-none break-words">
			{@html renderedContent}
		</div>

		{#if message.isStreaming}
			<div class="chat-message__streaming">
				<span class="chat-message__streaming-dot"></span>
				Generating...
			</div>
		{/if}

		{#if message.role === 'assistant' && !message.isStreaming && showAssistantActions}
			<div class="chat-message__actions">
				{#if canRegenerate}
					<button
						type="button"
						onclick={onRegenerate}
						class="chat-message__action"
						title="Regenerate this response"
					>
						<RefreshCw class="h-3 w-3" />
						<span>Regenerate</span>
					</button>
				{/if}

				<button
					type="button"
					onclick={onContinue}
					class="chat-message__action"
					title="Continue this response"
				>
					<Waypoints class="h-3 w-3" />
					<span>Continue</span>
				</button>

				<button
					type="button"
					onclick={onBranchFromHere}
					class="chat-message__action"
					title="Branch this conversation from here"
				>
					<GitBranchPlus class="h-3 w-3" />
					<span>Branch Chat</span>
				</button>

				{#if codeBlocks.length > 0}
				<button
					type="button"
					onclick={handleCopyAllCode}
					class="chat-message__action"
					title="Copy all code from this message"
				>
					{#if copied}
						<Check class="h-3 w-3 chat-message__action-icon--success" />
						<span class="chat-message__action-text--success">Copied!</span>
					{:else}
						<Copy class="h-3 w-3" />
						<span>Copy All Code</span>
					{/if}
				</button>
				<button
					type="button"
					onclick={handleSaveAllCode}
					class="chat-message__action"
					title="Save all code from this message to file"
				>
					<Download class="h-3 w-3" />
					<span>Save All Code</span>
				</button>
				{/if}
			</div>
		{/if}
	</div>
</article>

<style>
	.chat-message {
		display: flex;
		gap: 0.8rem;
		padding: 0.85rem 0.85rem 0.8rem;
		border: 1px solid var(--outline-soft);
		border-radius: calc(var(--radius-xl) + 1px);
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--surface-widget) 97%, black),
				color-mix(in srgb, var(--surface-subtle) 97%, black)
			),
			var(--surface-widget);
		box-shadow: var(--glow-subtle);
		animation: message-in var(--motion-medium);
	}

	.chat-message--user {
		border-color: color-mix(in srgb, var(--color-primary) 30%, var(--color-border));
		background:
			linear-gradient(
				140deg,
				color-mix(in srgb, var(--brand-soft) 72%, transparent),
				var(--surface-widget)
			),
			var(--surface-elevated);
	}

	.chat-message--assistant {
		border-color: var(--outline-soft);
	}

	.chat-message__avatar {
		width: 2rem;
		height: 2rem;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		border-radius: 999px;
		background: color-mix(in srgb, var(--color-primary) 82%, black);
		color: var(--color-primary-foreground);
		border: 1px solid color-mix(in srgb, var(--color-primary) 46%, transparent);
	}

	.chat-message--assistant .chat-message__avatar {
		background: color-mix(in srgb, var(--color-muted) 82%, black);
		color: var(--color-muted-foreground);
		border: 1px solid var(--outline-soft);
	}

	.chat-message__body {
		min-width: 0;
		flex: 1;
	}

	.chat-message__meta {
		display: flex;
		align-items: center;
		gap: 0.55rem;
		margin-bottom: 0.3rem;
	}

	.chat-message__role {
		font-size: 0.75rem;
		font-weight: 640;
		letter-spacing: 0.01em;
	}

	.chat-message__time {
		font-size: 0.66rem;
		color: var(--color-muted-foreground);
	}

	.chat-message__content {
		color: var(--color-foreground);
	}

	.chat-message__streaming {
		margin-top: 0.6rem;
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		font-size: 0.73rem;
		color: var(--color-muted-foreground);
	}

	.chat-message__streaming-dot {
		width: 0.46rem;
		height: 0.46rem;
		border-radius: 999px;
		background: var(--color-success);
		animation: pulse-dot 1.2s ease-in-out infinite;
	}

	.chat-message__actions {
		margin-top: 0.72rem;
		display: flex;
		flex-wrap: wrap;
		gap: 0.38rem;
	}

	.chat-message__action {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		padding: 0.32rem 0.56rem;
		border-radius: var(--radius-md);
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 96%, black);
		font-size: 0.67rem;
		font-weight: 620;
		color: var(--color-muted-foreground);
		cursor: pointer;
		transition:
			color var(--motion-fast),
			background var(--motion-fast),
			border-color var(--motion-fast),
			transform var(--motion-fast);
	}

	.chat-message__action:hover {
		color: var(--color-foreground);
		border-color: var(--outline-strong);
		background: var(--surface-active);
		transform: translateY(-0.5px);
	}

	.chat-message__action-icon--success,
	.chat-message__action-text--success {
		color: var(--color-success);
	}

	@keyframes message-in {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	@keyframes pulse-dot {
		0%,
		100% {
			opacity: 0.45;
		}
		50% {
			opacity: 1;
		}
	}

	/* Custom prose styling for code blocks */
	:global(.code-block) {
		margin-top: 0.8rem;
		margin-bottom: 0.8rem;
		overflow: hidden;
		border-radius: var(--radius-lg);
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
	}

	:global(.code-block-head) {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.45rem 0.7rem;
		border-bottom: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-hover) 70%, black);
	}

	:global(.code-block-lang) {
		font-size: 0.67rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.07em;
		color: var(--color-muted-foreground);
	}

	:global(.code-copy-btn-frame) {
		padding: 0.2rem;
		border: 0;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--color-muted-foreground);
		cursor: pointer;
	}

	:global(.code-copy-btn-frame:hover) {
		background: color-mix(in srgb, var(--surface-hover) 80%, black);
		color: var(--color-foreground);
	}

	:global(.code-block-pre) {
		padding: 0.75rem;
		overflow-x: auto;
	}

	:global(.code-block-code) {
		font-size: 0.8rem;
		font-family: var(--font-code, 'JetBrains Mono', monospace);
		color: var(--color-foreground);
	}

	:global(.inline-code) {
		padding: 0.12rem 0.3rem;
		border-radius: var(--radius-sm);
		background: color-mix(in srgb, var(--surface-hover) 70%, black);
		color: var(--color-foreground);
		font-size: 0.78rem;
	}

	:global(.markdown-link) {
		color: var(--color-primary);
		text-decoration: underline;
		text-decoration-thickness: 0.08em;
		text-underline-offset: 0.12em;
	}

	:global(.prose code) {
		border-radius: 0.35rem;
		padding-left: 0.375rem;
		padding-right: 0.375rem;
		padding-top: 0.125rem;
		padding-bottom: 0.125rem;
		background: var(--surface-hover);
		font-family: var(--font-code, 'JetBrains Mono', monospace);
	}

	:global(.prose pre) {
		border-radius: var(--radius-lg);
		background: color-mix(in srgb, var(--color-card) 88%, transparent);
	}

	:global(.prose p) {
		margin-top: 0.4rem;
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
