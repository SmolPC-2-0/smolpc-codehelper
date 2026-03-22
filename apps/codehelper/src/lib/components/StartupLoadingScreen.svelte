<script lang="ts">
	import { Check, Loader2, Circle, AlertCircle, RefreshCw } from '@lucide/svelte';
	import type { EngineReadinessDto } from '$lib/types/inference';

	interface Props {
		readiness: EngineReadinessDto | null;
		onRetry: () => void;
		visible: boolean;
	}

	let { readiness, onRetry, visible }: Props = $props();

	type StepStatus = 'pending' | 'active' | 'complete' | 'error';

	const STEPS = [
		{ id: 'starting', label: 'Starting engine' },
		{ id: 'probing', label: 'Detecting hardware' },
		{ id: 'resolving_assets', label: 'Checking model files' },
		{ id: 'loading_model', label: 'Loading AI model' },
		{ id: 'ready', label: 'Ready' }
	] as const;

	const STATE_ORDER = ['idle', 'starting', 'probing', 'resolving_assets', 'loading_model', 'ready'];

	const stepStatuses = $derived.by((): StepStatus[] => {
		const state = readiness?.state ?? 'idle';

		if (state === 'failed') {
			// Mark all steps up to the failure point as error
			const failIndex = STATE_ORDER.indexOf('loading_model'); // assume failed during load
			return STEPS.map((_, i) => {
				const stepState = STEPS[i].id === 'starting' ? 'starting' : STEPS[i].id;
				const stepIndex = STATE_ORDER.indexOf(stepState);
				if (stepIndex < failIndex) return 'complete' as StepStatus;
				if (stepIndex === failIndex) return 'error' as StepStatus;
				return 'pending' as StepStatus;
			});
		}

		const currentIndex = STATE_ORDER.indexOf(state);
		// idle maps to starting
		const effectiveIndex = state === 'idle' ? STATE_ORDER.indexOf('starting') : currentIndex;

		return STEPS.map((step) => {
			const stepState = step.id === 'starting' ? 'starting' : step.id;
			const stepIndex = STATE_ORDER.indexOf(stepState);
			if (stepIndex < effectiveIndex) return 'complete' as StepStatus;
			if (stepIndex === effectiveIndex) return 'active' as StepStatus;
			return 'pending' as StepStatus;
		});
	});

	const isFailed = $derived(readiness?.state === 'failed');
	const errorMessage = $derived(
		readiness?.error_message ?? 'Something went wrong during startup.'
	);
	const showSlowHint = $derived.by(() => {
		if (!readiness?.state_since) return false;
		const since = new Date(readiness.state_since).getTime();
		return Date.now() - since > 15000;
	});
</script>

{#if visible}
	<div class="startup-overlay" class:fading={readiness?.state === 'ready'}>
		<div class="startup-content">
			<h1 class="startup-title">SmolPC Code Helper</h1>
			<p class="startup-subtitle">Preparing your assistant</p>

			<div class="startup-steps">
				{#each STEPS as step, index (step.id)}
					{@const status = stepStatuses[index]}
					<div class="step" class:step--complete={status === 'complete'} class:step--active={status === 'active'} class:step--error={status === 'error'} class:step--pending={status === 'pending'}>
						<span class="step__icon">
							{#if status === 'complete'}
								<Check size={16} />
							{:else if status === 'active'}
								<Loader2 size={16} class="spin" />
							{:else if status === 'error'}
								<AlertCircle size={16} />
							{:else}
								<Circle size={16} />
							{/if}
						</span>
						<span class="step__label">{step.label}</span>
					</div>
				{/each}
			</div>

			{#if isFailed}
				<div class="startup-error">
					<p class="startup-error__message">{errorMessage}</p>
					<button class="startup-error__retry" onclick={onRetry}>
						<RefreshCw size={14} />
						Try again
					</button>
				</div>
			{/if}

			{#if showSlowHint && !isFailed}
				<p class="startup-hint">This may take a moment on first launch</p>
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
		gap: 1.5rem;
		max-width: 320px;
		padding: 2rem;
	}

	.startup-title {
		font-size: 1.4rem;
		font-weight: 600;
		color: var(--color-foreground, #e4e4e7);
		margin: 0;
	}

	.startup-subtitle {
		font-size: 0.82rem;
		color: var(--color-muted-foreground, #71717a);
		margin: -0.5rem 0 0.5rem;
	}

	.startup-steps {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		width: 100%;
	}

	.step {
		display: flex;
		align-items: center;
		gap: 0.65rem;
		padding: 0.45rem 0.6rem;
		border-radius: 8px;
		font-size: 0.82rem;
		transition: all 0.2s ease;
	}

	.step--pending {
		color: var(--color-muted-foreground, #52525b);
	}

	.step--active {
		color: var(--color-foreground, #e4e4e7);
		background: color-mix(in srgb, var(--color-foreground, #e4e4e7) 5%, transparent);
	}

	.step--complete {
		color: #22c55e;
	}

	.step--error {
		color: #ef4444;
	}

	.step__icon {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
	}

	:global(.spin) {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	.step__label {
		font-weight: 450;
	}

	.startup-error {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.75rem;
		padding: 1rem;
		border-radius: 10px;
		background: color-mix(in srgb, #ef4444 8%, transparent);
		border: 1px solid color-mix(in srgb, #ef4444 20%, transparent);
		width: 100%;
	}

	.startup-error__message {
		font-size: 0.78rem;
		color: #fca5a5;
		margin: 0;
		text-align: center;
		line-height: 1.45;
	}

	.startup-error__retry {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.5rem 1rem;
		border-radius: 8px;
		border: 1px solid color-mix(in srgb, #ef4444 30%, transparent);
		background: color-mix(in srgb, #ef4444 12%, transparent);
		color: #fca5a5;
		font-size: 0.78rem;
		cursor: pointer;
		transition: background 0.15s;
	}

	.startup-error__retry:hover {
		background: color-mix(in srgb, #ef4444 20%, transparent);
	}

	.startup-hint {
		font-size: 0.72rem;
		color: var(--color-muted-foreground, #52525b);
		margin: 0;
		animation: fade-in 0.3s ease;
	}

	@keyframes fade-in {
		from { opacity: 0; }
		to { opacity: 1; }
	}
</style>
