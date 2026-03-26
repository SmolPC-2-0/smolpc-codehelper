<script lang="ts">
	import { Loader2, Check, AlertCircle, RefreshCw } from '@lucide/svelte';
	import { inferenceStore } from '$lib/stores/inference.svelte';
	import type { EngineReadinessDto } from '$lib/types/inference';

	interface Props {
		readiness: EngineReadinessDto | null;
		onRetry: () => void;
		visible: boolean;
	}

	let { readiness, onRetry, visible }: Props = $props();

	// Fast poll engine status while loading screen is visible
	$effect(() => {
		if (!visible) return;
		const state = readiness?.state;
		if (state === 'ready' || state === 'failed') return;

		const intervalId = setInterval(() => {
			inferenceStore.refreshReadiness();
		}, 500);

		return () => clearInterval(intervalId);
	});

	const STATUS_MESSAGES: Record<string, string> = {
		idle: 'Starting engine...',
		starting: 'Starting engine...',
		probing: 'Detecting hardware...',
		resolving_assets: 'Checking model files...',
		loading_model: 'Loading AI model...',
		ready: 'Ready'
	};

	const statusMessage = $derived(
		STATUS_MESSAGES[readiness?.state ?? 'idle'] ?? 'Starting engine...'
	);
	const isFailed = $derived(readiness?.state === 'failed');
	const isReady = $derived(readiness?.state === 'ready');
	const errorMessage = $derived(
		readiness?.error_message ?? 'Something went wrong during startup.'
	);

	// Show model info once we're in loading_model or ready state
	const modelInfo = $derived.by(() => {
		const model = readiness?.active_model_id;
		const backend = readiness?.active_backend;
		if (!model && !backend) return null;
		if (model && backend) return `${model} on ${backend}`;
		if (model) return model;
		return null;
	});

	const showSlowHint = $derived.by(() => {
		if (!readiness?.state_since) return false;
		const since = new Date(readiness.state_since).getTime();
		return Date.now() - since > 15000;
	});
</script>

{#if visible}
	<div class="startup-overlay" class:fading={isReady}>
		<div class="startup-content">
			<h1 class="startup-title">SmolPC 2.0</h1>

			{#if isFailed}
				<div class="startup-status startup-status--error">
					<AlertCircle size={20} />
					<span>{errorMessage}</span>
				</div>
				<button class="startup-retry" onclick={onRetry}>
					<RefreshCw size={14} />
					Try again
				</button>
			{:else if isReady}
				<div class="startup-status startup-status--ready">
					<Check size={20} />
					<span>{statusMessage}</span>
				</div>
				{#if modelInfo}
					<p class="startup-detail">{modelInfo}</p>
				{/if}
			{:else}
				<div class="startup-status startup-status--loading">
					<Loader2 size={20} class="spin" />
					<span>{statusMessage}</span>
				</div>
				{#if modelInfo}
					<p class="startup-detail">{modelInfo}</p>
				{/if}
				{#if showSlowHint}
					<p class="startup-hint">This may take a moment on first launch</p>
				{/if}
			{/if}
		</div>
	</div>
{/if}

<style>
	.startup-overlay {
		position: fixed;
		inset: 0;
		z-index: 100;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--surface-base, #0a0a0f);
		transition: opacity 0.4s ease-out;
	}

	.startup-overlay.fading {
		opacity: 0;
		pointer-events: none;
	}

	.startup-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1.2rem;
		max-width: 320px;
		padding: 2rem;
	}

	.startup-title {
		font-size: 1.5rem;
		font-weight: 600;
		color: var(--color-foreground, #e4e4e7);
		margin: 0 0 0.5rem;
		letter-spacing: -0.01em;
	}

	.startup-status {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		font-size: 0.88rem;
		font-weight: 450;
		min-height: 1.5rem;
	}

	.startup-status--loading {
		color: var(--color-muted-foreground, #a1a1aa);
	}

	.startup-status--ready {
		color: #22c55e;
	}

	.startup-status--error {
		color: #f87171;
	}

	.startup-detail {
		font-size: 0.72rem;
		color: var(--color-muted-foreground, #52525b);
		margin: -0.4rem 0 0;
		letter-spacing: 0.01em;
	}

	.startup-retry {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.5rem 1.2rem;
		border-radius: 8px;
		border: 1px solid color-mix(in srgb, #ef4444 30%, transparent);
		background: color-mix(in srgb, #ef4444 10%, transparent);
		color: #fca5a5;
		font-size: 0.78rem;
		cursor: pointer;
		transition: background 0.15s;
	}

	.startup-retry:hover {
		background: color-mix(in srgb, #ef4444 18%, transparent);
	}

	.startup-hint {
		font-size: 0.7rem;
		color: var(--color-muted-foreground, #52525b);
		margin: 0;
		animation: fade-in 0.3s ease;
	}

	:global(.spin) {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	@keyframes fade-in {
		from { opacity: 0; }
		to { opacity: 1; }
	}
</style>
