<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import BenchmarkPanel from '$lib/components/BenchmarkPanel.svelte';
	import HardwarePanel from '$lib/components/HardwarePanel.svelte';
	import KeyboardShortcutsOverlay from '$lib/components/KeyboardShortcutsOverlay.svelte';
	import WorkspaceHeader from '$lib/components/layout/WorkspaceHeader.svelte';
	import WorkspaceControls from '$lib/components/layout/WorkspaceControls.svelte';
	import ConversationView from '$lib/components/chat/ConversationView.svelte';
	import ComposerBar from '$lib/components/chat/ComposerBar.svelte';
	import { chatsStore } from '$lib/stores/chats.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { inferenceStore } from '$lib/stores/inference.svelte';
	import { hardwareStore } from '$lib/stores/hardware.svelte';
	import { uiStore } from '$lib/stores/ui.svelte';
	import { applyTheme, watchSystemTheme } from '$lib/utils/theme';
	import type { Message } from '$lib/types/chat';
	import type { GenerationConfig } from '$lib/types/inference';

	let messagesContainer: HTMLDivElement | undefined = $state();
	let cancelRequested = $state(false);
	let currentStreamingChatId = $state<string | null>(null);
	let currentStreamingMessageId = $state<string | null>(null);
	let bottomOffset = $state(0);
	let showShortcutsOverlay = $state(false);

	const currentChat = $derived(chatsStore.currentChat);
	const messages = $derived(currentChat?.messages ?? []);
	const hasNoChats = $derived(chatsStore.chats.length === 0);
	const pageTitle = $derived(currentChat?.title ?? 'New Chat');
	const showBenchmarkPanel = $derived(uiStore.activeOverlay === 'benchmark');
	const showHardwarePanel = $derived(uiStore.activeOverlay === 'hardware');
	const showScrollToLatest = $derived(uiStore.userHasScrolledUp && messages.length > 0);
	const latestAssistantMessageId = $derived(
		[...messages].reverse().find((message) => message.role === 'assistant')?.id ?? null
	);

	function setMessagesContainer(element: HTMLDivElement) {
		messagesContainer = element;
	}

	function isAtBottom(): boolean {
		if (!messagesContainer) return true;
		const threshold = 5;
		const distanceFromBottom =
			messagesContainer.scrollHeight - messagesContainer.scrollTop - messagesContainer.clientHeight;
		return distanceFromBottom <= threshold;
	}

	function markScrollIntentUp() {
		uiStore.setUserHasScrolledUp(true);
	}

	function handleScroll() {
		if (isAtBottom()) {
			uiStore.resetScrollState();
		}
	}

	function scrollToBottom() {
		if (messagesContainer && !uiStore.userHasScrolledUp) {
			messagesContainer.scrollTop = messagesContainer.scrollHeight;
		}
	}

	const SYSTEM_PROMPT =
		'You are a helpful coding assistant for students. ' +
		'Give clear, concise explanations. ' +
		'When showing code, use simple examples and add brief comments.';

	// Build ChatML-formatted prompt for Qwen chat-template style models.
	function buildChatMLPrompt(userMessage: string, historyMessages: Message[]): string {
		let prompt = `<|im_start|>system\n${SYSTEM_PROMPT}<|im_end|>\n`;

		// Include conversation history if context is enabled
		if (settingsStore.contextEnabled) {
			for (const msg of historyMessages) {
				const role = msg.role === 'user' ? 'user' : 'assistant';
				prompt += `<|im_start|>${role}\n${msg.content}<|im_end|>\n`;
			}
		}

		// Add current user message and open assistant turn
		prompt += `<|im_start|>user\n${userMessage}<|im_end|>\n<|im_start|>assistant\n`;

		return prompt;
	}

	function handleScrollToLatest() {
		uiStore.resetScrollState();
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

	async function handleSendMessage(content: string) {
		if (!inferenceStore.isLoaded || inferenceStore.isGenerating) return;

		const activeChat = currentChat ?? chatsStore.createChat(inferenceStore.currentModel ?? 'onnx-model');
		if (!activeChat) return;
		const historyBeforeMessage = [...activeChat.messages];

		uiStore.setShowQuickExamples(false);
		uiStore.resetScrollState();

		const userMessage: Message = {
			id: crypto.randomUUID(),
			role: 'user',
			content,
			timestamp: Date.now()
		};
		chatsStore.addMessage(activeChat.id, userMessage);
		scrollToBottom();

		const assistantMessage: Message = {
			id: crypto.randomUUID(),
			role: 'assistant',
			content: '',
			timestamp: Date.now(),
			isStreaming: true
		};
		chatsStore.addMessage(activeChat.id, assistantMessage);
		scrollToBottom();

		cancelRequested = false;
		currentStreamingChatId = activeChat.id;
		currentStreamingMessageId = assistantMessage.id;

		const chatId = activeChat.id;
		const messageId = assistantMessage.id;

		try {
			const prompt = buildChatMLPrompt(content, historyBeforeMessage);
			const config: Partial<GenerationConfig> = {
				max_length: 2048,
				temperature: settingsStore.temperature,
				top_k: 40,
				top_p: 0.9,
				repetition_penalty: 1.1,
				repetition_penalty_last_n: 64
			};

			await inferenceStore.generateStream(
				prompt,
				(token: string) => {
					if (cancelRequested) return;

					const streamingChat = chatsStore.chats.find((chat) => chat.id === chatId);
					if (!streamingChat) return;

					const streamingMessage = streamingChat.messages.find((msg) => msg.id === messageId);
					if (!streamingMessage || !streamingMessage.isStreaming) return;

					chatsStore.updateMessage(chatId, messageId, {
						content: streamingMessage.content + token
					});

					if (currentChat?.id === chatId) {
						scrollToBottom();
					}
				},
				config
			);

			chatsStore.updateMessage(chatId, messageId, {
				isStreaming: false
			});
		} catch (error) {
			console.error('Generation error:', error);
			chatsStore.updateMessage(chatId, messageId, {
				content: `Error: ${error}`,
				isStreaming: false
			});
		} finally {
			currentStreamingChatId = null;
			currentStreamingMessageId = null;
		}
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
			? `Continue your previous response to: "${basePrompt}". Expand with more details and an example.`
			: 'Continue your previous response with additional detail and examples.';
		handleSendMessage(continuationPrompt);
	}

	function handleBranchFromMessage(messageId: string) {
		if (!currentChat) return;
		const messageIndex = currentChat.messages.findIndex((message) => message.id === messageId);
		if (messageIndex < 0) return;

		const branchSource = currentChat.messages.slice(0, messageIndex + 1);
		if (branchSource.length === 0) return;

		const targetModel = currentChat.model ?? inferenceStore.currentModel ?? 'onnx-model';
		const branchChat = chatsStore.createChat(targetModel);

		for (const message of branchSource) {
			chatsStore.addMessage(branchChat.id, {
				...message,
				id: crypto.randomUUID(),
				isStreaming: false
			});
		}

		chatsStore.updateChatTitle(branchChat.id, `${currentChat.title} · Branch`);
		uiStore.setShowQuickExamples(false);
		uiStore.resetScrollState();
	}

	async function handleExportChat() {
		if (!currentChat || currentChat.messages.length === 0) return;

		const markdown = [
			`# ${currentChat.title}`,
			'',
			`Exported: ${new Date().toLocaleString()}`,
			`Model: ${currentChat.model}`,
			'',
			...currentChat.messages.flatMap((message) => [
				`## ${message.role === 'user' ? 'User' : 'Assistant'} • ${new Date(message.timestamp).toLocaleString()}`,
				'',
				message.content,
				''
			])
		].join('\n');

		try {
			await invoke('save_code', { code: markdown });
		} catch (error) {
			console.error('Failed to export chat:', error);
		}
	}

	function handleExampleSelect(prompt: string) {
		handleSendMessage(prompt);
	}

	async function handleCancelGeneration() {
		cancelRequested = true;

		try {
			await inferenceStore.cancel();
		} catch (error) {
			console.error('Failed to cancel generation:', error);
		}

		if (currentStreamingChatId && currentStreamingMessageId) {
			chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
				isStreaming: false
			});
		}

		currentStreamingChatId = null;
		currentStreamingMessageId = null;
	}

	function handleKeyDown(event: KeyboardEvent) {
		const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
		const modifierKey = isMac ? event.metaKey : event.ctrlKey;
		const typingInInput = isTypingTarget(event.target);

		if (modifierKey && event.shiftKey && event.key.toLowerCase() === 'b') {
			event.preventDefault();
			uiStore.toggleOverlay('benchmark');
			return;
		}

		if (modifierKey && event.key === '\\') {
			event.preventDefault();
			uiStore.toggleSidebar();
			return;
		}

		if (modifierKey && event.key === '/') {
			event.preventDefault();
			showShortcutsOverlay = !showShortcutsOverlay;
			return;
		}

		if (!typingInInput && event.key === '?') {
			event.preventDefault();
			showShortcutsOverlay = true;
			return;
		}

		if (event.key === 'Escape') {
			if (showShortcutsOverlay) {
				showShortcutsOverlay = false;
				event.preventDefault();
				return;
			}

			if (uiStore.activeOverlay !== 'none') {
				uiStore.closeOverlay();
				event.preventDefault();
			}
		}
	}

	function calculateBottomOffset() {
		const visualViewportHeight = window.visualViewport?.height || window.innerHeight;
		const windowHeight = window.innerHeight;
		bottomOffset = Math.max(0, windowHeight - visualViewportHeight);
	}

	onMount(() => {
		calculateBottomOffset();

		const handleResize = () => calculateBottomOffset();
		window.addEventListener('resize', handleResize);
		window.visualViewport?.addEventListener('resize', handleResize);

		async function initInference() {
			await inferenceStore.listModels();
			await inferenceStore.syncStatus();

			if (inferenceStore.availableModels.length === 0) {
				return;
			}

			const availableModelIds = new Set(
				inferenceStore.availableModels.map((model) => model.id)
			);
			let targetModelId = settingsStore.selectedModel;
			if (!availableModelIds.has(targetModelId)) {
				targetModelId = inferenceStore.availableModels[0].id;
			}

			if (inferenceStore.currentModel && availableModelIds.has(inferenceStore.currentModel)) {
				settingsStore.setModel(inferenceStore.currentModel);
				return;
			}

			const loaded = await inferenceStore.loadModel(targetModelId);
			if (loaded) {
				settingsStore.setModel(targetModelId);
			}
		}

		initInference();
		hardwareStore.getCached();

		if (hasNoChats) {
			chatsStore.createChat(inferenceStore.currentModel ?? 'onnx-model');
		}

		window.addEventListener('keydown', handleKeyDown);

		return () => {
			window.removeEventListener('keydown', handleKeyDown);
			window.removeEventListener('resize', handleResize);
			window.visualViewport?.removeEventListener('resize', handleResize);
		};
	});

	$effect(() => {
		currentChat?.id;
		uiStore.resetScrollState();
	});

	$effect(() => {
		if (messages.length > 0) {
			scrollToBottom();
		}
	});

	$effect(() => {
		const theme = settingsStore.theme;
		applyTheme(theme);
		if (theme !== 'system') {
			return;
		}

		const unwatch = watchSystemTheme(() => applyTheme(theme));
		return () => unwatch();
	});
