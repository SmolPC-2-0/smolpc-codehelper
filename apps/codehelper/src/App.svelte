<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { assistantCancel, assistantSend, undoModeAction } from '$lib/api/unified';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import BenchmarkPanel from '$lib/components/BenchmarkPanel.svelte';
	import HardwarePanel from '$lib/components/HardwarePanel.svelte';
	import ModelInfoPanel from '$lib/components/ModelInfoPanel.svelte';
	import KeyboardShortcutsOverlay from '$lib/components/KeyboardShortcutsOverlay.svelte';
	import WorkspaceHeader from '$lib/components/layout/WorkspaceHeader.svelte';
	import WorkspaceControls from '$lib/components/layout/WorkspaceControls.svelte';
	import ConversationView from '$lib/components/chat/ConversationView.svelte';
	import ComposerBar from '$lib/components/chat/ComposerBar.svelte';
	import { chatsStore } from '$lib/stores/chats.svelte';
	import { modeStore } from '$lib/stores/mode.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { inferenceStore } from '$lib/stores/inference.svelte';
	import { hardwareStore } from '$lib/stores/hardware.svelte';
	import { uiStore } from '$lib/stores/ui.svelte';
	import { applyTheme, watchSystemTheme } from '$lib/utils/theme';
	import type { Message } from '$lib/types/chat';
	import type {
		AssistantMessageDto,
		AssistantResponseDto,
		AssistantStreamEvent
	} from '$lib/types/assistant';
	import type {
		GenerationConfig,
		InferenceBackend,
		InferenceChatMessage,
		InferenceStatus
	} from '$lib/types/inference';
	import type { AppMode } from '$lib/types/mode';

	let messagesContainer: HTMLDivElement | undefined = $state();
	let cancelRequested = $state(false);
	let currentStreamingChatId = $state<string | null>(null);
	let currentStreamingMessageId = $state<string | null>(null);
	let currentUnifiedChatId = $state<string | null>(null);
	let currentUnifiedMessageId = $state<string | null>(null);
	let currentUnifiedMode = $state<AppMode | null>(null);
	let bottomOffset = $state(0);
	let showShortcutsOverlay = $state(false);
	const NON_CODE_DISABLED_REASON =
		'This mode is visible in the unified shell, but chat execution is not wired yet.';
	const LIBREOFFICE_DISABLED_REASON =
		'LibreOffice integration is scaffolded in the unified app, but live document actions are not wired yet.';
	const GIMP_COMPOSER_PLACEHOLDER =
		'Describe the change you want to make to the image (Shift+Enter for new line)...';
	const BLENDER_COMPOSER_PLACEHOLDER =
		'Ask about your Blender scene or a Blender workflow (Shift+Enter for new line)...';
	const WRITER_COMPOSER_PLACEHOLDER =
		'Ask LibreOffice Writer to create or edit a document (Shift+Enter for new line)...';
	const CALC_COMPOSER_PLACEHOLDER =
		'LibreOffice Calc is scaffolded here, but live spreadsheet chat is not active yet.';
	const SLIDES_COMPOSER_PLACEHOLDER =
		'Ask LibreOffice Slides to create or edit a presentation (Shift+Enter for new line)...';

	const activeMode = $derived(modeStore.activeMode);
	const activeModeConfig = $derived(modeStore.activeConfig);
	const activeModeStatus = $derived(modeStore.activeStatus);
	const inferenceStatus = $derived(inferenceStore.status);
	const currentChat = $derived(chatsStore.getCurrentChatForMode(activeMode));
	const messages = $derived(currentChat?.messages ?? []);
	const hasNoChats = $derived(chatsStore.chats.length === 0);
	const canUseCodePath = $derived(activeMode === 'code');
	const canUseGimpPath = $derived(activeMode === 'gimp');
	const canUseBlenderPath = $derived(activeMode === 'blender');
	const canUseWriterPath = $derived(activeMode === 'writer');
	const canUseImpressPath = $derived(activeMode === 'impress');
	const isUnifiedRequestRunning = $derived(
		currentUnifiedMode !== null && currentUnifiedChatId !== null && currentUnifiedMessageId !== null
	);
	const isGimpRequestRunning = $derived(currentUnifiedMode === 'gimp' && isUnifiedRequestRunning);
	const isBlenderRequestRunning = $derived(
		currentUnifiedMode === 'blender' && isUnifiedRequestRunning
	);
	const isWriterRequestRunning = $derived(
		currentUnifiedMode === 'writer' && isUnifiedRequestRunning
	);
	const isImpressRequestRunning = $derived(
		currentUnifiedMode === 'impress' && isUnifiedRequestRunning
	);
	const currentUnifiedModeLabel = $derived(
		currentUnifiedMode ? (modeStore.getConfig(currentUnifiedMode)?.label ?? 'Another mode') : null
	);
	const hasLiveComposer = $derived(
		canUseCodePath || canUseGimpPath || canUseBlenderPath || canUseWriterPath || canUseImpressPath
	);
	const modeLabel = $derived(activeModeConfig?.label ?? 'Mode');
	const modeSubtitle = $derived(activeModeConfig?.subtitle ?? 'Unified assistant workspace');
	const modeSuggestions = $derived(activeModeConfig?.suggestions ?? []);
	const showContextControls = $derived(activeModeConfig?.capabilities.showContextControls ?? false);
	const canExport = $derived(
		Boolean(activeModeConfig?.capabilities.showExport) && messages.length > 0
	);
	function isLibreOfficeMode(mode: AppMode): boolean {
		return mode === 'writer' || mode === 'calc' || mode === 'impress';
	}

	function composerPlaceholderForMode(mode: AppMode): string {
		switch (mode) {
			case 'gimp':
				return GIMP_COMPOSER_PLACEHOLDER;
			case 'blender':
				return BLENDER_COMPOSER_PLACEHOLDER;
			case 'writer':
				return WRITER_COMPOSER_PLACEHOLDER;
			case 'calc':
				return CALC_COMPOSER_PLACEHOLDER;
			case 'impress':
				return SLIDES_COMPOSER_PLACEHOLDER;
			case 'code':
			default:
				return 'Ask a coding question (Shift+Enter for new line)...';
		}
	}

	function defaultChatTitleForMode(mode: AppMode): string {
		switch (mode) {
			case 'code':
				return 'New Code Chat';
			case 'gimp':
				return 'New GIMP Chat';
			case 'blender':
				return 'New Blender Chat';
			case 'writer':
				return 'New Writer Chat';
			case 'calc':
				return 'New Calc Chat';
			case 'impress':
				return 'New Slides Chat';
			default:
				return 'New Chat';
		}
	}

	const composerDisabledReason = $derived.by(() => {
		if (!hasLiveComposer) {
			return isLibreOfficeMode(activeMode) ? LIBREOFFICE_DISABLED_REASON : NON_CODE_DISABLED_REASON;
		}

		if (canUseCodePath && isUnifiedRequestRunning) {
			return `${currentUnifiedModeLabel ?? 'Another mode'} is still processing a request. Switch back to that mode to wait or cancel it.`;
		}

		if (
			(canUseGimpPath || canUseBlenderPath || canUseWriterPath || canUseImpressPath) &&
			inferenceStore.isGenerating
		) {
			return `Code mode is still generating a response. Wait for it to finish before starting a ${modeLabel} request.`;
		}

		if (
			(canUseGimpPath || canUseBlenderPath || canUseWriterPath || canUseImpressPath) &&
			isUnifiedRequestRunning &&
			currentUnifiedMode !== activeMode
		) {
			return `${currentUnifiedModeLabel ?? 'Another mode'} is still processing a request. Switch back to that mode to wait or cancel it.`;
		}

		return null;
	});
	const composerIsLoaded = $derived(canUseCodePath ? inferenceStore.isLoaded : hasLiveComposer);
	const composerIsGenerating = $derived(
		canUseCodePath
			? inferenceStore.isGenerating
			: canUseGimpPath
				? isGimpRequestRunning
				: canUseBlenderPath
					? isBlenderRequestRunning
					: canUseWriterPath
						? isWriterRequestRunning
						: canUseImpressPath
							? isImpressRequestRunning
							: false
	);
	const composerPlaceholder = $derived(composerPlaceholderForMode(activeMode));
	const pageTitle = $derived(currentChat?.title ?? defaultChatTitleForMode(activeMode));
	const showBenchmarkPanel = $derived(uiStore.activeOverlay === 'benchmark');
	const showHardwarePanel = $derived(uiStore.activeOverlay === 'hardware');
	const showModelInfoPanel = $derived(uiStore.activeOverlay === 'modelInfo');
	const showScrollToLatest = $derived(uiStore.userHasScrolledUp && messages.length > 0);
	const latestAssistantMessageId = $derived(
		[...messages].reverse().find((message) => message.role === 'assistant')?.id ?? null
	);
	function formatBackendLabel(backend: InferenceBackend | null): string | null {
		if (!backend) {
			return null;
		}

		switch (backend) {
			case 'openvino_npu':
				return 'OpenVINO NPU';
			case 'directml':
				return 'DirectML';
			case 'cpu':
				return 'CPU';
			default:
				return backend;
		}
	}

	function buildCodeModeStatusLabel(status: InferenceStatus): string {
		if (status.isGenerating) {
			return 'generating';
		}

		switch (status.readinessState) {
			case 'ready':
				return formatBackendLabel(status.activeBackend)?.toLowerCase() ?? 'ready';
			case 'failed':
				return 'startup failed';
			case 'idle':
				return 'engine idle';
			case 'starting':
			case 'probing':
			case 'resolving_assets':
			case 'loading_model':
				return 'starting engine';
			default:
				return 'status pending';
		}
	}

	function buildCodeModeStatusDetail(
		status: InferenceStatus,
		shellWarning: string | null
	): string | null {
		const details: string[] = [];

		if (shellWarning) {
			details.push(`Shell warning: ${shellWarning}`);
		}

		if (status.readinessState === 'failed') {
			if (status.startupErrorMessage) {
				details.push(status.startupErrorMessage);
			} else if (status.startupErrorCode) {
				details.push(`Startup error: ${status.startupErrorCode}`);
			}
			return details.length > 0 ? details.join(' · ') : null;
		}

		if (status.currentModel) {
			details.push(`Model: ${status.currentModel}`);
		}

		const backendLabel = formatBackendLabel(status.activeBackend);
		if (backendLabel) {
			details.push(`Backend: ${backendLabel}`);
		}

		if (status.isGenerating) {
			details.push('Streaming response');
		}

		return details.length > 0 ? details.join(' · ') : null;
	}

	const modeStatusLabel = $derived(
		canUseCodePath
			? buildCodeModeStatusLabel(inferenceStatus)
			: activeModeStatus?.providerState
				? activeModeStatus.providerState.state.replace(/_/g, ' ')
				: modeStore.error
					? 'fallback active'
					: modeStore.loading
						? 'loading'
						: 'status pending'
	);
	const modeStatusDetail = $derived.by(() => {
		if (canUseCodePath) {
			return buildCodeModeStatusDetail(inferenceStatus, modeStore.error);
		}

		const details = [
			modeStore.error ? `Shell warning: ${modeStore.error}` : null,
			activeModeStatus?.providerState.detail ?? null
		].filter((detail): detail is string => Boolean(detail));

		return details.length > 0 ? details.join(' · ') : null;
	});

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

	function buildAssistantMessages(
		userMessage: string,
		historyMessages: Message[]
	): AssistantMessageDto[] {
		const payload: AssistantMessageDto[] = [];

		for (const message of historyMessages) {
			payload.push({
				role: message.role,
				content: message.content
			});
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

	async function handleModeChange(mode: AppMode) {
		await modeStore.setActiveMode(mode);
		uiStore.resetScrollState();
		uiStore.setShowQuickExamples(true);
	}

	async function handleSendMessage(content: string) {
		if (activeMode === 'gimp') {
			await handleGimpMessage(content);
			return;
		}

		if (activeMode === 'blender') {
			await handleBlenderMessage(content);
			return;
		}

		if (activeMode === 'writer') {
			await handleWriterMessage(content);
			return;
		}

		if (activeMode === 'impress') {
			await handleImpressMessage(content);
			return;
		}

		if (
			activeMode !== 'code' ||
			!inferenceStore.isLoaded ||
			inferenceStore.isGenerating ||
			isUnifiedRequestRunning
		) {
			return;
		}

		const activeChat =
			currentChat ??
			chatsStore.createChat(
				'code',
				inferenceStore.currentModel ?? settingsStore.selectedModel ?? 'onnx-model'
			);
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
			const messagesPayload = buildStructuredMessages(content, historyBeforeMessage);
			const isOpenVinoNpu = inferenceStore.status.activeBackend === 'openvino_npu';
			const config: Partial<GenerationConfig> = {
				// OpenVINO NPU runs are more stable with greedy decoding and a tighter default
				// token budget; users can still continue generation explicitly from the UI.
				max_length: isOpenVinoNpu ? 512 : 2048,
				temperature: isOpenVinoNpu ? 0 : settingsStore.temperature,
				top_k: 40,
				top_p: 0.85,
				repetition_penalty: 1.15,
				repetition_penalty_last_n: 128
			};

			await inferenceStore.generateStreamMessages(
				messagesPayload,
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

	function finalizeUnifiedResponse(
		mode: AppMode,
		chatId: string,
		messageId: string,
		response: AssistantResponseDto
	) {
		if (mode === 'gimp' && response.undoable) {
			chatsStore.clearUndoableAssistantMessages(chatId);
		}

		chatsStore.updateMessage(chatId, messageId, {
			content: response.reply,
			explain: response.explain ?? null,
			undoable: response.undoable,
			toolResults: response.toolResults.length > 0 ? response.toolResults : undefined,
			plan: response.plan,
			isStreaming: false
		});
	}

	function updateUnifiedStreamingMessage(
		mode: AppMode,
		chatId: string,
		messageId: string,
		updates: Partial<Message>
	) {
		chatsStore.updateMessage(chatId, messageId, updates);
		if (activeMode === mode && chatsStore.getCurrentChatIdForMode(mode) === chatId) {
			scrollToBottom();
		}
	}

	interface UnifiedStreamAccumulator {
		current: AssistantResponseDto | null;
		hasSeenToken: boolean;
		accumulatedText: string;
		toolResults: Message['toolResults'];
	}

	function applyGimpEvent(
		chatId: string,
		messageId: string,
		event: AssistantStreamEvent,
		streamedResponse: UnifiedStreamAccumulator
	) {
		switch (event.kind) {
			case 'status':
				updateUnifiedStreamingMessage('gimp', chatId, messageId, {
					content: event.detail,
					isStreaming: true
				});
				break;
			case 'tool_call':
				updateUnifiedStreamingMessage('gimp', chatId, messageId, {
					content: `Running ${event.name}...`,
					isStreaming: true
				});
				break;
			case 'tool_result': {
				const toolResults = [...(streamedResponse.toolResults ?? []), event.result];
				streamedResponse.toolResults = toolResults;
				updateUnifiedStreamingMessage('gimp', chatId, messageId, {
					content: event.result.ok ? event.result.summary : `Error: ${event.result.summary}`,
					toolResults,
					isStreaming: true
				});
				break;
			}
			case 'complete':
				streamedResponse.current = event.response;
				finalizeUnifiedResponse('gimp', chatId, messageId, {
					...event.response,
					toolResults: streamedResponse.toolResults ?? event.response.toolResults
				});
				break;
			case 'error':
				updateUnifiedStreamingMessage('gimp', chatId, messageId, {
					content: `Error: ${event.message}`,
					explain: null,
					undoable: false,
					toolResults: streamedResponse.toolResults,
					plan: undefined,
					isStreaming: false
				});
				break;
			case 'token':
				break;
		}
	}

	function applyBlenderEvent(
		chatId: string,
		messageId: string,
		event: AssistantStreamEvent,
		streamedResponse: UnifiedStreamAccumulator
	) {
		switch (event.kind) {
			case 'status':
				if (!streamedResponse.hasSeenToken) {
					updateUnifiedStreamingMessage('blender', chatId, messageId, {
						content: event.detail,
						toolResults: streamedResponse.toolResults,
						isStreaming: true
					});
				}
				break;
			case 'tool_call':
				if (!streamedResponse.hasSeenToken) {
					updateUnifiedStreamingMessage('blender', chatId, messageId, {
						content: `Running ${event.name}...`,
						toolResults: streamedResponse.toolResults,
						isStreaming: true
					});
				}
				break;
			case 'tool_result': {
				const toolResults = [...(streamedResponse.toolResults ?? []), event.result];
				streamedResponse.toolResults = toolResults;
				updateUnifiedStreamingMessage('blender', chatId, messageId, {
					content: streamedResponse.hasSeenToken
						? streamedResponse.accumulatedText
						: event.result.summary,
					toolResults,
					isStreaming: true
				});
				break;
			}
			case 'token':
				streamedResponse.hasSeenToken = true;
				streamedResponse.accumulatedText += event.token;
				updateUnifiedStreamingMessage('blender', chatId, messageId, {
					content: streamedResponse.accumulatedText,
					toolResults: streamedResponse.toolResults,
					isStreaming: true
				});
				break;
			case 'complete': {
				streamedResponse.current = event.response;
				const reply =
					streamedResponse.hasSeenToken && streamedResponse.accumulatedText.length > 0
						? streamedResponse.accumulatedText
						: event.response.reply;
				finalizeUnifiedResponse('blender', chatId, messageId, {
					...event.response,
					reply,
					toolResults: streamedResponse.toolResults ?? event.response.toolResults
				});
				break;
			}
			case 'error':
				updateUnifiedStreamingMessage('blender', chatId, messageId, {
					content: `Error: ${event.message}`,
					explain: null,
					undoable: false,
					toolResults: streamedResponse.toolResults,
					plan: undefined,
					isStreaming: false
				});
				break;
		}
	}

	function applyLibreOfficeEvent(
		mode: 'writer' | 'impress',
		chatId: string,
		messageId: string,
		event: AssistantStreamEvent,
		streamedResponse: UnifiedStreamAccumulator
	) {
		switch (event.kind) {
			case 'status':
				if (!streamedResponse.hasSeenToken) {
					updateUnifiedStreamingMessage(mode, chatId, messageId, {
						content: event.detail,
						toolResults: streamedResponse.toolResults,
						isStreaming: true
					});
				}
				break;
			case 'tool_call':
				if (!streamedResponse.hasSeenToken) {
					updateUnifiedStreamingMessage(mode, chatId, messageId, {
						content: `Running ${event.name}...`,
						toolResults: streamedResponse.toolResults,
						isStreaming: true
					});
				}
				break;
			case 'tool_result': {
				const toolResults = [...(streamedResponse.toolResults ?? []), event.result];
				streamedResponse.toolResults = toolResults;
				updateUnifiedStreamingMessage(mode, chatId, messageId, {
					content: streamedResponse.hasSeenToken
						? streamedResponse.accumulatedText
						: event.result.summary,
					toolResults,
					isStreaming: true
				});
				break;
			}
			case 'token':
				streamedResponse.hasSeenToken = true;
				streamedResponse.accumulatedText += event.token;
				updateUnifiedStreamingMessage(mode, chatId, messageId, {
					content: streamedResponse.accumulatedText,
					toolResults: streamedResponse.toolResults,
					isStreaming: true
				});
				break;
			case 'complete': {
				streamedResponse.current = event.response;
				finalizeUnifiedResponse(mode, chatId, messageId, {
					...event.response,
					reply:
						streamedResponse.hasSeenToken && streamedResponse.accumulatedText.length > 0
							? streamedResponse.accumulatedText
							: event.response.reply,
					toolResults: streamedResponse.toolResults ?? event.response.toolResults
				});
				break;
			}
			case 'error':
				updateUnifiedStreamingMessage(mode, chatId, messageId, {
					content: `Error: ${event.message}`,
					explain: null,
					undoable: false,
					toolResults: streamedResponse.toolResults,
					plan: undefined,
					isStreaming: false
				});
				break;
		}
	}

	async function handleUnifiedModeMessage(
		mode: 'gimp' | 'blender' | 'writer' | 'impress',
		content: string
	) {
		if (
			activeMode !== mode ||
			currentUnifiedMode !== null ||
			inferenceStore.isGenerating ||
			(mode === 'gimp'
				? !canUseGimpPath
				: mode === 'blender'
					? !canUseBlenderPath
					: mode === 'writer'
						? !canUseWriterPath
						: !canUseImpressPath)
		) {
			return;
		}

		const activeChat =
			currentChat ??
			chatsStore.createChat(
				mode,
				inferenceStore.currentModel ?? settingsStore.selectedModel ?? 'onnx-model'
			);
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
			content:
				mode === 'gimp'
					? 'Selecting the best GIMP action for this request.'
					: mode === 'blender'
						? 'Starting the Blender tutoring request.'
						: mode === 'writer'
							? 'Starting the Writer request.'
							: 'Starting the Slides request.',
			timestamp: Date.now(),
			isStreaming: true
		};
		chatsStore.addMessage(activeChat.id, assistantMessage);
		scrollToBottom();

		currentUnifiedChatId = activeChat.id;
		currentUnifiedMessageId = assistantMessage.id;
		currentUnifiedMode = mode;

		const chatId = activeChat.id;
		const messageId = assistantMessage.id;
		const streamedResponse: UnifiedStreamAccumulator = {
			current: null,
			hasSeenToken: false,
			accumulatedText: '',
			toolResults: undefined
		};

		try {
			await modeStore.refreshModeStatus(mode);
			const request = {
				mode,
				chatId,
				messages: buildAssistantMessages(content, historyBeforeMessage),
				userText: content
			};

			const response = await assistantSend(request, (event) => {
				if (mode === 'gimp') {
					applyGimpEvent(chatId, messageId, event, streamedResponse);
					return;
				}

				if (mode === 'blender') {
					applyBlenderEvent(chatId, messageId, event, streamedResponse);
					return;
				}

				applyLibreOfficeEvent(mode, chatId, messageId, event, streamedResponse);
			});

			if (!streamedResponse.current) {
				finalizeUnifiedResponse(mode, chatId, messageId, {
					...response,
					reply:
						(mode === 'blender' || mode === 'writer' || mode === 'impress') &&
						streamedResponse.hasSeenToken &&
						streamedResponse.accumulatedText.length > 0
							? streamedResponse.accumulatedText
							: response.reply,
					toolResults: streamedResponse.toolResults ?? response.toolResults
				});
			}
		} catch (error) {
			console.error(`${mode.toUpperCase()} request failed:`, error);
			chatsStore.updateMessage(chatId, messageId, {
				content: `Error: ${error}`,
				explain: null,
				undoable: false,
				toolResults: streamedResponse.toolResults,
				plan: undefined,
				isStreaming: false
			});
		} finally {
			currentUnifiedChatId = null;
			currentUnifiedMessageId = null;
			currentUnifiedMode = null;
			await modeStore.refreshModeStatus(mode);
		}
	}

	async function handleGimpMessage(content: string) {
		await handleUnifiedModeMessage('gimp', content);
	}

	async function handleBlenderMessage(content: string) {
		await handleUnifiedModeMessage('blender', content);
	}

	async function handleWriterMessage(content: string) {
		await handleUnifiedModeMessage('writer', content);
	}

	async function handleImpressMessage(content: string) {
		await handleUnifiedModeMessage('impress', content);
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
		if (
			(activeMode !== 'code' && activeMode !== 'blender') ||
			inferenceStore.isGenerating ||
			(activeMode === 'blender' && isBlenderRequestRunning)
		) {
			return;
		}
		const sourcePrompt = findNearestUserPrompt(messageId);
		if (!sourcePrompt) return;
		handleSendMessage(sourcePrompt);
	}

	function handleContinueMessage(messageId: string) {
		if (
			(activeMode !== 'code' && activeMode !== 'blender') ||
			inferenceStore.isGenerating ||
			(activeMode === 'blender' && isBlenderRequestRunning)
		) {
			return;
		}
		const basePrompt = findNearestUserPrompt(messageId);
		const continuationPrompt = basePrompt
			? `Continue your previous response to: "${basePrompt}". Expand with more details and an example.`
			: 'Continue your previous response with additional detail and examples.';
		handleSendMessage(continuationPrompt);
	}

	function handleBranchFromMessage(messageId: string) {
		if (
			(activeMode !== 'code' && activeMode !== 'blender') ||
			!currentChat ||
			(activeMode === 'blender' && isBlenderRequestRunning)
		) {
			return;
		}
		const messageIndex = currentChat.messages.findIndex((message) => message.id === messageId);
		if (messageIndex < 0) return;

		const branchSource = currentChat.messages.slice(0, messageIndex + 1);
		if (branchSource.length === 0) return;

		const targetModel = currentChat.model ?? inferenceStore.currentModel ?? 'onnx-model';
		const branchChat = chatsStore.createChat(currentChat.mode, targetModel);

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
		if (activeMode !== 'code' || !currentChat || currentChat.messages.length === 0) return;

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
		if (
			activeMode !== 'code' &&
			activeMode !== 'gimp' &&
			activeMode !== 'blender' &&
			activeMode !== 'writer' &&
			activeMode !== 'impress'
		)
			return;
		handleSendMessage(prompt);
	}

	async function handleCancelGeneration() {
		if (
			(activeMode === 'gimp' && isGimpRequestRunning) ||
			(activeMode === 'blender' && isBlenderRequestRunning) ||
			(activeMode === 'writer' && isWriterRequestRunning) ||
			(activeMode === 'impress' && isImpressRequestRunning)
		) {
			try {
				await assistantCancel();
				if (currentUnifiedChatId && currentUnifiedMessageId) {
					chatsStore.updateMessage(currentUnifiedChatId, currentUnifiedMessageId, {
						content: 'Cancelling request...'
					});
				}
			} catch (error) {
				console.error('Failed to cancel unified mode request:', error);
			}
			return;
		}

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

	async function handleUndoMessage(messageId: string) {
		if (activeMode !== 'gimp' || !currentChat || isGimpRequestRunning) {
			return;
		}

		const targetMessage = currentChat.messages.find((message) => message.id === messageId);
		if (!targetMessage?.undoable) {
			return;
		}

		try {
			await undoModeAction('gimp');
			chatsStore.clearUndoableAssistantMessages(currentChat.id);
			chatsStore.updateMessage(currentChat.id, messageId, {
				undoable: false
			});
			chatsStore.addMessage(currentChat.id, {
				id: crypto.randomUUID(),
				role: 'assistant',
				content: '↩ Last change undone.',
				timestamp: Date.now(),
				explain:
					'To do this yourself in GIMP: press Ctrl+Z (or Cmd+Z on macOS), or choose Edit → Undo.',
				undoable: false
			});
			await modeStore.refreshModeStatus('gimp');
		} catch (error) {
			console.error('Failed to undo GIMP action:', error);
			chatsStore.addMessage(currentChat.id, {
				id: crypto.randomUUID(),
				role: 'assistant',
				content: `Error: ${error}`,
				timestamp: Date.now(),
				undoable: false
			});
		}
	}

	function handleKeyDown(event: KeyboardEvent) {
		const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
		const modifierKey = isMac ? event.metaKey : event.ctrlKey;
		const typingInInput = isTypingTarget(event.target);

		if (modifierKey && event.shiftKey && event.key.toLowerCase() === 'b') {
			if (!(activeModeConfig?.capabilities.showBenchmarkPanel ?? false)) {
				return;
			}
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

		async function initApp() {
			await modeStore.initialize();
			await initInference();
			hardwareStore.getCached();
			await modeStore.refreshModeStatus();

			if (hasNoChats) {
				chatsStore.createChat(
					'code',
					inferenceStore.currentModel ?? settingsStore.selectedModel ?? 'onnx-model'
				);
			}
		}

		initApp().catch((error) => {
			console.error('[initApp]', error);
		});

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
		const canShowBenchmark = activeModeConfig?.capabilities.showBenchmarkPanel ?? false;
		// Centralize benchmark overlay cleanup here so mode changes and config refreshes
		// follow the same capability gate.
		if (!canShowBenchmark && uiStore.activeOverlay === 'benchmark') {
			uiStore.closeOverlay();
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
	<div
		class={`sidebar-stage ${uiStore.isSidebarOpen ? 'sidebar-stage--open' : 'sidebar-stage--closed'}`}
	>
		<Sidebar
			{activeMode}
			activeModeLabel={modeLabel}
			activeModeSubtitle={modeSubtitle}
			onClose={() => uiStore.setSidebarOpen(false)}
		/>
	</div>

	<div class="workspace-shell">
		<WorkspaceHeader
			title={pageTitle}
			{modeLabel}
			{modeSubtitle}
			{modeStatusLabel}
			{modeStatusDetail}
			modes={modeStore.modeConfigs}
			{activeMode}
			showSidebarToggle={!uiStore.isSidebarOpen}
			status={inferenceStatus}
			modelInfoActive={showModelInfoPanel}
			hardwareActive={showHardwarePanel}
			shortcutsOpen={showShortcutsOverlay}
			{canExport}
			onOpenSidebar={() => uiStore.setSidebarOpen(true)}
			onChangeMode={handleModeChange}
			onToggleModelInfo={() => uiStore.toggleOverlay('modelInfo')}
			onToggleHardware={() => uiStore.toggleOverlay('hardware')}
			onToggleShortcuts={() => (showShortcutsOverlay = !showShortcutsOverlay)}
			onExportChat={handleExportChat}
		/>

		<WorkspaceControls {showContextControls} />

		<ConversationView
			mode={activeMode}
			{modeLabel}
			{modeSubtitle}
			suggestions={modeSuggestions}
			providerState={activeModeStatus?.providerState ?? null}
			statusLabel={modeStatusLabel}
			statusDetail={modeStatusDetail}
			{messages}
			{latestAssistantMessageId}
			showQuickExamples={uiStore.showQuickExamples}
			disabledExamples={!hasLiveComposer || !!composerDisabledReason}
			disabledReason={composerDisabledReason}
			onSelectExample={handleExampleSelect}
			onToggleExamples={(show) => uiStore.setShowQuickExamples(show)}
			onUserScrollUp={markScrollIntentUp}
			onScroll={handleScroll}
			{showScrollToLatest}
			onScrollToLatest={handleScrollToLatest}
			onRegenerateMessage={handleRegenerateMessage}
			onContinueMessage={handleContinueMessage}
			onBranchFromMessage={handleBranchFromMessage}
			onUndoMessage={handleUndoMessage}
			onContainerReady={setMessagesContainer}
		/>

		<ComposerBar
			isLoaded={composerIsLoaded}
			isGenerating={composerIsGenerating}
			disabledReason={composerDisabledReason}
			placeholder={composerPlaceholder}
			{bottomOffset}
			onSend={handleSendMessage}
			onCancel={handleCancelGeneration}
		/>
	</div>

	<BenchmarkPanel visible={showBenchmarkPanel} onClose={() => uiStore.closeOverlay()} />
	<HardwarePanel visible={showHardwarePanel} onClose={() => uiStore.closeOverlay()} />
	<ModelInfoPanel visible={showModelInfoPanel} onClose={() => uiStore.closeOverlay()} />
	<KeyboardShortcutsOverlay
		open={showShortcutsOverlay}
		onClose={() => (showShortcutsOverlay = false)}
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
