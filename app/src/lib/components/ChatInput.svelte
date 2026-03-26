<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { composerDraftStore } from '$lib/stores/composerDraft.svelte';
	import { voiceStore } from '$lib/stores/voice.svelte';
	import { CornerDownLeft, Mic, Square, Loader2, Send } from '@lucide/svelte';

	interface Props {
		onSend: (message: string) => void;
		disabled?: boolean;
		placeholder?: string;
		draftKey?: string;
		showMicButton?: boolean;
	}

	let {
		onSend,
		disabled = false,
		placeholder = 'Ask a coding question...',
		draftKey = 'global',
		showMicButton = false
	}: Props = $props();

	let inputValue = $state('');
	let textarea: HTMLTextAreaElement;
	let hydratedDraftKey = $state<string | null>(null);
	const normalizedDraftKey = $derived(draftKey.trim() || 'global');
	const showMicStatusPill = $derived(
		showMicButton &&
			(voiceStore.micState === 'arming' ||
				voiceStore.micState === 'ready' ||
				voiceStore.micState === 'recording' ||
				voiceStore.micState === 'processing')
	);
	const isMicActionDisabled = $derived(
		disabled ||
			voiceStore.micState === 'arming' ||
			voiceStore.micState === 'processing' ||
			voiceStore.micState === 'disabled'
	);
	const isMicStopState = $derived(
		voiceStore.micState === 'ready' || voiceStore.micState === 'recording'
	);
	const micStatusTone = $derived.by(() => {
		switch (voiceStore.micState) {
			case 'arming':
			case 'processing':
				return 'loading';
			case 'ready':
			case 'recording':
				return 'recording';
			default:
				return 'idle';
		}
	});
	const micStatusCopy = $derived.by(() => {
		switch (voiceStore.micState) {
			case 'arming':
				return {
					headline: 'Starting microphone...',
					detail: 'Switching to listening...'
				};
			case 'ready':
			case 'recording':
				return {
					headline: 'Listening...',
					detail: 'Tap the square to stop'
				};
			case 'processing':
				return {
					headline: 'Transcribing...',
					detail: 'Adding text to your draft'
				};
			default:
				return {
					headline: '',
					detail: ''
				};
		}
	});

	function resizeTextarea() {
		if (textarea) {
			textarea.style.height = 'auto';
			textarea.style.height = textarea.scrollHeight + 'px';
		}
	}

	function handleSubmit() {
		const trimmed = inputValue.trim();
		if (trimmed && !disabled) {
			onSend(trimmed);
			inputValue = '';
			composerDraftStore.clearDraft(normalizedDraftKey);
			// Defer resize to next microtask so Svelte flushes the empty value
			// to the DOM before we measure scrollHeight.
			queueMicrotask(() => resizeTextarea());
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSubmit();
		}
	}

	function handleInput() {
		resizeTextarea();
		composerDraftStore.setDraft(normalizedDraftKey, inputValue);
	}

	async function handleMicClick() {
		if (voiceStore.micState === 'ready' || voiceStore.micState === 'recording') {
			const text = await voiceStore.stopRecording();
			if (text.trim()) {
				const start = textarea?.selectionStart ?? inputValue.length;
				const end = textarea?.selectionEnd ?? inputValue.length;
				const before = inputValue.slice(0, start);
				const after = inputValue.slice(end);
				const sep = before && !before.endsWith(' ') ? ' ' : '';
				inputValue = before + sep + text + after;
				composerDraftStore.setDraft(normalizedDraftKey, inputValue);
				queueMicrotask(() => {
					resizeTextarea();
					const newPos = before.length + sep.length + text.length;
					if (textarea) {
						textarea.selectionStart = newPos;
						textarea.selectionEnd = newPos;
						textarea.focus();
					}
				});
			}
		} else if (voiceStore.micState === 'idle') {
			await voiceStore.startRecording();
		}
	}

	const micButtonTitle = $derived.by(() => {
		switch (voiceStore.micState) {
			case 'arming':
				return 'Starting microphone...';
			case 'ready':
				return 'Microphone is live. Tap to stop recording.';
			case 'recording':
				return 'Stop recording';
			case 'processing':
				return 'Transcribing...';
			case 'disabled':
				return 'No microphone detected';
			case 'idle':
			default:
				return 'Voice input';
		}
	});

	$effect(() => {
		const key = normalizedDraftKey;
		if (hydratedDraftKey === key) {
			return;
		}

		hydratedDraftKey = key;
		inputValue = composerDraftStore.getDraft(key);
		queueMicrotask(() => resizeTextarea());
	});
