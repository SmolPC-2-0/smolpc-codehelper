<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import BenchmarkPanel from '$lib/components/BenchmarkPanel.svelte';
	import HardwarePanel from '$lib/components/HardwarePanel.svelte';
	import ModelInfoPanel from '$lib/components/ModelInfoPanel.svelte';
	import KeyboardShortcutsOverlay from '$lib/components/KeyboardShortcutsOverlay.svelte';
	import ModeHelpDrawer from '$lib/components/ModeHelpDrawer.svelte';
	import WorkspaceHeader from '$lib/components/layout/WorkspaceHeader.svelte';
	import WorkspaceControls from '$lib/components/layout/WorkspaceControls.svelte';
	import AppModeDropdown from '$lib/components/layout/AppModeDropdown.svelte';
	import ConversationView from '$lib/components/chat/ConversationView.svelte';
	import ComposerBar from '$lib/components/chat/ComposerBar.svelte';
	import SetupBanner from '$lib/components/setup/SetupBanner.svelte';
	import SetupPanel from '$lib/components/setup/SetupPanel.svelte';
	import StartupLoadingScreen from '$lib/components/StartupLoadingScreen.svelte';
	import { chatsStore } from '$lib/stores/chats.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { inferenceStore } from '$lib/stores/inference.svelte';
	import { hardwareStore } from '$lib/stores/hardware.svelte';
	import { uiStore } from '$lib/stores/ui.svelte';
	import { modeStore } from '$lib/stores/mode.svelte';
	import { setupStore } from '$lib/stores/setup.svelte';
	import { assistantSend, assistantCancel, openModeHostApp } from '$lib/api/unified';
	import { applyTheme, watchSystemTheme } from '$lib/utils/theme';
	import type { Message } from '$lib/types/chat';
	import type {
		GenerationConfig,
		InferenceChatMessage,
		MemoryPressureStatus
	} from '$lib/types/inference';
	import type { AppMode } from '$lib/types/mode';
	import type { AssistantStreamEvent } from '$lib/types/assistant';

	let messagesContainer: HTMLDivElement | undefined = $state();
	let activeStreamSessionId = $state<string | null>(null);
	let currentStreamingChatId = $state<string | null>(null);
	let currentStreamingMessageId = $state<string | null>(null);
	let bottomOffset = $state(0);
	let showShortcutsOverlay = $state(false);
	let showHelpDrawer = $state(false);
	let showSetupPanel = $state(false);
	let startupComplete = $state(false);
	let isSwitchingMode = $state(false);
	let launchingHostApp = $state(false);
	let reconnectingEngineSession = $state(false);
	let memoryPressureNotice = $state<string | null>(null);
	let dismissedMemoryPressureKey = $state<string | null>(null);
	let hostAppLaunchError = $state<string | null>(null);

	// Unified mode state
	const activeMode = $derived(modeStore.activeMode);
	const activeModeConfigs = $derived(modeStore.modeConfigs);
	const activeModeConfig = $derived(modeStore.activeConfig);
	const activeModeLabel = $derived(activeModeConfig?.label ?? 'Mode');
	const modeSuggestions = $derived(activeModeConfig?.suggestions ?? []);
	const setupNeedsAttention = $derived(setupStore.initialized && setupStore.needsAttention);
	const setupStatus = $derived(setupStore.status);
	const setupError = $derived(setupStore.error);

	// Availability gating: map setup item detection → mode availability.
	// Static mapping — modes not listed here default to available.
	const SETUP_ID_TO_MODES: Readonly<Record<string, string[]>> = {
		host_gimp: ['gimp'],
		host_blender: ['blender'],
		host_libreoffice: ['writer', 'calc', 'impress']
	};
	const HOST_LAUNCH_LABELS: Partial<Record<AppMode, string>> = {
		gimp: 'Open GIMP',
		blender: 'Open Blender',
		writer: 'Open LibreOffice',
		calc: 'Open LibreOffice',
		impress: 'Open LibreOffice'
	};

	const modeAvailability = $derived.by(() => {
		const result: Record<string, boolean> = { code: true }; // Code always available
		for (const item of setupStore.items) {
			const modes = SETUP_ID_TO_MODES[item.id];
			if (modes) {
				const available = item.state === 'ready';
				for (const mode of modes) {
					result[mode] = available;
				}
			}
		}
		for (const mode of activeModeConfigs) {
			if (!(mode.id in result)) result[mode.id] = true;
		}
		return result;
	});

	const unavailableReasons = $derived.by(() => {
		const result: Record<string, string> = {};
		for (const item of setupStore.items) {
			const modes = SETUP_ID_TO_MODES[item.id];
			if (modes && item.state !== 'ready') {
				const reason =
					item.state === 'missing'
						? `Install ${item.label} to enable`
						: (item.detail ?? `${item.label} not ready`);
				for (const mode of modes) {
					result[mode] = reason;
				}
			}
		}
		return result;
	});
	const activeHostLaunchLabel = $derived(HOST_LAUNCH_LABELS[activeMode] ?? null);
	const canOpenHostApp = $derived(
		activeHostLaunchLabel !== null && (modeAvailability[activeMode] ?? false)
	);

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
	const composerDraftKey = $derived(`${activeMode}:${currentChat?.id ?? 'new'}`);
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

	function errorMessage(cause: unknown): string {
		return cause instanceof Error ? cause.message : String(cause);
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

	function resolvePreferredModelId(): string | null {
		const available = inferenceStore.availableModels;
		if (available.length === 0) {
			return null;
		}

		const selected = settingsStore.selectedModel?.trim();
		if (selected && available.some((model) => model.id === selected)) {
			return selected;
		}

		return available[0]?.id ?? null;
	}

	async function reestablishEngineSession() {
		const preferredModelId = resolvePreferredModelId();
		const preferredRuntimeMode = settingsStore.runtimeModePreference;

		// Pass the runtime mode preference in the startup request so the engine
		// starts on the preferred backend in a single startup. Previously this
		// was applied AFTER startup via setRuntimeMode which force-restarted
		// the engine — causing double startup (#173) and stuck UI on failure (#196).
		await inferenceStore.ensureStarted({
			mode: 'auto',
			startup_policy: preferredModelId ? { default_model_id: preferredModelId } : null,
			runtime_mode_preference: preferredRuntimeMode !== 'auto' ? preferredRuntimeMode : null
		});

		await inferenceStore.syncStatus();
		if (inferenceStore.currentModel) {
			settingsStore.setModel(inferenceStore.currentModel);
		}
	}

	function memoryPressureNoticeKey(snapshot: MemoryPressureStatus): string {
		return `${snapshot.level}:${snapshot.auto_unloaded ? '1' : '0'}:${snapshot.recommended_model_id ?? ''}`;
	}

	async function pollMemoryPressure() {
		let appMinimized = false;
		try {
			appMinimized = await getCurrentWindow().isMinimized();
		} catch {
			appMinimized = false;
		}

		const snapshot = await inferenceStore.evaluateMemoryPressure({
			activeMode: modeStore.activeMode,
			appMinimized
		});
		if (!snapshot) {
			return;
		}

		if (snapshot.auto_unloaded) {
			await inferenceStore.syncStatus();
		}

		const notice = snapshot.message;
		if (!notice) {
			memoryPressureNotice = null;
			dismissedMemoryPressureKey = null;
			return;
		}

		const noticeKey = memoryPressureNoticeKey(snapshot);
		if (dismissedMemoryPressureKey === noticeKey) {
			memoryPressureNotice = null;
			return;
		}

		memoryPressureNotice = notice;
	}

	function dismissMemoryPressureNotice() {
		const snapshot = inferenceStore.memoryPressure;
		const notice = memoryPressureNotice;
		if (snapshot && notice) {
			dismissedMemoryPressureKey = memoryPressureNoticeKey(snapshot);
		}
		memoryPressureNotice = null;
	}

	function finalizeActiveStreamingMessage(fallbackContent: string) {
		if (!currentStreamingChatId || !currentStreamingMessageId) {
			activeStreamSessionId = null;
			return;
		}

		const chat = chatsStore.chats.find((entry) => entry.id === currentStreamingChatId);
		const message = chat?.messages.find((entry) => entry.id === currentStreamingMessageId);
		const content = message?.content?.trim() ? message.content : fallbackContent;

		chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
			content,
			isStreaming: false
		});

		currentStreamingChatId = null;
		currentStreamingMessageId = null;
		activeStreamSessionId = null;
	}

	async function handleSendMessage(content: string) {
		if (activeMode === 'code' && (!inferenceStore.isLoaded || isInferenceBusy)) return;
		if (activeMode !== 'code' && isInferenceBusy) return;

		const chatLabel =
			activeMode === 'code' ? (inferenceStore.currentModel ?? 'onnx-model') : activeMode;
		const activeChat = currentChat ?? chatsStore.createChat(chatLabel, activeMode);
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

		let handledByEventStream = false;

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
							handledByEventStream = true;
							chatsStore.updateMessage(chatId, messageId, {
								content: `Error: ${event.message}`
							});
							break;
						case 'complete': {
							handledByEventStream = true;
							const activeMessage = chatsStore.chats
								.find((chat) => chat.id === chatId)
								?.messages.find((message) => message.id === messageId);
							const reply = event.response.reply.trim();
							chatsStore.updateMessage(chatId, messageId, {
								content: reply || activeMessage?.content || 'Done.'
							});
							// TODO: wire up event.response.undoable for undo support (#132)
							break;
						}
					}
				});
			}
		} catch (error) {
			if (!handledByEventStream) {
				console.error('Generation error:', error);
				chatsStore.updateMessage(chatId, messageId, {
					content: `Error: ${error}`,
					isStreaming: false
				});
			}
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
		if (!modeAvailability[mode]) return;

		isSwitchingMode = true;
		try {
			// Cancel any in-flight generation before switching modes
			if (hasActiveStream || inferenceStore.isGenerating) {
				await handleCancelGeneration();
			}
			// Don't await setActiveMode — the status refresh can hang if a
			// provider is slow to connect. The mode switch itself is synchronous
			// (activeMode + storage); status refresh is fire-and-forget.
			modeStore.setActiveMode(mode);
			chatsStore.setMode(mode);
			await pollMemoryPressure();
		} finally {
			isSwitchingMode = false;
		}
	}

	async function handleOpenHostApp() {
		if (!activeHostLaunchLabel || launchingHostApp || !canOpenHostApp) return;

		const mode = activeMode;
		let launchFailed = false;
		hostAppLaunchError = null;
		launchingHostApp = true;
		try {
			await openModeHostApp(mode);
		} catch (error) {
			launchFailed = true;
			hostAppLaunchError = errorMessage(error);
		} finally {
			launchingHostApp = false;
		}

		if (launchFailed) return;

		// Status refresh can hang for slow providers; keep the button responsive.
		void modeStore.refreshModeStatus(mode).catch((error) => {
			console.warn(`Failed to refresh mode status after opening ${mode}:`, error);
		});
	}

	function closeHelpDrawer() {
		showHelpDrawer = false;
	}

	function closeCompetingOverlaysForHelp() {
		showShortcutsOverlay = false;
		showSetupPanel = false;
		if (uiStore.activeOverlay !== 'none') {
			uiStore.closeOverlay();
		}
	}

	function handleToggleHelpDrawer() {
		if (showHelpDrawer) {
			closeHelpDrawer();
			return;
		}
		closeCompetingOverlaysForHelp();
		showHelpDrawer = true;
	}

	function handleToggleShortcutsOverlay() {
		if (!showShortcutsOverlay && showHelpDrawer) {
			closeHelpDrawer();
		}
		showShortcutsOverlay = !showShortcutsOverlay;
	}

	function handleOpenShortcutsOverlay() {
		if (showHelpDrawer) {
			closeHelpDrawer();
		}
		showShortcutsOverlay = true;
	}

	function handleToggleHeaderOverlay(overlay: 'benchmark' | 'hardware' | 'modelInfo') {
		if (showHelpDrawer) {
			closeHelpDrawer();
		}
		uiStore.toggleOverlay(overlay);
	}

	function openSetupPanel() {
		if (showHelpDrawer) {
			closeHelpDrawer();
		}
		showSetupPanel = true;
	}

	function handleKeyDown(event: KeyboardEvent) {
		const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
		const modifierKey = isMac ? event.metaKey : event.ctrlKey;
		const typingInInput = isTypingTarget(event.target);

		if (modifierKey && event.shiftKey && event.key.toLowerCase() === 'b') {
			event.preventDefault();
			handleToggleHeaderOverlay('benchmark');
			return;
		}

		if (modifierKey && event.key === '\\') {
			event.preventDefault();
			uiStore.toggleSidebar();
			return;
		}

		if (modifierKey && event.key === '/') {
			event.preventDefault();
			handleToggleShortcutsOverlay();
			return;
		}

		if (!typingInInput && event.key === '?') {
			event.preventDefault();
			handleOpenShortcutsOverlay();
			return;
		}

		if (event.key === 'Escape') {
			if (showHelpDrawer) {
				closeHelpDrawer();
				event.preventDefault();
				return;
			}

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
			try {
				await inferenceStore.listModels();
				await reestablishEngineSession();
				await pollMemoryPressure();
			} catch (error) {
				console.error('Failed to initialize inference session:', error);
			}
		}

		initInference();
		modeStore.initialize();
		setupStore.initialize();
		hardwareStore.getCached();
		chatsStore.finalizeStaleStreamingMessages();
		chatsStore.setMode(modeStore.activeMode);

		// Bootstrap a Code chat for brand-new users (zero chats across all modes).
		// When switching to a mode with no chats, handleSendMessage lazy-creates one.
		if (hasNoChats) {
			chatsStore.createChat(inferenceStore.currentModel ?? 'onnx-model', 'code');
		}

		window.addEventListener('keydown', handleKeyDown);

		return () => {
			window.removeEventListener('keydown', handleKeyDown);
			window.removeEventListener('resize', handleResize);
			window.visualViewport?.removeEventListener('resize', handleResize);
		};
	});

	$effect(() => {
		chatsStore.setMode(activeMode);
	});

	$effect(() => {
		const mode = activeMode;
		if (mode) {
			hostAppLaunchError = null;
		}
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

		finalizeActiveStreamingMessage('Generation interrupted after cancel timeout.');
	});

	// When engine dies, immediately clear all generation state so the UI
	// is never stuck. This fires before any reconnection attempt.
	$effect(() => {
		if (inferenceStore.engineHealthy) {
			return;
		}

		// Engine is down — nothing can be generating. Clear stale state immediately.
		inferenceStore.forceResetGenerationState();

		if (currentStreamingChatId && currentStreamingMessageId) {
			finalizeActiveStreamingMessage('Generation interrupted — engine disconnected.');
		}
	});

	// Startup loading screen — dismiss when engine is ready
	$effect(() => {
		if (inferenceStore.isReady && !startupComplete) {
			setTimeout(() => {
				startupComplete = true;
			}, 800);
		}
	});

	// Engine health polling — immediate check + 10s interval
	$effect(() => {
		inferenceStore.checkHealth(); // immediate first check
		void pollMemoryPressure();

		const intervalId = setInterval(async () => {
			const wasHealthy = inferenceStore.engineHealthy;
			const isHealthy = await inferenceStore.checkHealth();
			await pollMemoryPressure();

			if (!wasHealthy && isHealthy && !reconnectingEngineSession) {
				reconnectingEngineSession = true;
				try {
					await reestablishEngineSession();
					await pollMemoryPressure();
				} catch (error) {
					console.error('Failed to restore engine session after reconnect:', error);
				} finally {
					reconnectingEngineSession = false;
				}
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

<StartupLoadingScreen
	readiness={inferenceStore.readiness}
	onRetry={reestablishEngineSession}
	visible={!startupComplete}
/>

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
			onToggleModelInfo={() => handleToggleHeaderOverlay('modelInfo')}
			onToggleHardware={() => handleToggleHeaderOverlay('hardware')}
			onToggleShortcuts={handleToggleShortcutsOverlay}
			onExportChat={handleExportChat}
		>
			{#snippet modeSwitcher()}
				<AppModeDropdown
					modes={activeModeConfigs}
					{activeMode}
					onChange={handleModeChange}
					disabled={isSwitchingMode}
					{modeAvailability}
					{unavailableReasons}
				/>
			{/snippet}
		</WorkspaceHeader>

		<WorkspaceControls helpOpen={showHelpDrawer} onToggleHelp={handleToggleHelpDrawer}>
			{#snippet leadingContent()}
				{#if activeHostLaunchLabel}
					<button
						type="button"
						class="mode-launch-button"
						onclick={handleOpenHostApp}
						disabled={!canOpenHostApp || launchingHostApp}
						title={canOpenHostApp
							? activeHostLaunchLabel
							: (unavailableReasons[activeMode] ?? `${activeHostLaunchLabel} unavailable`)}
					>
						{launchingHostApp ? 'Opening...' : activeHostLaunchLabel}
					</button>
				{/if}
			{/snippet}
		</WorkspaceControls>

		{#if !inferenceStore.engineHealthy || reconnectingEngineSession}
			<div
				class="mx-4 mt-2 rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-2 text-sm text-amber-200"
			>
				Engine disconnected — restoring session state...
			</div>
		{/if}

		{#if memoryPressureNotice}
			<div
				class={`mx-4 mt-2 rounded-lg border px-4 py-2 text-sm ${
					inferenceStore.memoryPressure?.level === 'critical'
						? 'border-rose-500/35 bg-rose-500/10 text-rose-200'
						: 'border-amber-500/30 bg-amber-500/10 text-amber-200'
				}`}
			>
				<div class="flex items-start justify-between gap-3">
					<span>{memoryPressureNotice}</span>
					<button
						type="button"
						class="shrink-0 rounded border border-current/40 px-2 py-0.5 text-xs opacity-80 hover:opacity-100"
						onclick={dismissMemoryPressureNotice}
						aria-label="Dismiss memory warning"
					>
						Dismiss
					</button>
				</div>
			</div>
		{/if}

		{#if hostAppLaunchError}
			<div
				class="mx-4 mt-2 rounded-lg border border-rose-500/35 bg-rose-500/10 px-4 py-2 text-sm text-rose-200"
			>
				{hostAppLaunchError}
			</div>
		{/if}

		{#if setupNeedsAttention}
			<SetupBanner status={setupStatus} error={setupError} onOpen={openSetupPanel} />
		{/if}

		<ConversationView
			mode={activeMode}
			suggestions={modeSuggestions}
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
			draftKey={composerDraftKey}
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
	<ModeHelpDrawer
		open={showHelpDrawer}
		mode={activeMode}
		modeLabel={activeModeLabel}
		onClose={closeHelpDrawer}
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

	.mode-selector-group {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
	}

	.mode-launch-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 0.45rem 0.72rem;
		border-radius: var(--radius-xl);
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
		color: var(--color-foreground);
		font-size: 0.78rem;
		line-height: 1.25;
		cursor: pointer;
		white-space: nowrap;
	}

	.mode-launch-button:hover:enabled {
		background: color-mix(in srgb, var(--surface-widget) 82%, var(--color-foreground) 18%);
	}

	.mode-launch-button:disabled {
		opacity: 0.55;
		cursor: not-allowed;
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

		.mode-selector-group {
			display: flex;
			width: 100%;
		}

		.mode-launch-button {
			flex: 0 0 auto;
		}
	}
</style>
