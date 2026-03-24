<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { X } from '@lucide/svelte';

	interface Props {
		recommendedModelLabel: string;
		message: string;
		severity?: 'default' | 'warning' | 'critical';
		busy?: boolean;
		disabled?: boolean;
		onSwitch: () => void;
		onDismiss?: () => void;
	}

	let {
		recommendedModelLabel,
		message,
		severity = 'default',
		busy = false,
		disabled = false,
		onSwitch,
		onDismiss
	}: Props = $props();

	const actionLabel = $derived(
		busy ? `Switching to ${recommendedModelLabel}...` : `Switch to ${recommendedModelLabel}`
	);
	const highlightedMessageParts = $derived.by(() => {
		const markerIndex = message.indexOf(recommendedModelLabel);
		if (markerIndex === -1) {
			return { before: message, after: '' };
		}

		return {
			before: message.slice(0, markerIndex),
			after: message.slice(markerIndex + recommendedModelLabel.length)
		};
	});
</script>

<aside
	class={`model-recommendation-banner model-recommendation-banner--${severity}`}
	aria-live="polite"
>
	<div class="model-recommendation-banner__copy">
		<span class="model-recommendation-banner__eyebrow">Recommended model</span>
		<p>
			{highlightedMessageParts.before}<strong>{recommendedModelLabel}</strong
			>{highlightedMessageParts.after}
		</p>
	</div>
	<div class="model-recommendation-banner__actions">
		<Button variant="outline" onclick={onSwitch} disabled={disabled || busy}>
			{actionLabel}
		</Button>
		{#if onDismiss}
			<button
				type="button"
				class="model-recommendation-banner__dismiss"
				onclick={onDismiss}
				aria-label="Dismiss model recommendation"
				title="Dismiss"
			>
				<X class="h-4 w-4" />
			</button>
		{/if}
	</div>
</aside>

<style>
	.model-recommendation-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.85rem;
		padding: 0.85rem 1rem;
		border-bottom: 1px solid color-mix(in srgb, var(--color-primary) 18%, var(--outline-soft));
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--brand-soft) 44%, var(--surface-widget)),
				color-mix(in srgb, var(--surface-widget) 97%, black)
			),
			var(--surface-widget);
	}

	.model-recommendation-banner--warning {
		border-bottom-color: color-mix(in srgb, var(--color-warning) 24%, var(--outline-soft));
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--color-warning) 12%, var(--surface-widget)),
				color-mix(in srgb, var(--surface-widget) 97%, black)
			),
			var(--surface-widget);
	}

	.model-recommendation-banner--critical {
		border-bottom-color: color-mix(in srgb, var(--color-destructive) 28%, var(--outline-soft));
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--color-destructive) 11%, var(--surface-widget)),
				color-mix(in srgb, var(--surface-widget) 97%, black)
			),
			var(--surface-widget);
	}

	.model-recommendation-banner__copy {
		display: grid;
		gap: 0.22rem;
		min-width: 0;
	}

	.model-recommendation-banner__actions {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
	}

	.model-recommendation-banner__eyebrow {
		font-size: 0.68rem;
		font-weight: 700;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: color-mix(in srgb, var(--color-primary) 68%, var(--color-foreground));
	}

	.model-recommendation-banner--warning .model-recommendation-banner__eyebrow {
		color: color-mix(in srgb, var(--color-warning) 70%, var(--color-foreground));
	}

	.model-recommendation-banner--critical .model-recommendation-banner__eyebrow {
		color: color-mix(in srgb, var(--color-destructive) 76%, var(--color-foreground));
	}

	.model-recommendation-banner__copy p {
		font-size: 0.8rem;
		color: var(--color-muted-foreground);
	}

	.model-recommendation-banner__copy strong {
		color: var(--color-foreground);
		font-weight: 600;
	}

	.model-recommendation-banner__dismiss {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 0.38rem;
		border-radius: var(--radius-md);
		border: 1px solid transparent;
		background: transparent;
		color: var(--color-muted-foreground);
		cursor: pointer;
		transition:
			color var(--motion-fast),
			border-color var(--motion-fast),
			background var(--motion-fast);
	}

	.model-recommendation-banner__dismiss:hover {
		color: var(--color-foreground);
		border-color: var(--outline-soft);
		background: var(--surface-hover);
	}

	@media (max-width: 720px) {
		.model-recommendation-banner {
			flex-direction: column;
			align-items: stretch;
		}

		.model-recommendation-banner__actions {
			justify-content: space-between;
		}
	}
</style>
