<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import ChatMessage from '$lib/components/ChatMessage.svelte';
	import ChatInput from '$lib/components/ChatInput.svelte';
	import StatusIndicator from '$lib/components/StatusIndicator.svelte';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import type { Message, OllamaStatus } from '$lib/types/ollama';

	// State
	let messages = $state<Message[]>([]);
	let ollamaStatus = $state<OllamaStatus>({
		connected: false,
		checking: true
	});
	let isGenerating = $state(false);
	let currentModel = $state('qwen2.5-coder:7b');
	let messagesContainer: HTMLDivElement;

	// Generate unique ID
	function generateId(): string {
		return Date.now().toString(36) + Math.random().toString(36).substr(2);
	}

	// Scroll to bottom of messages
	function scrollToBottom() {
		if (messagesContainer) {
			setTimeout(() => {
				messagesContainer.scrollTop = messagesContainer.scrollHeight;
			}, 0);
		}
	}

	// Check Ollama status
	async function checkOllama() {
		ollamaStatus.checking = true;
		try {
			const connected = await invoke<boolean>('check_ollama');
			ollamaStatus = {
				connected,
				checking: false,
				error: connected ? undefined : 'Please start Ollama server'
			};
		} catch (error) {
			ollamaStatus = {
				connected: false,
				checking: false,
				error: 'Failed to check Ollama status'
			};
		}
	}

	// Handle sending a message
	async function handleSendMessage(content: string) {
		if (!ollamaStatus.connected || isGenerating) return;

		// Add user message
		const userMessage: Message = {
			id: generateId(),
			role: 'user',
			content,
			timestamp: Date.now()
		};
		messages.push(userMessage);
		messages = messages; // Trigger reactivity
		scrollToBottom();

		// Create placeholder for assistant response
		const assistantMessage: Message = {
			id: generateId(),
			role: 'assistant',
			content: '',
			timestamp: Date.now(),
			isStreaming: true
		};
		messages.push(assistantMessage);
		messages = messages; // Trigger reactivity
		scrollToBottom();

		isGenerating = true;

		try {
			// Start streaming generation
			await invoke('generate_stream', {
				prompt: content,
				model: currentModel,
				context: null // Phase 1: no context yet
			});
		} catch (error) {
			console.error('Generation error:', error);
			assistantMessage.content = `Error: ${error}`;
			assistantMessage.isStreaming = false;
			messages = messages;
		}
	}

	// Setup event listeners
	onMount(() => {
		let unlistenChunk: UnlistenFn;
		let unlistenDone: UnlistenFn;
		let unlistenError: UnlistenFn;

		async function setupListeners() {
			// Listen for streaming chunks
			unlistenChunk = await listen<string>('ollama_chunk', (event) => {
				const lastMessage = messages[messages.length - 1];
				if (lastMessage && lastMessage.role === 'assistant' && lastMessage.isStreaming) {
					lastMessage.content += event.payload;
					messages = messages; // Trigger reactivity
					scrollToBottom();
				}
			});

			// Listen for generation complete
			unlistenDone = await listen('ollama_done', () => {
				const lastMessage = messages[messages.length - 1];
				if (lastMessage && lastMessage.role === 'assistant') {
					lastMessage.isStreaming = false;
					messages = messages; // Trigger reactivity
				}
				isGenerating = false;
			});

			// Listen for errors
			unlistenError = await listen<string>('ollama_error', (event) => {
				const lastMessage = messages[messages.length - 1];
				if (lastMessage && lastMessage.role === 'assistant') {
					lastMessage.content = `Error: ${event.payload}`;
					lastMessage.isStreaming = false;
					messages = messages; // Trigger reactivity
				}
				isGenerating = false;
			});
		}

		// Initial Ollama check
		checkOllama();

		// Setup event listeners
		setupListeners();

		// Cleanup
		return () => {
			if (unlistenChunk) unlistenChunk();
			if (unlistenDone) unlistenDone();
			if (unlistenError) unlistenError();
		};
	});
</script>

<main class="flex h-screen flex-col bg-background p-4">
	<div class="mx-auto w-full max-w-4xl flex-1 flex flex-col gap-4">
		<!-- Header -->
		<Card>
			<CardHeader class="flex flex-row items-center justify-between space-y-0 pb-4">
				<CardTitle class="text-2xl font-bold">SmolPC Code Helper</CardTitle>
				<StatusIndicator status={ollamaStatus} />
			</CardHeader>
		</Card>

		<!-- Messages -->
		<Card class="flex-1 flex flex-col overflow-hidden">
			<CardContent class="flex-1 overflow-y-auto p-4" bind:this={messagesContainer}>
				{#if messages.length === 0}
					<div class="flex h-full items-center justify-center text-center text-muted-foreground">
						<div>
							<h3 class="mb-2 text-lg font-semibold">Welcome to SmolPC Code Helper!</h3>
							<p>Ask any coding question to get started.</p>
						</div>
					</div>
				{:else}
					{#each messages as message (message.id)}
						<ChatMessage {message} />
					{/each}
				{/if}
			</CardContent>
		</Card>

		<!-- Input -->
		<Card>
			<CardContent class="p-4">
				<ChatInput onSend={handleSendMessage} disabled={!ollamaStatus.connected || isGenerating} />
			</CardContent>
		</Card>
	</div>
</main>