</script>

<div class="chat-input-shell">
	{#if showMicStatusPill}
		<div class="chat-input__status-row">
			<div
				class={`chat-input__status chat-input__status--${micStatusTone} ${showMicButton ? 'chat-input__status--with-mic' : ''}`}
				role="status"
				aria-live="polite"
			>
				<span class="chat-input__status-headline">{micStatusCopy.headline}</span>
				<span class="chat-input__status-detail">{micStatusCopy.detail}</span>
			</div>
		</div>
	{/if}

	<div class="chat-input">
		<div class="chat-input__field">
			<textarea
				bind:this={textarea}
				bind:value={inputValue}
				onkeydown={handleKeydown}
				oninput={handleInput}
				{placeholder}
				{disabled}
				rows="1"
				class="chat-input__textarea"
				style="max-height: 200px; overflow-y: auto;"
			></textarea>
			<div class="chat-input__hint">
				<CornerDownLeft class="h-3.5 w-3.5" />
				<span>Enter to send, Shift+Enter newline</span>
			</div>
		</div>
		{#if showMicButton}
			<button
				type="button"
				class="chat-input__mic"
				class:chat-input__mic--arming={voiceStore.micState === 'arming'}
				class:chat-input__mic--ready={voiceStore.micState === 'ready'}
				class:chat-input__mic--recording={voiceStore.micState === 'recording'}
				class:chat-input__mic--processing={voiceStore.micState === 'processing'}
				onclick={handleMicClick}
				disabled={isMicActionDisabled}
				title={micButtonTitle}
			>
				{#if voiceStore.micState === 'arming' || voiceStore.micState === 'processing'}
					<Loader2 class="h-4 w-4 animate-spin" />
				{:else if isMicStopState}
					<Square class="h-4 w-4" />
				{:else}
					<Mic class="h-4 w-4" />
				{/if}
			</button>
		{/if}
		<Button onclick={handleSubmit} {disabled} size="icon" class="chat-input__send">
			<Send class="h-4.5 w-4.5" />
		</Button>
	</div>
</div>

<style>
	.chat-input-shell {
		display: grid;
		gap: 0.45rem;
	}

	.chat-input__status-row {
		display: flex;
		justify-content: flex-end;
	}

	.chat-input__status {
		display: flex;
		flex-direction: column;
		gap: 0.08rem;
		max-width: min(18rem, 100%);
		padding: 0.55rem 0.75rem;
		border-radius: var(--radius-lg);
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-floating) 96%, black);
		box-shadow: var(--glow-subtle);
		backdrop-filter: blur(10px);
	}

	.chat-input__status--with-mic {
		margin-right: 3.65rem;
	}

	.chat-input__status-headline {
		font-size: 0.72rem;
		font-weight: 650;
		line-height: 1.15;
	}

	.chat-input__status-detail {
		font-size: 0.65rem;
		line-height: 1.2;
		color: var(--color-muted-foreground);
	}

	.chat-input__status--loading {
		border-color: color-mix(in srgb, var(--outline-strong) 86%, transparent);
		background: color-mix(in srgb, var(--brand-soft) 86%, var(--surface-floating));
	}

	.chat-input__status--recording {
		border-color: color-mix(in srgb, var(--color-primary) 42%, var(--outline-strong));
		background: color-mix(in srgb, var(--color-primary) 12%, var(--surface-floating));
	}

	.chat-input {
		display: flex;
		align-items: flex-end;
		gap: 0.65rem;
		padding: 0.45rem;
		border-radius: calc(var(--radius-xl) + 5px);
		border: 1px solid var(--outline-soft);
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--surface-elevated) 96%, black),
				color-mix(in srgb, var(--surface-subtle) 96%, black)
			),
			var(--surface-elevated);
		box-shadow: var(--shadow-strong);
	}

	.chat-input__field {
		position: relative;
		flex: 1;
	}

	.chat-input__textarea {
		width: 100%;
		resize: none;
		padding: 0.78rem 0.95rem 1.85rem;
		border-radius: calc(var(--radius-xl) - 1px);
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
		color: var(--color-foreground);
		font-size: 0.9rem;
		line-height: 1.45;
		outline: none;
		transition:
			border-color var(--motion-fast),
			box-shadow var(--motion-fast),
			background var(--motion-fast);
	}

	.chat-input__textarea::placeholder {
		color: var(--color-muted-foreground);
	}

	.chat-input__textarea:focus {
		border-color: var(--outline-strong);
		box-shadow: 0 0 0 4px var(--focus-ring);
	}

	.chat-input__textarea:disabled {
		opacity: 0.7;
		cursor: not-allowed;
	}

	.chat-input__hint {
		position: absolute;
		left: 0.75rem;
		bottom: 0.45rem;
		display: inline-flex;
		align-items: center;
		gap: 0.28rem;
		font-size: 0.65rem;
		color: var(--color-muted-foreground);
	}

	:global(.chat-input__send) {
		height: 3rem;
		width: 3rem;
		border-radius: calc(var(--radius-xl) + 4px);
		background: color-mix(in srgb, var(--color-primary) 80%, black);
		color: var(--color-primary-foreground);
		box-shadow: var(--glow-subtle);
	}

	:global(.chat-input__send:hover) {
		filter: brightness(1.03);
	}

	textarea::-webkit-scrollbar {
		width: 8px;
	}

	textarea::-webkit-scrollbar-track {
		background: transparent;
	}

	textarea::-webkit-scrollbar-thumb {
		background: color-mix(in srgb, var(--color-muted-foreground) 60%, transparent);
		border-radius: 4px;
	}

	textarea::-webkit-scrollbar-thumb:hover {
		background: color-mix(in srgb, var(--color-muted-foreground) 80%, transparent);
	}

	.chat-input__mic {
		position: relative;
		height: 3rem;
		width: 3rem;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		border-radius: calc(var(--radius-xl) + 4px);
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 96%, black);
		color: var(--color-muted-foreground);
		cursor: pointer;
		transition:
			color var(--motion-fast),
			background var(--motion-fast),
			border-color var(--motion-fast);
	}

	.chat-input__mic:hover:not(:disabled) {
		color: var(--color-foreground);
		border-color: var(--outline-strong);
	}

	.chat-input__mic--arming,
	.chat-input__mic--processing {
		border-color: var(--outline-strong);
		color: var(--color-primary);
		background: color-mix(in srgb, var(--brand-soft) 78%, var(--surface-widget));
	}

	.chat-input__mic:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.chat-input__mic--ready {
		border-color: color-mix(in srgb, var(--color-success) 44%, var(--outline-strong));
		color: color-mix(in srgb, var(--color-success) 82%, var(--color-foreground));
		background: color-mix(in srgb, var(--color-success) 12%, var(--surface-widget));
		animation: mic-ready-flash 1.1s ease-out 1;
	}

	.chat-input__mic--recording {
		border-color: var(--color-destructive);
		color: var(--color-destructive);
		background: color-mix(in srgb, var(--color-destructive) 10%, var(--surface-widget));
		animation: mic-pulse 1.4s ease-in-out infinite;
	}

	@keyframes mic-ready-flash {
		0% {
			box-shadow: 0 0 0 0 color-mix(in srgb, var(--color-success) 18%, transparent);
			transform: scale(0.96);
		}
		45% {
			box-shadow: 0 0 0 8px color-mix(in srgb, var(--color-success) 10%, transparent);
			transform: scale(1.03);
		}
		100% {
			box-shadow: var(--glow-subtle);
			transform: scale(1);
		}
	}

	@keyframes mic-pulse {
		0%,
		100% {
			opacity: 0.7;
		}
		50% {
			opacity: 1;
		}
	}

	@media (max-width: 768px) {
		.chat-input__status {
			max-width: none;
			width: 100%;
		}

		.chat-input__status--with-mic {
			margin-right: 0;
		}
	}
</style>
