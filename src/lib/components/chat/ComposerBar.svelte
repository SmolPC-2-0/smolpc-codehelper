<script lang="ts">
	import ChatInput from '$lib/components/ChatInput.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Square } from '@lucide/svelte';

	interface Props {
		isLoaded: boolean;
		isGenerating: boolean;
		bottomOffset: number;
		onSend: (content: string) => void;
		onCancel: () => void;
	}

	let { isLoaded, isGenerating, bottomOffset, onSend, onCancel }: Props = $props();

	const inputPlaceholder = $derived(
		isGenerating
			? 'Generating response...'
			: !isLoaded
				? 'Loading model...'
				: 'Ask a coding question (Shift+Enter for new line)...'
	);
</script>

<section class="composer-shell" style="bottom: {bottomOffset}px">
	<div class="composer-shell__inner">
		{#if isGenerating}
			<div class="composer-shell__actions">
				<Button type="button" variant="outline" class="composer-shell__cancel" onclick={onCancel}>
					<Square class="mr-2 h-3.5 w-3.5" />
					Stop generation
				</Button>
			</div>
		{/if}

		<ChatInput
			{onSend}
			disabled={!isLoaded || isGenerating}
			placeholder={inputPlaceholder}
		/>
	</div>
</section>

<style>
	.composer-shell {
		position: sticky;
		z-index: 12;
		padding: 0.9rem 1rem 1rem;
		border-top: 1px solid var(--color-border);
		background:
			linear-gradient(
				180deg,
				var(--surface-widget),
				var(--surface-subtle)
			),
			var(--surface-subtle);
		backdrop-filter: blur(10px);
	}

	.composer-shell__inner {
		max-width: 66rem;
		margin: 0 auto;
	}

	.composer-shell__actions {
		display: flex;
		justify-content: center;
		margin-bottom: 0.65rem;
	}

	:global(.composer-shell__cancel) {
		border-color: color-mix(in srgb, var(--color-destructive) 55%, var(--color-border));
		color: var(--color-destructive);
		background: color-mix(in srgb, var(--color-destructive) 9%, transparent);
		box-shadow: var(--shadow-soft);
	}

	:global(.composer-shell__cancel:hover) {
		background: color-mix(in srgb, var(--color-destructive) 12%, transparent);
	}

	@media (max-width: 768px) {
		.composer-shell {
			padding: 0.7rem 0.8rem 0.85rem;
		}
	}
</style>
