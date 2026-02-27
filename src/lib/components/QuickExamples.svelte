<script lang="ts">
	import { QUICK_EXAMPLES } from '$lib/types/examples';
	import { X } from '@lucide/svelte';

	interface Props {
		onSelectExample: (prompt: string) => void;
		onClose?: () => void;
	}

	let { onSelectExample, onClose }: Props = $props();

	function handleSelect(prompt: string) {
		onSelectExample(prompt);
		if (onClose) onClose();
	}
</script>

<div class="quick-examples">
	<div class="quick-examples__header">
		<div class="quick-examples__title">
			<div>
				<h3>Prompt Starters</h3>
				<p>Pick one and adapt it to your assignment.</p>
			</div>
		</div>
		{#if onClose}
			<button
				onclick={onClose}
				class="quick-examples__close"
				aria-label="Close examples"
			>
				<X class="h-4 w-4" />
			</button>
		{/if}
	</div>

	<div class="quick-examples__grid">
		{#each QUICK_EXAMPLES as example (example.id)}
				<button
					onclick={() => handleSelect(example.prompt)}
					class="quick-examples__item"
				>
					<div class="quick-examples__item-head">
						<span>{example.title}</span>
					</div>
					<p>{example.prompt}</p>
				</button>
		{/each}
	</div>
</div>

<style>
	.quick-examples {
		border-radius: var(--radius-xl);
		border: 1px solid var(--outline-soft);
		padding: 1rem;
		background:
			linear-gradient(
				145deg,
				color-mix(in srgb, var(--brand-soft) 65%, transparent),
				color-mix(in srgb, var(--surface-widget) 96%, black)
			),
			var(--surface-subtle);
		box-shadow: var(--glow-subtle);
	}

	.quick-examples__header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 0.7rem;
		margin-bottom: 0.85rem;
	}

	.quick-examples__title {
		display: block;
		color: color-mix(in srgb, var(--color-primary) 55%, var(--color-foreground));
	}

	.quick-examples__title h3 {
		font-size: 0.92rem;
		font-weight: 700;
		color: var(--color-foreground);
	}

	.quick-examples__title p {
		font-size: 0.75rem;
		color: var(--color-muted-foreground);
	}

	.quick-examples__close {
		padding: 0.28rem;
		border-radius: var(--radius-md);
		border: 1px solid transparent;
		color: var(--color-muted-foreground);
		background: transparent;
		cursor: pointer;
	}

	.quick-examples__close:hover {
		color: var(--color-foreground);
		border-color: var(--outline-soft);
		background: var(--surface-hover);
	}

	.quick-examples__grid {
		display: grid;
		gap: 0.55rem;
		grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr));
	}

	.quick-examples__item {
		display: grid;
		gap: 0.4rem;
		padding: 0.75rem 0.8rem;
		text-align: left;
		border-radius: var(--radius-lg);
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 96%, black);
		color: inherit;
		cursor: pointer;
		transition:
			transform var(--motion-fast),
			border-color var(--motion-fast),
			background var(--motion-fast);
	}

	.quick-examples__item:hover {
		transform: translateY(-0.5px);
		border-color: var(--outline-strong);
		background: var(--surface-active);
	}

	.quick-examples__item-head {
		display: block;
		font-size: 0.81rem;
		font-weight: 700;
	}

	.quick-examples__item p {
		font-size: 0.74rem;
		color: var(--color-muted-foreground);
		line-height: 1.35;
		display: -webkit-box;
		line-clamp: 2;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}
</style>
