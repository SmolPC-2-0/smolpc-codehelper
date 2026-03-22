<script lang="ts">
	import { Gauge, Loader2 } from '@lucide/svelte';
	import { inferenceStore } from '$lib/stores/inference.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import type { CheckModelResponse, InferenceRuntimeMode } from '$lib/types/inference';

	interface Props {
		busy?: boolean;
	}

	interface ModeOption {
		value: InferenceRuntimeMode;
		label: string;
		disabled: boolean;
		title: string;
	}

	let { busy = false }: Props = $props();
	let isSwitching = $state(false);
	let isCheckingReadiness = $state(false);
	let laneReadiness = $state<CheckModelResponse | null>(null);
	// Not reactive; used only to discard stale async readiness responses.
	let readinessRequestNonce = 0;

	const effectiveModelId = $derived(
		inferenceStore.currentModel ?? settingsStore.selectedModel ?? null
	);

	function humanizeReason(reason: string | null | undefined): string {
		if (!reason) return 'unavailable';
		return reason.replaceAll('_', ' ');
	}

	function normalizeLaneReason(kind: 'dml' | 'npu', reason: string | null | undefined): string {
		const humanized = humanizeReason(reason);
		const normalized = humanized.toLowerCase();

		if (
			kind === 'npu' &&
			(normalized.includes('bundle') ||
				normalized.includes('dll') ||
				(normalized.includes('runtime') &&
					(normalized.includes('missing') || normalized.includes('not installed'))))
		) {
			return 'Intel NPU runtime not installed';
		}

		return humanized;
	}

	function fallbackLaneReason(kind: 'dml' | 'npu'): string | null {
		const lane =
			kind === 'dml'
				? inferenceStore.backendStatus?.lanes.directml
				: inferenceStore.backendStatus?.lanes.openvino_npu;
		const reason =
			lane?.last_failure_message ?? lane?.last_failure_class ?? lane?.preflight_state ?? null;

		if (!reason || reason === 'not_started' || reason === 'ready') {
			return null;
		}

		return normalizeLaneReason(kind, reason);
	}

	function laneAvailability(kind: 'dml' | 'npu'): boolean | null {
		if (!laneReadiness) {
			return null;
		}

		return kind === 'dml'
			? laneReadiness.lanes.directml.ready
			: laneReadiness.lanes.openvino_npu.ready;
	}

	function laneStatusReason(kind: 'dml' | 'npu'): string | null {
		if (!laneReadiness) {
			return fallbackLaneReason(kind);
		}

		const lane = kind === 'dml' ? laneReadiness.lanes.directml : laneReadiness.lanes.openvino_npu;
		if (lane.ready) {
			return null;
		}

		return normalizeLaneReason(kind, lane.reason);
	}

	const dmlDisabled = $derived(laneAvailability('dml') === false);
	const npuDisabled = $derived(laneAvailability('npu') === false);
	const dmlStatusReason = $derived(laneStatusReason('dml'));
	const npuStatusReason = $derived(laneStatusReason('npu'));

	const modeOptions = $derived.by<ModeOption[]>(() => [
		{
			value: 'auto',
			label: 'Mode: Auto',
			disabled: false,
			title: 'Automatic backend selection'
		},
		{
			value: 'dml',
			label: `Mode: DirectML${dmlDisabled ? ' (unavailable)' : ''}`,
			disabled: dmlDisabled,
			title: dmlDisabled
				? `DirectML unavailable: ${dmlStatusReason ?? 'unavailable'}`
				: 'Force DirectML GPU acceleration'
		},
		{
			value: 'cpu',
			label: 'Mode: CPU',
			disabled: false,
			title: 'Force CPU inference'
		},
		{
			value: 'npu',
			label: `Mode: NPU${npuDisabled ? ' (unavailable)' : ''}`,
			disabled: npuDisabled,
			title: npuDisabled
				? `NPU unavailable: ${npuStatusReason ?? 'unavailable'}`
				: 'Force Intel NPU via OpenVINO'
		}
	]);

	$effect(() => {
		const modelId = effectiveModelId;
		const requestNonce = ++readinessRequestNonce;

		if (!modelId) {
			laneReadiness = null;
			isCheckingReadiness = false;
			return;
		}

		laneReadiness = null;
		isCheckingReadiness = true;

		void inferenceStore
			.checkModelReadiness(modelId)
			.then((readiness) => {
				if (requestNonce !== readinessRequestNonce) {
					return;
				}
				laneReadiness = readiness;
			})
			.catch((error) => {
				if (requestNonce !== readinessRequestNonce) {
					return;
				}
				console.error('Failed to check model readiness:', error);
				laneReadiness = null;
			})
			.finally(() => {
				if (requestNonce === readinessRequestNonce) {
					isCheckingReadiness = false;
				}
			});
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
			const switched = await inferenceStore.setRuntimeMode(mode, reloadModelId);
			if (switched) {
				settingsStore.setRuntimeModePreference(mode);
			}
		} finally {
			isSwitching = false;
		}
	}
</script>

<div class="mode-selector">
	{#if isSwitching || isCheckingReadiness}
		<Loader2 class="mode-selector__icon mode-selector__icon--loading" />
	{:else}
		<Gauge class="mode-selector__icon" />
	{/if}
	<select
		value={inferenceStore.runtimeMode}
		onchange={handleModeChange}
		disabled={isSwitching || busy}
		class="mode-selector__control"
		aria-label="Select inference runtime mode"
		title="Switch runtime mode and restart the shared engine host"
	>
		{#each modeOptions as option (option.value)}
			<option value={option.value} disabled={option.disabled} title={option.title}>
				{option.label}
			</option>
		{/each}
	</select>
</div>

{#if dmlStatusReason || npuStatusReason}
	<div class="mode-selector__status">
		{#if dmlStatusReason}
			<span>DirectML: {dmlStatusReason}</span>
		{/if}
		{#if npuStatusReason}
			<span>NPU: {npuStatusReason}</span>
		{/if}
	</div>
{/if}

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

	.mode-selector__status {
		display: grid;
		gap: 0.25rem;
		font-size: 0.68rem;
		color: var(--color-muted-foreground);
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
