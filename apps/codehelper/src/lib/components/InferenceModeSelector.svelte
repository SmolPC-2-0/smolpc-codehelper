<script lang="ts">
	import { Gauge, Loader2 } from '@lucide/svelte';
	import { inferenceStore } from '$lib/stores/inference.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import type { InferenceRuntimeMode, BackendLaneStatus } from '$lib/types/inference';

	let isSwitching = $state(false);

	interface ModeOption {
		value: InferenceRuntimeMode;
		label: string;
		disabled: boolean;
		title: string;
	}

	function isLaneFailed(lane: BackendLaneStatus | undefined): boolean {
		if (!lane) return false;
		return !!lane.last_failure_class;
	}

	function laneFailureReason(lane: BackendLaneStatus | undefined): string {
		if (!lane?.last_failure_message) return 'unavailable';
		return lane.last_failure_message;
	}

	const modeOptions = $derived.by<ModeOption[]>(() => {
		const bs = inferenceStore.backendStatus;
		const npuLane = bs?.lanes?.openvino_npu;
		const dmlLane = bs?.lanes?.directml;

		const npuFailed = isLaneFailed(npuLane);
		const dmlFailed = isLaneFailed(dmlLane);

		return [
			{ value: 'auto' as const, label: 'Mode: Auto', disabled: false, title: 'Automatic backend selection' },
			{
				value: 'dml' as const,
				label: `Mode: DirectML${dmlFailed ? ' (unavailable)' : ''}`,
				disabled: dmlFailed,
				title: dmlFailed ? `DirectML unavailable: ${laneFailureReason(dmlLane)}` : 'Force DirectML GPU acceleration'
			},
			{ value: 'cpu' as const, label: 'Mode: CPU', disabled: false, title: 'Force CPU inference' },
			{
				value: 'npu' as const,
				label: `Mode: NPU${npuFailed ? ' (unavailable)' : ''}`,
				disabled: npuFailed,
				title: npuFailed ? `NPU unavailable: ${laneFailureReason(npuLane)}` : 'Force Intel NPU via OpenVINO'
			}
		];
	});

	async function handleModeChange(event: Event) {
		const target = event.target as HTMLSelectElement;
		const mode = target.value as InferenceRuntimeMode;

		if (mode === inferenceStore.runtimeMode) {
			return;
		}

		isSwitching = true;
		try {
			const reloadModelId = inferenceStore.currentModel ?? settingsStore.selectedModel ?? null;
			await inferenceStore.setRuntimeMode(mode, reloadModelId);
		} finally {
			isSwitching = false;
		}
	}
</script>

<div class="mode-selector">
	{#if isSwitching}
		<Loader2 class="mode-selector__icon mode-selector__icon--loading" />
	{:else}
		<Gauge class="mode-selector__icon" />
	{/if}
	<select
		value={inferenceStore.runtimeMode}
		onchange={handleModeChange}
		disabled={isSwitching || inferenceStore.isGenerating}
		class="mode-selector__control"
		aria-label="Select inference runtime mode"
		title="Switch runtime mode and restart the shared engine host"
	>
		{#each modeOptions as option}
			<option value={option.value} disabled={option.disabled} title={option.title}>
				{option.label}
			</option>
		{/each}
	</select>
</div>

<style>
	.mode-selector {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		min-width: 11.5rem;
		padding: 0.45rem 0.68rem;
		border-radius: var(--radius-xl);
		border: 1px solid var(--outline-soft);
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
		box-shadow: var(--glow-subtle);
	}

	:global(.mode-selector__icon) {
		width: 0.95rem;
		height: 0.95rem;
		color: var(--color-muted-foreground);
		flex-shrink: 0;
	}

	:global(.mode-selector__icon--loading) {
		animation: spin 1s linear infinite;
	}

	.mode-selector__control {
		flex: 1;
		font-size: 0.78rem;
		background: transparent;
		color: var(--color-foreground);
		line-height: 1.25;
		outline: none;
		border: none;
		appearance: none;
		padding-right: 0.4rem;
		color-scheme: light dark;
	}

	.mode-selector__control option {
		background: var(--surface-floating);
		color: var(--color-foreground);
	}

	.mode-selector__control:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	@media (max-width: 768px) {
		.mode-selector {
			min-width: 10.5rem;
			width: 100%;
		}
	}
</style>
