<script lang="ts">
	import { onMount } from 'svelte';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import WorkspaceHeader from '$lib/components/layout/WorkspaceHeader.svelte';
	import WorkspaceControls from '$lib/components/layout/WorkspaceControls.svelte';
	import ConversationView from '$lib/components/chat/ConversationView.svelte';
	import ComposerBar from '$lib/components/chat/ComposerBar.svelte';
	import { chatsStore } from '$lib/stores/chats.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { ragStore } from '$lib/stores/rag.svelte';
	import { blenderStore } from '$lib/stores/blender.svelte';
	import { inferenceStore } from '$lib/stores/inference.svelte';
	import type { Message } from '$lib/types/chat';

	let isSidebarOpen = $state(true);
	let isMobileViewport = $state(false);
	let isWaitingForResponse = $state(false);
	let serverReady = $state(false);
	let appError = $state<string | null>(null);
	let isSwitchingBackend = $state(false);
	let showQuickExamples = $state(true);
	let userHasScrolledUp = $state(false);
	let bottomOffset = $state(0);

	let messagesContainer: HTMLDivElement | undefined = $state();
	let stopRagPolling: (() => void) | null = null;
	let stopBlenderPolling: (() => void) | null = null;
	let cancelRequested = $state(false);
	let currentStreamingChatId = $state<string | null>(null);
	let currentStreamingMessageId = $state<string | null>(null);

	type BackendOption = 'ollama' | 'shared_engine';

	const currentChat = $derived(chatsStore.currentChat);
	const messages = $derived(currentChat?.messages ?? []);
	const pageTitle = $derived(currentChat?.title ?? 'New Blender Chat');
	const currentBackend = $derived(
		ragStore.status.backend === 'ollama' ||
			ragStore.status.backend === 'shared_engine'
			? ragStore.status.backend
			: settingsStore.generationBackend
	);
	const showScrollToLatest = $derived(userHasScrolledUp && messages.length > 0);
	const latestAssistantMessageId = $derived(
		[...messages].reverse().find((message) => message.role === 'assistant')?.id ?? null
	);

	function getErrorMessage(error: unknown, fallback: string): string {
		if (error instanceof Error && error.message.trim()) {
			return error.message;
		}
		if (typeof error === 'string' && error.trim()) {
			return error;
		}
		if (error && typeof error === 'object' && 'message' in error) {
			const candidate = String((error as { message?: unknown }).message ?? '').trim();
			if (candidate) return candidate;
		}
		const rendered = String(error ?? '').trim();
		return rendered || fallback;
	}

	function isBackendOption(value: string): value is BackendOption {
		return value === 'ollama' || value === 'shared_engine';
	}

	function formatBackendLabel(backend: BackendOption): string {
		return backend === 'shared_engine' ? 'ENGINE' : backend.toUpperCase();
	}

	function compactError(message: string): string {
		const cleaned = message.replace(/^Error:\s*/i, '').trim();
		if (cleaned.length <= 96) return cleaned;
		return `${cleaned.slice(0, 93)}...`;
	}

	function conciseBackendError(message: string): string {
		if (message.includes('No shared model artifacts found')) {
			return 'Bundled model files missing. Run `npm run bundle:stage:model` before building.';
		}
		if (message.includes('engine is not available')) {
			return 'Shared engine is not running.';
		}
		return compactError(message);
	}

	function backendAttemptOrder(active: BackendOption): BackendOption[] {
		switch (active) {
			case 'ollama':
				return ['shared_engine'];
			case 'shared_engine':
			default:
				return ['ollama'];
		}
	}

	function setMessagesContainer(element: HTMLDivElement) {
		messagesContainer = element;
	}

	function updateViewportState(source: MediaQueryList | MediaQueryListEvent) {
		isMobileViewport = source.matches;
		if (isMobileViewport) {
			isSidebarOpen = false;
		} else if (!isSidebarOpen) {
			isSidebarOpen = true;
		}
	}

	function isAtBottom(): boolean {
		if (!messagesContainer) return true;
		const threshold = 5;
		const distanceFromBottom =
			messagesContainer.scrollHeight - messagesContainer.scrollTop - messagesContainer.clientHeight;
		return distanceFromBottom <= threshold;
	}

	function markScrollIntentUp() {
		userHasScrolledUp = true;
	}

	function handleScroll() {
		if (isAtBottom()) {
			userHasScrolledUp = false;
		}
	}

	function scrollToBottom() {
		if (!messagesContainer || userHasScrolledUp || !settingsStore.autoScrollChat) return;
		messagesContainer.scrollTop = messagesContainer.scrollHeight;
	}

	function handleScrollToLatest() {
		userHasScrolledUp = false;
		if (!messagesContainer) return;
		messagesContainer.scrollTop = messagesContainer.scrollHeight;
	}

	function isTypingTarget(target: EventTarget | null): boolean {
		if (!(target instanceof HTMLElement)) return false;
		if (target.isContentEditable) return true;
		return (
			target.tagName === 'TEXTAREA' ||
			target.tagName === 'INPUT' ||
			target.tagName === 'SELECT'
		);
	}

	function handleKeyDown(event: KeyboardEvent) {
		const isMac = (navigator as any).userAgentData?.platform?.toUpperCase()?.includes('MAC')
			?? navigator.platform?.toUpperCase()?.includes('MAC')
			?? false;
		const modifierKey = isMac ? event.metaKey : event.ctrlKey;
		const typingInInput = isTypingTarget(event.target);

		if (!typingInInput && modifierKey && event.key === '\\') {
			event.preventDefault();
			isSidebarOpen = !isSidebarOpen;
		}
	}

	function calculateBottomOffset() {
		const visualViewportHeight = window.visualViewport?.height || window.innerHeight;
		const windowHeight = window.innerHeight;
		bottomOffset = Math.max(0, windowHeight - visualViewportHeight);
	}

	async function handleSendMessage(content: string) {
		if (isWaitingForResponse || inferenceStore.isGenerating) return;

		const activeChat = currentChat ?? chatsStore.createChat();
		if (!activeChat) return;

		showQuickExamples = false;
		userHasScrolledUp = false;

		const chatId = activeChat.id;
		const userMessage: Message = {
			id: crypto.randomUUID(),
			role: 'user',
			content,
			timestamp: Date.now()
		};
		chatsStore.addMessage(chatId, userMessage);

		const assistantMessageId = crypto.randomUUID();
		const assistantMessage: Message = {
			id: assistantMessageId,
			role: 'assistant',
			content: '',
			timestamp: Date.now(),
			isStreaming: true
		};
		chatsStore.addMessage(chatId, assistantMessage);
		scrollToBottom();

		isWaitingForResponse = true;
		cancelRequested = false;
		currentStreamingChatId = chatId;
		currentStreamingMessageId = assistantMessageId;

		let accumulatedText = '';

		try {
			const sceneContext = blenderStore.getSceneContext();
			const metrics = await inferenceStore.askQuestionStream(
				content,
				sceneContext,
				(token: string) => {
					if (cancelRequested) return;
					accumulatedText += token;
					chatsStore.updateMessage(chatId, assistantMessageId, {
						content: accumulatedText,
						isStreaming: true
					});
					scrollToBottom();
				}
			);

			const finalContent =
				accumulatedText.length > 0
					? accumulatedText
					: inferenceStore.error
						? `Error: ${inferenceStore.error}`
						: 'Generation cancelled.';

			chatsStore.updateMessage(chatId, assistantMessageId, {
				content: finalContent,
				isStreaming: false
			});

			if (metrics) {
				console.log(
					`[App] Generated ${metrics.total_tokens} tokens at ${metrics.tokens_per_second.toFixed(1)} tok/s`
				);
			}
		} catch (error) {
			const errorMessage = error instanceof Error ? error.message : 'Failed to get response';
			chatsStore.updateMessage(chatId, assistantMessageId, {
				content: `Error: ${errorMessage}`,
				isStreaming: false
			});
		} finally {
			currentStreamingChatId = null;
			currentStreamingMessageId = null;
			isWaitingForResponse = false;
		}
	}

	function handleExampleSelect(prompt: string) {
		handleSendMessage(prompt);
	}

	function findNearestUserPrompt(messageId: string): string | null {
		if (!currentChat) return null;
		const messageIndex = currentChat.messages.findIndex((message) => message.id === messageId);
		if (messageIndex < 0) return null;

		for (let index = messageIndex - 1; index >= 0; index -= 1) {
			const candidate = currentChat.messages[index];
			if (candidate.role === 'user') {
				return candidate.content;
			}
		}

		return null;
	}

	function handleRegenerateMessage(messageId: string) {
		if (inferenceStore.isGenerating) return;
		const sourcePrompt = findNearestUserPrompt(messageId);
		if (!sourcePrompt) return;
		handleSendMessage(sourcePrompt);
	}

	function handleContinueMessage(messageId: string) {
		if (inferenceStore.isGenerating) return;
		const basePrompt = findNearestUserPrompt(messageId);
		const continuationPrompt = basePrompt
			? `Continue your previous response to: "${basePrompt}". Expand with more detail and examples.`
			: 'Continue your previous response with additional detail and examples.';
		handleSendMessage(continuationPrompt);
	}

	function handleBranchFromMessage(messageId: string) {
		if (!currentChat) return;
		const messageIndex = currentChat.messages.findIndex((message) => message.id === messageId);
		if (messageIndex < 0) return;

		const branchSource = currentChat.messages.slice(0, messageIndex + 1);
		if (branchSource.length === 0) return;

		const targetModel = currentChat.model ?? 'blender-assistant';
		const branchChat = chatsStore.createChat(targetModel);

		for (const message of branchSource) {
			chatsStore.addMessage(branchChat.id, {
				...message,
				id: crypto.randomUUID(),
				isStreaming: false
			});
		}

		chatsStore.updateChatTitle(branchChat.id, `${currentChat.title} · Branch`);
		showQuickExamples = false;
		userHasScrolledUp = false;
	}

	async function handleCancelGeneration() {
		cancelRequested = true;

		try {
			await inferenceStore.cancel();
		} catch (error) {
			console.error('[App] Failed to cancel generation:', error);
		}

		if (currentStreamingChatId && currentStreamingMessageId) {
			chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
				isStreaming: false
			});
		}

		currentStreamingChatId = null;
		currentStreamingMessageId = null;
		isWaitingForResponse = false;
	}

	async function toggleBackend() {
		if (isSwitchingBackend) return;

		isSwitchingBackend = true;
		appError = null;

		try {
			const active: BackendOption = isBackendOption(currentBackend) ? currentBackend : 'shared_engine';
			const candidates = backendAttemptOrder(active);
			const failures: Array<{ backend: BackendOption; message: string }> = [];

			for (const target of candidates) {
				try {
					await settingsStore.setGenerationBackend(target);
					await ragStore.checkStatus();
					return;
				} catch (error) {
					failures.push({
						backend: target,
						message: getErrorMessage(error, 'Backend unavailable')
					});
				}
			}

			if (failures.length > 0) {
				const preferredFailure = failures.find((f) => f.backend === 'shared_engine') ?? failures[0];
				appError = `No alternate backend available. ${formatBackendLabel(preferredFailure.backend)}: ${conciseBackendError(preferredFailure.message)}`;
				console.warn('[Backend] Toggle failed for all candidates:', failures);
			} else {
				appError = 'Failed to switch generation backend';
			}
		} catch (error) {
			appError = getErrorMessage(error, 'Failed to switch generation backend');
		} finally {
			isSwitchingBackend = false;
		}
	}

	onMount(() => {
		const media = window.matchMedia('(max-width: 980px)');
		const viewportListener = (event: MediaQueryListEvent) => updateViewportState(event);
		updateViewportState(media);
		media.addEventListener('change', viewportListener);

		calculateBottomOffset();
		const handleResize = () => calculateBottomOffset();
		window.addEventListener('resize', handleResize);
		window.visualViewport?.addEventListener('resize', handleResize);
		window.addEventListener('keydown', handleKeyDown);

		try {
			if (!chatsStore.currentChat && chatsStore.chats.length === 0) {
				chatsStore.createChat();
			} else if (!chatsStore.currentChat && chatsStore.chats.length > 0) {
				chatsStore.setCurrentChat(chatsStore.chats[0].id);
			}
		} catch (error) {
			appError = error instanceof Error ? error.message : 'Error initializing chat';
		}

		(async () => {
			try {
				let retries = 0;
				const maxRetries = 30;

				while (retries < maxRetries && !ragStore.isConnected) {
					await ragStore.checkStatus();
					if (ragStore.isConnected) {
						serverReady = true;
						break;
					}
					await new Promise((resolve) => setTimeout(resolve, 1000));
					retries += 1;
				}

				if (!ragStore.isConnected) {
					serverReady = true;
				}

				stopRagPolling = ragStore.startPolling(settingsStore.pollingInterval);
				stopBlenderPolling = blenderStore.startPolling(5000);
			} catch (error) {
				appError = error instanceof Error ? error.message : 'Error initializing application';
				serverReady = true;
			}
		})();

		return () => {
			media.removeEventListener('change', viewportListener);
			window.removeEventListener('resize', handleResize);
			window.visualViewport?.removeEventListener('resize', handleResize);
			window.removeEventListener('keydown', handleKeyDown);
			if (stopRagPolling) stopRagPolling();
			if (stopBlenderPolling) stopBlenderPolling();
		};
	});

	$effect(() => {
		currentChat?.id;
		userHasScrolledUp = false;
	});

	$effect(() => {
		if (messages.length > 0) {
			scrollToBottom();
		}
	});