</script>

<div class="app-shell">
	<div class={`sidebar-stage ${uiStore.isSidebarOpen ? 'sidebar-stage--open' : 'sidebar-stage--closed'}`}>
		<Sidebar onClose={() => uiStore.setSidebarOpen(false)} />
	</div>

	<div class="workspace-shell">
		<WorkspaceHeader
			title={pageTitle}
			showSidebarToggle={!uiStore.isSidebarOpen}
			status={inferenceStore.status}
			hardwareActive={showHardwarePanel}
			shortcutsOpen={showShortcutsOverlay}
			canExport={messages.length > 0}
			onOpenSidebar={() => uiStore.setSidebarOpen(true)}
			onToggleHardware={() => uiStore.toggleOverlay('hardware')}
			onToggleShortcuts={() => (showShortcutsOverlay = !showShortcutsOverlay)}
			onExportChat={handleExportChat}
		/>

		<WorkspaceControls />

		<ConversationView
			{messages}
			{latestAssistantMessageId}
			showQuickExamples={uiStore.showQuickExamples}
			onSelectExample={handleExampleSelect}
			onToggleExamples={(show) => uiStore.setShowQuickExamples(show)}
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
			isLoaded={inferenceStore.isLoaded}
			isGenerating={inferenceStore.isGenerating}
			{bottomOffset}
			onSend={handleSendMessage}
			onCancel={handleCancelGeneration}
		/>
	</div>

	<BenchmarkPanel visible={showBenchmarkPanel} onClose={() => uiStore.closeOverlay()} />
	<HardwarePanel visible={showHardwarePanel} onClose={() => uiStore.closeOverlay()} />
	<KeyboardShortcutsOverlay open={showShortcutsOverlay} onClose={() => (showShortcutsOverlay = false)} />
</div>

<style>
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
		background: linear-gradient(
			180deg,
			rgb(255 255 255 / 4%),
			transparent 30%
		);
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
</style>
