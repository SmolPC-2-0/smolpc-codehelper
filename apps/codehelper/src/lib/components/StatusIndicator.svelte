<script lang="ts">
	import type { InferenceStatus } from '$lib/types/inference';

	interface Props {
		status: InferenceStatus;
		active?: boolean;
		onToggle?: () => void;
	}

	let { status, active = false, onToggle }: Props = $props();
	const statusClass = $derived(
		status.isGenerating
			? 'status-indicator--generating'
			: status.readinessState === 'ready'
				? 'status-indicator--ready'
				: status.readinessState === 'failed'
					? 'status-indicator--failed'
					: 'status-indicator--starting'
	);

	function handleClick() {
		onToggle?.();
	}
</script>

<button
	type="button"
	class={`status-indicator ${active ? 'status-indicator--active' : ''} ${statusClass}`}
	onclick={handleClick}
	aria-label="Open model and runtime info"
	title="Open model and runtime info"
>
	<div class="status-indicator__dot"></div>
	<div class="status-indicator__content">
		<span class="status-indicator__text">
			{#if status.isGenerating}
				Generating
			{:else if status.readinessState === 'ready'}
				{status.currentModel ?? 'Model loaded'}
			{:else if status.readinessState === 'failed'}
				Startup failed
			{:else}
				Starting engine...
			{/if}
		</span>
		{#if status.readinessState === 'failed' && status.startupErrorCode}
			<span class="status-indicator__runtime">{status.startupErrorCode}</span>
		{:else if status.isLoaded}
			<span class="status-indicator__runtime"> Open model and runtime settings </span>
		{/if}
	</div>
</button>

<style>
	.status-indicator {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		font: inherit;
		color: inherit;
		border: 1px solid var(--outline-soft);
		border-radius: var(--radius-lg);
		padding: 0.52rem 0.72rem;
		font-size: 0.76rem;
		line-height: 1.18;
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
		max-width: min(21rem, 58vw);
		box-shadow: var(--glow-subtle);
		cursor: pointer;
		text-align: left;
		transition: border-color 120ms ease;
	}

	.status-indicator:hover {
		border-color: var(--outline-strong);
	}

	.status-indicator--active {
		border-color: var(--outline-strong);
		background: var(--surface-active);
	}

	.status-indicator__dot {
		width: 0.58rem;
		height: 0.58rem;
		border-radius: 9999px;
		flex-shrink: 0;
	}

	.status-indicator__text {
		font-size: 0.74rem;
		font-weight: 620;
		line-height: 1.2;
		white-space: nowrap;
		text-overflow: ellipsis;
		overflow: hidden;
	}

	.status-indicator__content {
		display: flex;
		min-width: 0;
		flex-direction: column;
		gap: 0.16rem;
	}

	.status-indicator__runtime {
		font-size: 0.66rem;
		line-height: 1.2;
		color: var(--color-muted-foreground);
		white-space: nowrap;
		text-overflow: ellipsis;
		overflow: hidden;
	}

	.status-indicator--starting .status-indicator__dot {
		background: color-mix(in srgb, var(--color-muted-foreground) 75%, transparent);
	}

	.status-indicator--ready .status-indicator__dot {
		background: color-mix(in srgb, var(--color-success) 90%, transparent);
	}

	.status-indicator--failed .status-indicator__dot {
		background: color-mix(in srgb, var(--color-destructive) 92%, transparent);
	}

	.status-indicator--generating .status-indicator__dot {
		background: color-mix(in srgb, var(--color-warning) 92%, transparent);
		animation: pulse-dot 1.35s ease-in-out infinite;
	}

	@keyframes pulse-dot {
		0%,
		100% {
			transform: scale(1);
			opacity: 0.7;
		}
		50% {
			transform: scale(1.15);
			opacity: 1;
		}
	}

	@media (max-width: 768px) {
		.status-indicator {
			max-width: 100%;
		}
	}
</style>
