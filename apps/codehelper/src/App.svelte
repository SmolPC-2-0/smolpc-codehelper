<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import BenchmarkPanel from '$lib/components/BenchmarkPanel.svelte';
	import HardwarePanel from '$lib/components/HardwarePanel.svelte';
	import ModelInfoPanel from '$lib/components/ModelInfoPanel.svelte';
	import KeyboardShortcutsOverlay from '$lib/components/KeyboardShortcutsOverlay.svelte';
	import WorkspaceHeader from '$lib/components/layout/WorkspaceHeader.svelte';
	import WorkspaceControls from '$lib/components/layout/WorkspaceControls.svelte';
	import AppModeDropdown from '$lib/components/layout/AppModeDropdown.svelte';
	import ConversationView from '$lib/components/chat/ConversationView.svelte';
	import ComposerBar from '$lib/components/chat/ComposerBar.svelte';
	import SetupBanner from '$lib/components/setup/SetupBanner.svelte';
	import SetupPanel from '$lib/components/setup/SetupPanel.svelte';
	import { chatsStore } from '$lib/stores/chats.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { inferenceStore } from '$lib/stores/inference.svelte';
	import { hardwareStore } from '$lib/stores/hardware.svelte';
	import { uiStore } from '$lib/stores/ui.svelte';
	import { modeStore } from '$lib/stores/mode.svelte';
	import { setupStore } from '$lib/stores/setup.svelte';
	import { assistantSend, assistantCancel } from '$lib/api/unified';
	import { applyTheme, watchSystemTheme } from '$lib/utils/theme';
	import type { Message } from '$lib/types/chat';
	import type { GenerationConfig, InferenceChatMessage } from '$lib/types/inference';
	import type { AppMode } from '$lib/types/mode';
	import type { AssistantStreamEvent } from '$lib/types/assistant';

	let messagesContainer: HTMLDivElement | undefined = $state();
	let activeStreamSessionId = $state<string | null>(null);
	let currentStreamingChatId = $state<string | null>(null);
	let currentStreamingMessageId = $state<string | null>(null);
	let bottomOffset = $state(0);
	let showShortcutsOverlay = $state(false);
	let showSetupPanel = $state(false);
	let isSwitchingMode = $state(false);

	// Unified mode state
	const activeMode = $derived(modeStore.activeMode);
	const activeModeConfigs = $derived(modeStore.modeConfigs);
	const setupNeedsAttention = $derived(setupStore.initialized && setupStore.needsAttention);
	const setupStatus = $derived(setupStore.status);
	const setupError = $derived(setupStore.error);

	const currentChat = $derived(chatsStore.currentChat);
	const messages = $derived(currentChat?.messages ?? []);
	const hasNoChats = $derived(chatsStore.chats.length === 0);
	const pageTitle = $derived(currentChat?.title ?? 'New Chat');
	const showBenchmarkPanel = $derived(uiStore.activeOverlay === 'benchmark');
	const showHardwarePanel = $derived(uiStore.activeOverlay === 'hardware');
	const showModelInfoPanel = $derived(uiStore.activeOverlay === 'modelInfo');
	const showScrollToLatest = $derived(uiStore.userHasScrolledUp && messages.length > 0);
	const hasActiveStream = $derived(currentStreamingMessageId !== null);
	const isCancelling = $derived(inferenceStore.cancelState === 'pending');
	const isInferenceBusy = $derived(inferenceStore.isGenerating || isCancelling || hasActiveStream);
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

	const SYSTEM_PROMPT = `You are a rigorous coding tutor and engineering collaborator for secondary students.

Tone:
- Professional, calm, and clear.
- Friendly but not childish.
- No hype, flattery, or filler.

Response priorities:
- Solve the request directly.
- For create/build/write/implement/fix requests: start with runnable code.
- After code, give a short explanation (3-6 bullets max).
- Ask one concise clarifying question only if ambiguity changes the solution.

Quality rules:
- Be technically precise and state key assumptions.
- Use concrete examples when useful.
- Do not output generic planning checklists.
- Do not repeat phrases, sections, or the same plan.
- End once the answer is complete; do not restart the response.

Teaching rules:
- Treat the user as a capable learner.
- Explain difficult parts clearly, without oversimplifying.
`;

	function buildStructuredMessages(
		userMessage: string,
		historyMessages: Message[]
	): InferenceChatMessage[] {
		const payload: InferenceChatMessage[] = [
			{
				role: 'system',
				content: SYSTEM_PROMPT
			}
		];

		if (settingsStore.contextEnabled) {
			for (const msg of historyMessages) {
				payload.push({
					role: msg.role === 'user' ? 'user' : 'assistant',
					content: msg.content
				});
			}
		}

		payload.push({
			role: 'user',
			content: userMessage
		});
		return payload;
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
			target.tagName === 'TEXTAREA' || target.tagName === 'INPUT' || target.tagName === 'SELECT'
		);
	}

	function appendToken(chatId: string, messageId: string, sessionId: string, token: string) {
		if (activeStreamSessionId !== sessionId) return;
		const chat = chatsStore.chats.find((c) => c.id === chatId);
		if (!chat) return;
		const msg = chat.messages.find((m) => m.id === messageId);
		if (!msg || !msg.isStreaming) return;
		chatsStore.updateMessage(chatId, messageId, { content: msg.content + token });
		if (currentChat?.id === chatId) scrollToBottom();
	}

	async function handleSendMessage(content: string) {
		if (activeMode === 'code' && (!inferenceStore.isLoaded || isInferenceBusy)) return;
		if (activeMode !== 'code' && isInferenceBusy) return;

		const activeChat =
			currentChat ?? chatsStore.createChat(inferenceStore.currentModel ?? 'onnx-model');
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

		const streamSessionId = crypto.randomUUID();
		activeStreamSessionId = streamSessionId;
		currentStreamingChatId = activeChat.id;
		currentStreamingMessageId = assistantMessage.id;

		const chatId = activeChat.id;
		const messageId = assistantMessage.id;

			try {
			if (activeMode === 'code') {
				const messagesPayload = buildStructuredMessages(content, historyBeforeMessage);
				const isOpenVinoNpu = inferenceStore.status.activeBackend === 'openvino_npu';
				const config: Partial<GenerationConfig> = {
					max_length: isOpenVinoNpu ? 512 : 2048,
					temperature: isOpenVinoNpu ? 0 : settingsStore.temperature,
					top_k: 40,
					top_p: 0.85,
					repetition_penalty: 1.15,
					repetition_penalty_last_n: 128
				};

				await inferenceStore.generateStreamMessages(
					messagesPayload,
					(token: string) => appendToken(chatId, messageId, streamSessionId, token),
					config
				);
			} else {
				const request = {
					mode: activeMode,
					chatId,
					messages: historyBeforeMessage.map((m) => ({ role: m.role, content: m.content })),
					userText: content
				};

				await assistantSend(request, (event: AssistantStreamEvent) => {
					if (activeStreamSessionId !== streamSessionId) return;
					switch (event.kind) {
						case 'token':
							appendToken(chatId, messageId, streamSessionId, event.token);
							break;
						case 'error':
							chatsStore.updateMessage(chatId, messageId, {
								content: `Error: ${event.message}`
							});
							break;
					}
				});
			}
		} catch (error) {
			console.error('Generation error:', error);
			chatsStore.updateMessage(chatId, messageId, {
				content: `Error: ${error}`,
				isStreaming: false
			});
		} finally {
			chatsStore.updateMessage(chatId, messageId, {
				isStreaming: false
			});

			if (currentStreamingChatId === chatId && currentStreamingMessageId === messageId) {
				currentStreamingChatId = null;
				currentStreamingMessageId = null;
			}

			if (activeStreamSessionId === streamSessionId) {
				activeStreamSessionId = null;
			}
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
		if (isInferenceBusy) return;
		const sourcePrompt = findNearestUserPrompt(messageId);
		if (!sourcePrompt) return;
		handleSendMessage(sourcePrompt);
	}

	function handleContinueMessage(messageId: string) {
		if (isInferenceBusy) return;
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
		activeStreamSessionId = null;

		try {
			if (activeMode === 'code') {
				await inferenceStore.cancel();
			} else {
				await assistantCancel();
			}
		} catch (error) {
			console.error('Failed to cancel generation:', error);
		}
	}

	async function handleModeChange(mode: AppMode) {
		if (activeMode === mode || isSwitchingMode) return;

		isSwitchingMode = true;
		try {
			// Cancel any in-flight generation before switching modes
			if (hasActiveStream || inferenceStore.isGenerating) {
				await handleCancelGeneration();
			}
			modeStore.setActiveMode(mode);
		} finally {
			isSwitchingMode = false;
		}
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
			const availableModelIds = new Set(inferenceStore.availableModels.map((model) => model.id));
			const selectedModelId = availableModelIds.has(settingsStore.selectedModel)
				? settingsStore.selectedModel
				: (inferenceStore.availableModels[0]?.id ?? null);

			await inferenceStore.ensureStarted({
				mode: 'auto',
				startup_policy: selectedModelId ? { default_model_id: selectedModelId } : null
			});
			await inferenceStore.syncStatus();

			if (inferenceStore.currentModel) {
				settingsStore.setModel(inferenceStore.currentModel);
			}
		}

		initInference();
		modeStore.initialize();
		setupStore.initialize();
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
		if (currentChat?.id) {
			uiStore.resetScrollState();
			return;
		}
		uiStore.resetScrollState();
	});

	$effect(() => {
		if (messages.length > 0) {
			scrollToBottom();
		}
	});

	$effect(() => {
		if (inferenceStore.cancelState !== 'timed_out') {
			return;
		}

		activeStreamSessionId = null;

		if (currentStreamingChatId && currentStreamingMessageId) {
			chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
				isStreaming: false
			});
			currentStreamingChatId = null;
			currentStreamingMessageId = null;
		}
	});

	// Engine health polling — immediate check + 10s interval
	$effect(() => {
		inferenceStore.checkHealth(); // immediate first check

		const intervalId = setInterval(async () => {
			const wasHealthy = inferenceStore.engineHealthy;
			const isHealthy = await inferenceStore.checkHealth();

			if (!wasHealthy && isHealthy) {
				await inferenceStore.syncStatus();
			}
		}, 10_000);

		return () => clearInterval(intervalId);
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
	<div
		class={`sidebar-stage ${uiStore.isSidebarOpen ? 'sidebar-stage--open' : 'sidebar-stage--closed'}`}
	>
		<Sidebar onClose={() => uiStore.setSidebarOpen(false)} />
	</div>

	<div class="workspace-shell">
		<WorkspaceHeader
			title={pageTitle}
			showSidebarToggle={!uiStore.isSidebarOpen}
			status={inferenceStore.status}
			modelInfoActive={showModelInfoPanel}
			hardwareActive={showHardwarePanel}
			shortcutsOpen={showShortcutsOverlay}
			canExport={messages.length > 0}
			onOpenSidebar={() => uiStore.setSidebarOpen(true)}
			onToggleModelInfo={() => uiStore.toggleOverlay('modelInfo')}
			onToggleHardware={() => uiStore.toggleOverlay('hardware')}
			onToggleShortcuts={() => (showShortcutsOverlay = !showShortcutsOverlay)}
			onExportChat={handleExportChat}
		/>

		<WorkspaceControls>
			{#snippet modeSelector()}
				<AppModeDropdown
					modes={activeModeConfigs}
					{activeMode}
					onChange={handleModeChange}
				/>
			{/snippet}
		</WorkspaceControls>

		{#if !inferenceStore.engineHealthy}
			<div
				class="mx-4 mt-2 rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-2 text-sm text-amber-200"
			>
				Engine disconnected — attempting to reconnect...
			</div>
		{/if}

		{#if setupNeedsAttention}
			<SetupBanner
				status={setupStatus}
				error={setupError}
				onOpen={() => (showSetupPanel = true)}
			/>
		{/if}

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
			{hasActiveStream}
			{isCancelling}
			{bottomOffset}
			onSend={handleSendMessage}
			onCancel={handleCancelGeneration}
		/>
	</div>

	<BenchmarkPanel visible={showBenchmarkPanel} onClose={() => uiStore.closeOverlay()} />
	<HardwarePanel visible={showHardwarePanel} onClose={() => uiStore.closeOverlay()} />
	<ModelInfoPanel
		visible={showModelInfoPanel}
		busy={isInferenceBusy}
		onClose={() => uiStore.closeOverlay()}
	/>
	<KeyboardShortcutsOverlay
		open={showShortcutsOverlay}
		onClose={() => (showShortcutsOverlay = false)}
	/>
	<SetupPanel
		visible={showSetupPanel}
		status={setupStatus}
		error={setupError}
		loading={setupStore.loading}
		preparing={setupStore.preparing}
		onRefresh={() => setupStore.refresh()}
		onPrepare={() => setupStore.prepare()}
		onClose={() => (showSetupPanel = false)}
	/>
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
		background: linear-gradient(
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
</style>
