<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { CornerDownLeft, Send } from '@lucide/svelte';

	interface Props {
		onSend: (message: string) => void;
		disabled?: boolean;
		placeholder?: string;
	}

	let { onSend, disabled = false, placeholder = 'Ask a coding question...' }: Props = $props();

	let inputValue = $state('');
	let textarea: HTMLTextAreaElement;

	function handleSubmit() {
		const trimmed = inputValue.trim();
		if (trimmed && !disabled) {
			onSend(trimmed);
			inputValue = '';
			if (textarea) {
				textarea.style.height = 'auto';
			}
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSubmit();
		}
	}

	function handleInput() {
		if (textarea) {
			textarea.style.height = 'auto';
			textarea.style.height = textarea.scrollHeight + 'px';
		}
	}
</script>

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
	<Button onclick={handleSubmit} {disabled} size="icon" class="chat-input__send">
		<Send class="h-4.5 w-4.5" />
	</Button>
</div>

<style>
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
</style>
