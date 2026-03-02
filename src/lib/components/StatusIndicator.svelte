<script lang="ts">
	import type { InferenceStatus } from '$lib/types/inference';

	interface Props {
		status: InferenceStatus;
	}

	let { status }: Props = $props();

	function formatBackendLabel(backend: string | null): string {
		if (!backend) {
			return 'Unknown Backend';
		}
		if (backend === 'directml') {
			return 'DirectML';
		}
		if (backend === 'cpu') {
			return 'CPU';
		}
		return backend.toUpperCase();
	}

	function formatReason(reason: string | null): string {
		if (!reason) {
			return '';
		}
		return reason.replaceAll('_', ' ');
	}
</script>

<div class={`status-indicator ${status.isGenerating ? 'status-indicator--generating' : status.isLoaded ? 'status-indicator--ready' : 'status-indicator--idle'}`}>
	<div class="status-indicator__dot"></div>
	<div class="status-indicator__content">
		<span class="status-indicator__text">
		{#if status.isGenerating}
			Generating...
		{:else if status.isLoaded}
			{status.currentModel ?? 'Model Ready'} • {formatBackendLabel(status.activeBackend)}
		{:else}
			No Model Loaded
		{/if}
		</span>
		{#if status.isLoaded && status.runtimeEngine}
			<span class="status-indicator__runtime">{status.runtimeEngine}</span>
		{/if}
		{#if status.isLoaded && status.selectionState === 'fallback'}
			<span class="status-indicator__runtime">
				Fallback active{status.selectedDeviceName ? ` (${status.selectedDeviceName})` : ''}: {formatReason(status.selectionReason)}
			</span>
		{/if}
	</div>
	{#if status.error}
		<span class="status-indicator__error">({status.error})</span>
	{/if}
</div>

<style>
	.status-indicator {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		border: 1px solid var(--outline-soft);
		border-radius: var(--radius-lg);
		padding: 0.45rem 0.7rem;
		font-size: 0.76rem;
		line-height: 1;
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
		max-width: min(19rem, 48vw);
		box-shadow: var(--glow-subtle);
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
		color: var(--color-muted-foreground);
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
