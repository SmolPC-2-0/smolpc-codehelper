<script lang="ts">
	import ChatMessage from '$lib/components/ChatMessage.svelte';
	import WelcomeState from '$lib/components/chat/WelcomeState.svelte';
	import { ArrowDown } from '@lucide/svelte';
	import type { Message } from '$lib/types/chat';

	interface Props {
		messages: Message[];
		latestAssistantMessageId: string | null;
		showQuickExamples: boolean;
		onSelectExample: (prompt: string) => void;
		onToggleExamples: (show: boolean) => void;
		onUserScrollUp: () => void;
		onScroll: () => void;
		showScrollToLatest: boolean;
		onScrollToLatest: () => void;
		onRegenerateMessage: (messageId: string) => void;
		onContinueMessage: (messageId: string) => void;
		onBranchFromMessage: (messageId: string) => void;
		onContainerReady: (element: HTMLDivElement) => void;
	}

	let {
		messages,
		latestAssistantMessageId,
		showQuickExamples,
		onSelectExample,
		onToggleExamples,
		onUserScrollUp,
		onScroll,
		showScrollToLatest,
		onScrollToLatest,
		onRegenerateMessage,
		onContinueMessage,
		onBranchFromMessage,
		onContainerReady
	}: Props = $props();

	let touchStartY = $state(0);
	let scrollContainer: HTMLDivElement;

	$effect(() => {
		if (scrollContainer) {
			onContainerReady(scrollContainer);
		}
	});

	function handleWheel(event: WheelEvent) {
		if (event.deltaY < 0) {
			onUserScrollUp();
		}
	}

	function handleTouchStart(event: TouchEvent) {
		touchStartY = event.touches[0].clientY;
	}

	function handleTouchMove(event: TouchEvent) {
		const touchY = event.touches[0].clientY;
		const deltaY = touchStartY - touchY;
		if (deltaY < 0) {
			onUserScrollUp();
		}
	}
</script>

<div
	class="conversation-view"
	bind:this={scrollContainer}
	onscroll={onScroll}
	onwheel={handleWheel}
	ontouchstart={handleTouchStart}
	ontouchmove={handleTouchMove}
>
	<div class="conversation-view__inner">
		{#if messages.length === 0}
			<WelcomeState
				{showQuickExamples}
				{onSelectExample}
				{onToggleExamples}
			/>
		{:else}
			<div class="conversation-view__messages">
				{#each messages as message (message.id)}
					<ChatMessage
						{message}
						canRegenerate={message.id === latestAssistantMessageId}
						onRegenerate={() => onRegenerateMessage(message.id)}
						onContinue={() => onContinueMessage(message.id)}
						onBranchFromHere={() => onBranchFromMessage(message.id)}
					/>
				{/each}
			</div>
		{/if}
	</div>

	{#if showScrollToLatest}
		<button type="button" class="conversation-view__jump-latest" onclick={onScrollToLatest}>
			<ArrowDown class="h-3.5 w-3.5" />
			Latest messages
		</button>
	{/if}
</div>

<style>
	.conversation-view {
		flex: 1;
		overflow-y: auto;
		padding: 1rem;
		scroll-padding-bottom: 2rem;
		position: relative;
	}

	.conversation-view__inner {
		max-width: 66rem;
		margin: 0 auto;
	}

	.conversation-view__messages {
		display: grid;
		gap: 0.95rem;
	}

	.conversation-view__jump-latest {
		position: sticky;
		bottom: 0.7rem;
		margin-left: auto;
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.45rem 0.65rem;
		border-radius: var(--radius-lg);
		border: 1px solid color-mix(in srgb, var(--color-primary) 55%, var(--color-border));
		background: color-mix(in srgb, var(--color-card) 96%, transparent);
		color: var(--color-foreground);
		font-size: 0.72rem;
		font-weight: 700;
		box-shadow: var(--shadow-soft);
		cursor: pointer;
		z-index: 2;
	}

	.conversation-view__jump-latest:hover {
		background: color-mix(in srgb, var(--color-primary) 12%, transparent);
	}

	@media (max-width: 768px) {
		.conversation-view {
			padding: 0.8rem;
		}
	}
</style>
