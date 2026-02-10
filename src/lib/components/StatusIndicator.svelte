<script lang="ts">
	import type { InferenceStatus } from '$lib/types/inference';

	interface Props {
		status: InferenceStatus;
	}

	let { status }: Props = $props();
</script>

<div class={`status-indicator ${status.isGenerating ? 'status-indicator--generating' : status.isLoaded ? 'status-indicator--ready' : 'status-indicator--idle'}`}>
	<div class="status-indicator__dot"></div>
	<span class="status-indicator__text">
		{#if status.isGenerating}
			Generating...
		{:else if status.isLoaded}
			{status.currentModel ?? 'Model Ready'}
		{:else}
			No Model Loaded
		{/if}
	</span>
	{#if status.error}
		<span class="status-indicator__error">({status.error})</span>
	{/if}
</div>

<style>
	.status-indicator {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-lg);
		padding: 0.45rem 0.7rem;
		font-size: 0.8rem;
		line-height: 1;
		background: color-mix(in srgb, var(--color-card) 88%, transparent);
		max-width: min(19rem, 48vw);
	}

	.status-indicator__dot {
		width: 0.58rem;
		height: 0.58rem;
		border-radius: 9999px;
		flex-shrink: 0;
	}

	.status-indicator__text {
		font-size: 0.78rem;
		font-weight: 650;
		white-space: nowrap;
		text-overflow: ellipsis;
		overflow: hidden;
	}

	.status-indicator__error {
		color: var(--color-destructive);
		font-size: 0.7rem;
		white-space: nowrap;
		text-overflow: ellipsis;
		overflow: hidden;
	}

	.status-indicator--idle .status-indicator__dot {
		background: color-mix(in srgb, var(--color-muted-foreground) 75%, transparent);
	}

	.status-indicator--ready .status-indicator__dot {
		background: color-mix(in srgb, var(--color-success) 90%, transparent);
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
