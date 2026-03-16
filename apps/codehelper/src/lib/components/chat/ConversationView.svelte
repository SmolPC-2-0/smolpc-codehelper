<script lang="ts">
	import ChatMessage from '$lib/components/ChatMessage.svelte';
	import WelcomeState from '$lib/components/chat/WelcomeState.svelte';
	import { ArrowDown } from '@lucide/svelte';
	import type { Message } from '$lib/types/chat';
	import type { AppMode } from '$lib/types/mode';
	import type { ProviderStateDto } from '$lib/types/provider';

	interface Props {
		mode: AppMode;
		modeLabel: string;
		modeSubtitle: string;
		suggestions: string[];
		providerState?: ProviderStateDto | null;
		statusLabel?: string | null;
		statusDetail?: string | null;
		messages: Message[];
		latestAssistantMessageId: string | null;
		showQuickExamples: boolean;
		disabledExamples?: boolean;
		disabledReason?: string | null;
		onSelectExample: (prompt: string) => void;
		onToggleExamples: (show: boolean) => void;
		onUserScrollUp: () => void;
		onScroll: () => void;
		showScrollToLatest: boolean;
		onScrollToLatest: () => void;
		onRegenerateMessage: (messageId: string) => void;
		onContinueMessage: (messageId: string) => void;
		onBranchFromMessage: (messageId: string) => void;
		onUndoMessage: (messageId: string) => void;
		onContainerReady: (element: HTMLDivElement) => void;
	}

	let {
		mode,
		modeLabel,
		modeSubtitle,
		suggestions,
		providerState = null,
		statusLabel = null,
		statusDetail = null,
		messages,
		latestAssistantMessageId,
		showQuickExamples,
		disabledExamples = false,
		disabledReason = null,
		onSelectExample,
		onToggleExamples,
		onUserScrollUp,
		onScroll,
		showScrollToLatest,
		onScrollToLatest,
		onRegenerateMessage,
		onContinueMessage,
		onBranchFromMessage,
		onUndoMessage,
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
	role="region"
	aria-label="Conversation messages"
	bind:this={scrollContainer}
	onscroll={onScroll}
	onwheel={handleWheel}
	ontouchstart={handleTouchStart}
	ontouchmove={handleTouchMove}
>
	<div class="conversation-view__inner">
		{#if messages.length === 0}
			<WelcomeState
				{mode}
				{modeLabel}
				{modeSubtitle}
				{suggestions}
				{providerState}
				{statusLabel}
				{statusDetail}
				{showQuickExamples}
				{disabledExamples}
				{disabledReason}
				{onSelectExample}
				{onToggleExamples}
			/>
		{:else}
			<div class="conversation-view__messages">
				{#each messages as message (message.id)}
					<ChatMessage
						{mode}
						{message}
						canRegenerate={message.id === latestAssistantMessageId}
						onRegenerate={() => onRegenerateMessage(message.id)}
						onContinue={() => onContinueMessage(message.id)}
						onBranchFromHere={() => onBranchFromMessage(message.id)}
						onUndo={() => onUndoMessage(message.id)}
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
		padding: 1rem 1rem 0.9rem;
		scroll-padding-bottom: 2rem;
		position: relative;
		background: linear-gradient(
			90deg,
			rgb(255 255 255 / 1.2%) 0,
			transparent 24%,
			transparent 76%,
			rgb(255 255 255 / 1.2%) 100%
		);
	}

	.conversation-view__inner {
		max-width: 68rem;
		margin: 0 auto;
	}

	.conversation-view__messages {
		display: grid;
		gap: 0.78rem;
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
		border: 1px solid var(--outline-strong);
		background: color-mix(in srgb, var(--surface-widget) 94%, black);
		color: var(--color-foreground);
		font-size: 0.69rem;
		font-weight: 650;
		box-shadow: var(--glow-subtle);
		cursor: pointer;
		z-index: 2;
		backdrop-filter: blur(10px);
	}

	.conversation-view__jump-latest:hover {
		background: color-mix(in srgb, var(--surface-active) 90%, black);
	}

	@media (max-width: 768px) {
		.conversation-view {
			padding: 0.8rem;
		}
	}
</style>