</script>

{#if !serverReady}
	<div class="app-loading">
		<div class="app-loading__card">
			<div class="app-loading__spinner"></div>
			<h2>Starting Blender Learning Assistant</h2>
			<p>Initializing AI server</p>
		</div>
	</div>
{:else}
	<div class="app-shell">
		{#if appError}
			<div class="app-shell__error-wrap">
				<div class="app-shell__error">
					<span>{appError}</span>
					<button type="button" onclick={() => (appError = null)} aria-label="Dismiss error">Dismiss</button>
				</div>
			</div>
		{/if}

		<div class={`sidebar-stage ${isSidebarOpen ? 'sidebar-stage--open' : 'sidebar-stage--closed'}`}>
			<Sidebar onClose={() => (isSidebarOpen = false)} />
		</div>

		<div class="workspace-shell">
			<WorkspaceHeader
				title={pageTitle}
				showSidebarToggle={!isSidebarOpen}
				{currentBackend}
				{isSwitchingBackend}
				onOpenSidebar={() => (isSidebarOpen = true)}
				onToggleBackend={toggleBackend}
			/>

			<WorkspaceControls />

			<ConversationView
				{messages}
				showScenePanel={settingsStore.showScenePanel}
				{latestAssistantMessageId}
				{showQuickExamples}
				onSelectExample={handleExampleSelect}
				onToggleExamples={(show) => (showQuickExamples = show)}
				onUserScrollUp={markScrollIntentUp}
				onScroll={handleScroll}
				{showScrollToLatest}
				onScrollToLatest={handleScrollToLatest}
				onRegenerateMessage={handleRegenerateMessage}
				onContinueMessage={handleContinueMessage}
				onBranchFromMessage={handleBranchFromMessage}
				onContainerReady={setMessagesContainer}
			/>

			<ComposerBar
				isGenerating={inferenceStore.isGenerating}
				disabled={isWaitingForResponse || !ragStore.isConnected}
				placeholder={ragStore.isConnected
					? 'Ask about your Blender scene (Shift+Enter for newline)...'
					: 'Waiting for backend connection...'}
				{bottomOffset}
				onSend={handleSendMessage}
				onCancel={handleCancelGeneration}
			/>
		</div>
	</div>
{/if}

<style>
	.app-loading {
		display: grid;
		place-items: center;
		height: 100vh;
		background: var(--surface-canvas);
	}

	.app-loading__card {
		display: grid;
		gap: 0.5rem;
		text-align: center;
	}

	.app-loading__spinner {
		width: 2.5rem;
		height: 2.5rem;
		margin: 0 auto 0.35rem;
		border-radius: 999px;
		border: 3px solid color-mix(in srgb, var(--color-primary) 24%, transparent);
		border-right-color: transparent;
		animation: spin 0.9s linear infinite;
	}

	.app-loading__card h2 {
		font-size: 1.15rem;
		font-weight: 620;
	}

	.app-loading__card p {
		font-size: 0.85rem;
		color: var(--color-muted-foreground);
	}

	.app-shell {
		display: flex;
		height: 100vh;
		overflow: hidden;
		padding: 0.72rem;
		gap: 0.72rem;
		background: var(--surface-canvas);
		position: relative;
	}

	.app-shell::before {
		content: '';
		position: absolute;
		inset: 0;
		pointer-events: none;
		background:
			radial-gradient(
				75rem 38rem at 8% -10%,
				color-mix(in srgb, var(--color-primary) 10%, transparent),
				transparent 66%
			),
			radial-gradient(
				52rem 30rem at 100% 100%,
				color-mix(in srgb, var(--color-primary) 6%, transparent),
				transparent 72%
			);
		mix-blend-mode: screen;
	}

	.app-shell::after {
		content: '';
		position: absolute;
		inset: 0;
		pointer-events: none;
		background:
			linear-gradient(
				180deg,
				rgb(255 255 255 / 6%),
				transparent 12%,
				transparent 88%,
				rgb(0 0 0 / 20%)
			);
		opacity: 0.35;
	}

	.app-shell__error-wrap {
		position: absolute;
		top: 0.95rem;
		left: 50%;
		transform: translateX(-50%);
		z-index: 80;
		width: min(92vw, 40rem);
	}

	.app-shell__error {
		display: flex;
		align-items: center;
		gap: 0.7rem;
		padding: 0.55rem 0.7rem;
		border-radius: var(--radius-lg);
		border: 1px solid color-mix(in srgb, var(--color-destructive) 45%, var(--color-border));
		background: color-mix(in srgb, var(--color-destructive) 12%, transparent);
		color: color-mix(in srgb, var(--color-destructive) 86%, var(--color-foreground));
		font-size: 0.77rem;
	}

	.app-shell__error span {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.app-shell__error button {
		border: 0;
		background: transparent;
		color: inherit;
		font-size: 0.72rem;
		cursor: pointer;
		padding: 0.2rem 0.35rem;
		border-radius: var(--radius-sm);
	}

	.app-shell__error button:hover {
		background: color-mix(in srgb, var(--color-destructive) 18%, transparent);
	}

	.workspace-shell {
		position: relative;
		z-index: 1;
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border-radius: calc(var(--radius-xl) + 8px);
		border: 1px solid var(--outline-soft);
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--surface-elevated) 94%, black),
				var(--surface-subtle) 42%,
				color-mix(in srgb, var(--surface-subtle) 92%, black)
			),
			var(--surface-subtle);
		box-shadow: var(--shadow-strong);
	}

	.sidebar-stage {
		display: flex;
		height: 100%;
		width: 17.75rem;
		min-width: 0;
		overflow: hidden;
		transition: width 145ms cubic-bezier(0.2, 0.86, 0.34, 1);
		will-change: width;
		z-index: 2;
	}

	.sidebar-stage :global(.sidebar) {
		height: 100%;
		transition:
			transform 145ms cubic-bezier(0.2, 0.86, 0.34, 1),
			opacity 120ms ease;
		will-change: transform, opacity;
	}

	.sidebar-stage--closed {
		width: 0;
	}

	.sidebar-stage--closed :global(.sidebar) {
		transform: translateX(-16px);
		opacity: 0;
		pointer-events: none;
	}

	.sidebar-stage--open :global(.sidebar) {
		transform: translateX(0);
		opacity: 1;
		pointer-events: auto;
	}

	.workspace-shell::before {
		content: '';
		position: absolute;
		inset: 0;
		pointer-events: none;
		background: linear-gradient(180deg, rgb(255 255 255 / 4%), transparent 30%);
	}

	@media (max-width: 900px) {
		.app-shell {
			padding: 0.35rem;
			gap: 0.35rem;
		}

		.workspace-shell {
			border-radius: calc(var(--radius-xl) + 4px);
		}

		.sidebar-stage {
			width: min(17.5rem, 86vw);
		}
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
