<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import ChatMessage from '$lib/components/ChatMessage.svelte';
	import ChatInput from '$lib/components/ChatInput.svelte';
	import StatusIndicator from '$lib/components/StatusIndicator.svelte';
	import ModelSelector from '$lib/components/ModelSelector.svelte';
	import ContextToggle from '$lib/components/ContextToggle.svelte';
	import QuickExamples from '$lib/components/QuickExamples.svelte';
	import { chatsStore } from '$lib/stores/chats.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { ollamaStore } from '$lib/stores/ollama.svelte';
	import type { Message } from '$lib/types/chat';
	import type { OllamaMessage } from '$lib/types/ollama';
	import { Menu, X } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';

	// UI State
	let isSidebarOpen = $state(true);
	let isGenerating = $state(false);
	let showQuickExamples = $state(true);
	let messagesContainer: HTMLDivElement;
	let userHasScrolledUp = $state(false);
	let cancelRequested = $state(false);
	let currentStreamingChatId = $state<string | null>(null);
	let userInteractedWithScroll = $state(false);
	let touchStartY = $state(0);

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

	// Build context from previous messages
	function buildContext(): OllamaMessage[] {
		if (!settingsStore.contextEnabled || !currentChat) {
			return [];
		}

		return currentChat.messages.map((msg) => ({
			role: msg.role === 'user' ? 'user' : 'assistant',
			content: msg.content
		}));
	}

	// Handle sending a message
	async function handleSendMessage(content: string) {
		if (!ollamaStore.isConnected || isGenerating) return;

		// Create new chat if none exists or if this is first message after switching
		if (!currentChat) {
			chatsStore.createChat(settingsStore.selectedModel);
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

		try {
			// Build context from previous messages
			const context = buildContext();

			// Start streaming generation
			await invoke('generate_stream', {
				prompt: content,
				model: settingsStore.selectedModel,
				context: context.length > 0 ? context : null
			});
		} catch (error) {
			console.error('Generation error:', error);
			chatsStore.updateMessage(currentChat.id, assistantMessage.id, {
				content: `Error: ${error}`,
				isStreaming: false
			});
			isGenerating = false;
			currentStreamingChatId = null;
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
			await invoke('cancel_generation');
		} catch (error) {
			console.error('Failed to cancel generation:', error);
		}

		isGenerating = false;
		currentStreamingChatId = null;

		// Mark the last message as no longer streaming
		if (currentChat) {
			const lastMessage = messages[messages.length - 1];
			if (lastMessage && lastMessage.role === 'assistant' && lastMessage.isStreaming) {
				chatsStore.updateMessage(currentChat.id, lastMessage.id, {
					isStreaming: false
				});
			}
		}
	}

	// Setup event listeners and initialization
	onMount(() => {
		let unlistenChunk: UnlistenFn;
		let unlistenDone: UnlistenFn;
		let unlistenError: UnlistenFn;
		let unlistenCancelled: UnlistenFn;

		async function setupListeners() {
			// Listen for streaming chunks
			unlistenChunk = await listen<string>('ollama_chunk', (event) => {
				// Only process chunks if we're streaming for the current chat
				if (!currentChat || !currentStreamingChatId || currentChat.id !== currentStreamingChatId || cancelRequested) {
					return;
				}

				const lastMessage = messages[messages.length - 1];
				if (lastMessage && lastMessage.role === 'assistant' && lastMessage.isStreaming) {
					chatsStore.updateMessage(currentChat.id, lastMessage.id, {
						content: lastMessage.content + event.payload
					});
					scrollToBottom();
				}
			});

			// Listen for generation complete
			unlistenDone = await listen('ollama_done', () => {
				if (!currentChat || !currentStreamingChatId) return;

				const lastMessage = messages[messages.length - 1];
				if (lastMessage && lastMessage.role === 'assistant') {
					chatsStore.updateMessage(currentChat.id, lastMessage.id, {
						isStreaming: false
					});
				}
				isGenerating = false;
				currentStreamingChatId = null;
			});

			// Listen for cancellation
			unlistenCancelled = await listen('ollama_cancelled', () => {
				isGenerating = false;
				currentStreamingChatId = null;
			});

			// Listen for errors
			unlistenError = await listen<string>('ollama_error', (event) => {
				if (!currentChat) return;

				const lastMessage = messages[messages.length - 1];
				if (lastMessage && lastMessage.role === 'assistant') {
					chatsStore.updateMessage(currentChat.id, lastMessage.id, {
						content: `Error: ${event.payload}`,
						isStreaming: false
					});
				}
				isGenerating = false;
				currentStreamingChatId = null;
			});
		}

		// Initial Ollama check
		ollamaStore.checkConnection();

		// Setup event listeners and track cleanup
		const cleanupPromise = setupListeners();

		// Create initial chat if none exists
		if (hasNoChats) {
			chatsStore.createChat(settingsStore.selectedModel);
		}

		// Cleanup - wait for setup to complete before cleaning up
		return async () => {
			await cleanupPromise;
			if (unlistenChunk) unlistenChunk();
			if (unlistenDone) unlistenDone();
			if (unlistenError) unlistenError();
			if (unlistenCancelled) unlistenCancelled();
		};
	});

	// Watch for chat changes and cancel any active stream
	$effect(() => {
		const chatId = currentChat?.id;

		// If we're generating for a different chat, cancel it
		if (currentStreamingChatId && chatId !== currentStreamingChatId) {
			handleCancelGeneration();
		}

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
				<StatusIndicator status={ollamaStore.status} />
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
			class="border-t border-gray-200 bg-white px-4 py-4 dark:border-gray-800 dark:bg-gray-900"
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
					disabled={!ollamaStore.isConnected || isGenerating}
					placeholder={isGenerating
						? 'Generating response...'
						: 'Ask a coding question (Shift+Enter for new line)...'}
				/>
			</div>
		</div>
	</div>
</div>
