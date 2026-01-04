<script lang="ts">
	import { onMount } from 'svelte';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import ChatMessage from '$lib/components/ChatMessage.svelte';
	import ChatInput from '$lib/components/ChatInput.svelte';
	import StatusIndicator from '$lib/components/StatusIndicator.svelte';
	import HardwareIndicator from '$lib/components/HardwareIndicator.svelte';
	import ModelSelector from '$lib/components/ModelSelector.svelte';
	import ContextToggle from '$lib/components/ContextToggle.svelte';
	import QuickExamples from '$lib/components/QuickExamples.svelte';
	import BenchmarkPanel from '$lib/components/BenchmarkPanel.svelte';
	import HardwarePanel from '$lib/components/HardwarePanel.svelte';
	import { chatsStore } from '$lib/stores/chats.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { inferenceStore } from '$lib/stores/inference.svelte';
	import { hardwareStore } from '$lib/stores/hardware.svelte';
	import type { Message } from '$lib/types/chat';
	import type { GenerationConfig } from '$lib/types/inference';
	import { Menu, X } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';

	// UI State
	let isSidebarOpen = $state(true);
	let isGenerating = $state(false);
	let showQuickExamples = $state(true);
	let messagesContainer: HTMLDivElement;
	let inputAreaRef: HTMLDivElement;
	let userHasScrolledUp = $state(false);
	let cancelRequested = $state(false);
	let currentStreamingChatId = $state<string | null>(null);
	let currentStreamingMessageId = $state<string | null>(null);
	let userInteractedWithScroll = $state(false);
	let touchStartY = $state(0);
	let showBenchmarkPanel = $state(false);
	let showHardwarePanel = $state(false);
	let bottomOffset = $state(0);

	// Derived state
	const currentChat = $derived(chatsStore.currentChat);
	const messages = $derived(currentChat?.messages ?? []);
	const hasNoChats = $derived(chatsStore.chats.length === 0);

	// Check if user is at bottom of scroll
	function isAtBottom(): boolean {
		if (!messagesContainer) return true;
		const threshold = 5; // Very small threshold - basically at the bottom
		const distanceFromBottom =
			messagesContainer.scrollHeight - messagesContainer.scrollTop - messagesContainer.clientHeight;
		return distanceFromBottom <= threshold;
	}

	// Detect when user scrolls UP (instant detection)
	function handleUserScrollIntent(event: WheelEvent) {
		// Only break autoscroll if user is scrolling UP (deltaY < 0)
		// Scrolling down should not break autoscroll
		if (event.deltaY < 0) {
			userInteractedWithScroll = true;
			userHasScrolledUp = true;
		}
	}

	// Handle touch scrolling
	function handleTouchStart(event: TouchEvent) {
		touchStartY = event.touches[0].clientY;
	}

	function handleTouchMove(event: TouchEvent) {
		const touchY = event.touches[0].clientY;
		const deltaY = touchStartY - touchY;
		// If scrolling up (deltaY < 0), break autoscroll
		if (deltaY < 0) {
			userInteractedWithScroll = true;
			userHasScrolledUp = true;
		}
	}

	// Handle scroll events to re-enable autoscroll when at bottom
	function handleScroll() {
		// Check if user scrolled to bottom
		if (messagesContainer) {
			const atBottom = isAtBottom();
			if (atBottom) {
				// User is at bottom - resume autoscroll
				userHasScrolledUp = false;
				userInteractedWithScroll = false;
			}
		}
	}

	// Scroll to bottom of messages (only if user hasn't scrolled up)
	function scrollToBottom() {
		if (messagesContainer && !userHasScrolledUp) {
			messagesContainer.scrollTop = messagesContainer.scrollHeight;
		}
	}

	// Build context string from previous messages for ONNX inference
	function buildContextPrompt(userMessage: string): string {
		if (!settingsStore.contextEnabled || !currentChat || currentChat.messages.length === 0) {
			return userMessage;
		}

		// Build conversation history as a string
		const history = currentChat.messages
			.map((msg) => {
				const role = msg.role === 'user' ? 'User' : 'Assistant';
				return `${role}: ${msg.content}`;
			})
			.join('\n\n');

		return `${history}\n\nUser: ${userMessage}\n\nAssistant:`;
	}

	// Handle sending a message
	async function handleSendMessage(content: string) {
		if (!inferenceStore.isLoaded || isGenerating) return;

		// Create new chat if none exists or if this is first message after switching
		if (!currentChat) {
			chatsStore.createChat(inferenceStore.currentModel ?? 'onnx-model');
		}

		if (!currentChat) return; // Safety check

		// Hide quick examples after first message
		showQuickExamples = false;

		// Reset scroll state for new message
		userHasScrolledUp = false;
		userInteractedWithScroll = false;

		// Add user message
		const userMessage: Message = {
			id: crypto.randomUUID(),
			role: 'user',
			content,
			timestamp: Date.now()
		};
		chatsStore.addMessage(currentChat.id, userMessage);
		scrollToBottom();

		// Create placeholder for assistant response
		const assistantMessage: Message = {
			id: crypto.randomUUID(),
			role: 'assistant',
			content: '',
			timestamp: Date.now(),
			isStreaming: true
		};
		chatsStore.addMessage(currentChat.id, assistantMessage);
		scrollToBottom();

		isGenerating = true;
		cancelRequested = false; // Reset cancel flag
		currentStreamingChatId = currentChat.id; // Track which chat is streaming
		currentStreamingMessageId = assistantMessage.id; // Track which message is streaming

		// Capture chat and message IDs for the callback closure
		const chatId = currentChat.id;
		const messageId = assistantMessage.id;

		try {
			// Build prompt with context
			const prompt = buildContextPrompt(content);

			// Generation config using settings
			const config: Partial<GenerationConfig> = {
				max_length: 2048,
				temperature: settingsStore.temperature,
				top_k: 40,
				top_p: 0.9
			};

			// Start streaming generation with callback
			await inferenceStore.generateStream(
				prompt,
				(token: string) => {
					// Only process tokens if not cancelled
					if (cancelRequested) return;

					// Find the streaming message and update it
					const streamingChat = chatsStore.chats.find((c) => c.id === chatId);
					if (!streamingChat) return;

					const streamingMessage = streamingChat.messages.find((m) => m.id === messageId);
					if (!streamingMessage || !streamingMessage.isStreaming) return;

					// Update the message content
					chatsStore.updateMessage(chatId, messageId, {
						content: streamingMessage.content + token
					});

					// Only scroll if this is the currently displayed chat
					if (currentChat?.id === chatId) {
						scrollToBottom();
					}
				},
				config
			);

			// Generation complete - mark message as done
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
			isGenerating = false;
			currentStreamingChatId = null;
			currentStreamingMessageId = null;
		}
	}

	// Handle example selection
	function handleExampleSelect(prompt: string) {
		handleSendMessage(prompt);
	}

	// Handle cancel generation
	async function handleCancelGeneration() {
		cancelRequested = true;

		// Cancel the backend stream
		try {
			await inferenceStore.cancel();
		} catch (error) {
			console.error('Failed to cancel generation:', error);
		}

		isGenerating = false;

		// Mark the streaming message as no longer streaming
		if (currentStreamingChatId && currentStreamingMessageId) {
			chatsStore.updateMessage(currentStreamingChatId, currentStreamingMessageId, {
				isStreaming: false
			});
		}

		currentStreamingChatId = null;
		currentStreamingMessageId = null;
	}

	// Handle keyboard shortcuts
	function handleKeyDown(event: KeyboardEvent) {
		// Ctrl+Shift+B (Windows/Linux) or Cmd+Shift+B (Mac) to toggle benchmark panel
		const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
		const modifierKey = isMac ? event.metaKey : event.ctrlKey;

		if (modifierKey && event.shiftKey && event.key.toLowerCase() === 'b') {
			event.preventDefault();
			showBenchmarkPanel = !showBenchmarkPanel;
		}
	}

	// Calculate bottom offset to account for taskbar
	function calculateBottomOffset() {
		// Calculate the difference between visual viewport and window
		// This accounts for system UI like taskbars
		const visualViewportHeight = window.visualViewport?.height || window.innerHeight;
		const windowHeight = window.innerHeight;
		const offset = Math.max(0, windowHeight - visualViewportHeight);
		bottomOffset = offset;
	}

	// Setup event listeners and initialization
	onMount(() => {
		// Calculate initial offset
		calculateBottomOffset();

		// Update offset on resize
		const handleResize = () => calculateBottomOffset();
		window.addEventListener('resize', handleResize);
		if (window.visualViewport) {
			window.visualViewport.addEventListener('resize', handleResize);
		}

		// Initialize ONNX inference
		async function initInference() {
			// List available models
			await inferenceStore.listModels();

			// Auto-load first model if available and none loaded
			if (!inferenceStore.isLoaded && inferenceStore.availableModels.length > 0) {
				const firstModel = inferenceStore.availableModels[0];
				console.log('Auto-loading model:', firstModel.id);
				await inferenceStore.loadModel(firstModel.id);
			}
		}

		initInference();

		// Load cached hardware info
		hardwareStore.getCached();

		// Create initial chat if none exists
		if (hasNoChats) {
			chatsStore.createChat(inferenceStore.currentModel ?? 'onnx-model');
		}

		// Add keyboard event listener
		window.addEventListener('keydown', handleKeyDown);

		// Cleanup
		return () => {
			window.removeEventListener('keydown', handleKeyDown);
			window.removeEventListener('resize', handleResize);
			if (window.visualViewport) {
				window.visualViewport.removeEventListener('resize', handleResize);
			}
		};
	});

	// Watch for chat changes
	$effect(() => {
		// Track current chat ID to trigger effect
		currentChat?.id;

		// Reset scroll state when switching chats
		userHasScrolledUp = false;
		userInteractedWithScroll = false;
	});

	// Watch messages to auto-scroll
	$effect(() => {
		if (messages.length > 0) {
			scrollToBottom();
		}
	});
</script>

<div class="flex h-screen overflow-hidden bg-gray-50 dark:bg-gray-950">
	<!-- Sidebar -->
	{#if isSidebarOpen}
		<Sidebar isOpen={isSidebarOpen} onClose={() => (isSidebarOpen = false)} />
	{/if}

	<!-- Main Content -->
	<div class="flex flex-1 flex-col overflow-hidden">
		<!-- Header -->
		<header
			class="flex items-center justify-between border-b border-gray-200 bg-white px-4 py-3 dark:border-gray-800 dark:bg-gray-900"
		>
			<div class="flex items-center gap-3">
				{#if !isSidebarOpen}
					<Button variant="ghost" size="icon" onclick={() => (isSidebarOpen = true)}>
						<Menu class="h-5 w-5" />
					</Button>
				{/if}
				<h1 class="text-lg font-semibold text-gray-900 dark:text-white">
					{currentChat?.title ?? 'New Chat'}
				</h1>
			</div>

			<div class="flex items-center gap-3">
				<HardwareIndicator onclick={() => (showHardwarePanel = !showHardwarePanel)} />
				<StatusIndicator status={inferenceStore.status} />
			</div>
		</header>

		<!-- Controls Bar -->
		<div
			class="flex flex-wrap items-center gap-3 border-b border-gray-200 bg-white px-4 py-3 dark:border-gray-800 dark:bg-gray-900"
		>
			<ModelSelector />
			<ContextToggle />
		</div>

		<!-- Messages Area -->
		<div
			class="flex-1 overflow-y-auto p-4"
			bind:this={messagesContainer}
			onscroll={handleScroll}
			onwheel={handleUserScrollIntent}
			ontouchstart={handleTouchStart}
			ontouchmove={handleTouchMove}
		>
			<div class="mx-auto max-w-4xl">
				{#if messages.length === 0}
					<div class="flex min-h-[60vh] flex-col items-center justify-center text-center">
						<div class="mb-8">
							<h2 class="mb-2 text-2xl font-bold text-gray-900 dark:text-white">
								Welcome to SmolPC Code Helper!
							</h2>
							<p class="text-gray-600 dark:text-gray-400">
								Your offline AI coding assistant for learning and problem-solving
							</p>
						</div>

						{#if showQuickExamples}
							<div class="w-full max-w-3xl">
								<QuickExamples
									onSelectExample={handleExampleSelect}
									onClose={() => (showQuickExamples = false)}
								/>
							</div>
						{:else}
							<Button onclick={() => (showQuickExamples = true)} variant="outline">
								Show Quick Examples
							</Button>
						{/if}
					</div>
				{:else}
					<div class="space-y-4">
						{#each messages as message (message.id)}
							<ChatMessage {message} />
						{/each}
					</div>
				{/if}
			</div>
		</div>

		<!-- Input Area -->
		<div
			bind:this={inputAreaRef}
			class="sticky z-10 border-t border-gray-200 bg-white px-4 py-4 shadow-lg dark:border-gray-800 dark:bg-gray-900"
			style="bottom: {bottomOffset}px"
		>
			<div class="mx-auto max-w-4xl">
				{#if isGenerating}
					<div class="mb-3 flex items-center justify-center">
						<Button
							type="button"
							variant="outline"
							onclick={handleCancelGeneration}
							class="border-red-300 text-red-600 hover:bg-red-50 dark:border-red-700 dark:text-red-400 dark:hover:bg-red-950/20"
						>
							<X class="mr-2 h-4 w-4" />
							Cancel Generation
						</Button>
					</div>
				{/if}
				<ChatInput
					onSend={handleSendMessage}
					disabled={!inferenceStore.isLoaded || isGenerating}
					placeholder={isGenerating
						? 'Generating response...'
						: !inferenceStore.isLoaded
							? 'Loading model...'
							: 'Ask a coding question (Shift+Enter for new line)...'}
				/>
			</div>
		</div>
	</div>

	<!-- Hidden Benchmark Panel (Ctrl+Shift+B / Cmd+Shift+B to toggle) -->
	<BenchmarkPanel bind:visible={showBenchmarkPanel} />

	<!-- Hardware Panel -->
	<HardwarePanel bind:visible={showHardwarePanel} />
</div>
