<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from '$lib/components/ui/button';
	import { Cpu, RefreshCw, Settings2, X } from '@lucide/svelte';
	import { inferenceStore } from '$lib/stores/inference.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import ModelSelector from '$lib/components/ModelSelector.svelte';
	import InferenceModeSelector from '$lib/components/InferenceModeSelector.svelte';
	import type {
		BackendSelectionState,
		EngineReadinessState,
		InferenceBackend
	} from '$lib/types/inference';

	interface Props {
		visible: boolean;
		busy?: boolean;
		onClose?: () => void;
	}

	let { visible, busy = false, onClose }: Props = $props();
	let refreshing = $state(false);
	const status = $derived(inferenceStore.status);
	const reloadModelId = $derived(
		inferenceStore.currentModel ?? settingsStore.selectedModel ?? null
	);
	const controlsBusy = $derived(busy || refreshing);

	onMount(() => {
		function handleKeydown(event: KeyboardEvent) {
			if (event.key === 'Escape' && visible) {
				closePanel();
			}
		}

		window.addEventListener('keydown', handleKeydown);
		return () => window.removeEventListener('keydown', handleKeydown);
	});

	$effect(() => {
		if (!visible) {
			return;
		}
		void inferenceStore.listModels();
		void inferenceStore.syncStatus();
	});

	function closePanel() {
		onClose?.();
	}

	async function reloadCurrentModel() {
		refreshing = true;
		try {
			if (reloadModelId) {
				await inferenceStore.loadModel(reloadModelId);
			}
			await inferenceStore.listModels();
		} finally {
			refreshing = false;
		}
	}

	async function retryStartup() {
		refreshing = true;
		try {
			await inferenceStore.retryStartup(settingsStore.selectedModel ?? null);
			await inferenceStore.syncStatus();
		} finally {
			refreshing = false;
		}
	}

	function friendlyReadinessLabel(state: string): string {
		if (state === 'ready') return 'Ready';
		if (state === 'failed') return 'Failed';
		if (state === 'loading_model') return 'Loading model...';
		if (state === 'probing') return 'Detecting hardware...';
		if (state === 'resolving_assets') return 'Resolving model files...';
		if (state === 'starting') return 'Starting...';
		return 'Idle';
	}

	function friendlyBackendLabel(
		backend: InferenceBackend | null,
		selectionState: BackendSelectionState | null = null
	): string {
		let label = 'Unknown';

		if (backend === 'directml') {
			label = 'DirectML GPU';
		} else if (backend === 'cpu') {
			label = 'CPU';
		} else if (backend === 'openvino_npu') {
			label = 'OpenVINO NPU';
		}

		if (selectionState === 'fallback' && label !== 'Unknown') {
			return `${label} (fallback)`;
		}

		return label;
	}

	function formatModeLabel(mode: string): string {
		return mode.toUpperCase();
	}

	function formatReason(value: string | null): string {
		if (!value) {
			return 'n/a';
		}
		return value.replaceAll('_', ' ');
	}

	function formatReadinessState(value: EngineReadinessState | 'unknown'): string {
		return formatReason(value);
	}
</script>

{#if visible}
	<div class="model-info-panel">
		<div class="model-info-panel__header">
			<h3 class="model-info-panel__title">
				<Settings2 class="h-4.5 w-4.5" />
				Model & Runtime
			</h3>
			<div class="model-info-panel__actions">
				<Button
					variant="ghost"
					size="icon"
					onclick={reloadCurrentModel}
					disabled={controlsBusy || !reloadModelId}
					aria-label="Reload model"
					title="Reload model"
				>
					<RefreshCw class={`h-4 w-4 ${refreshing ? 'animate-spin' : ''}`} />
				</Button>
				<Button
					variant="ghost"
					size="icon"
					onclick={closePanel}
					aria-label="Close model info panel"
				>
					<X class="h-4 w-4" />
				</Button>
			</div>
		</div>

		<div class="model-info-panel__content">
			<div class="model-info-panel__control-block">
				<div class="model-info-panel__control-label">Model</div>
				<ModelSelector busy={controlsBusy} />
			</div>

			<div class="model-info-panel__control-block">
				<div class="model-info-panel__control-label">Inference Mode</div>
				<InferenceModeSelector busy={controlsBusy} />
				<div class="model-info-panel__helper">
					Switch runtime mode and reload the current model into the selected backend.
				</div>
			</div>

			<section class="model-info-panel__summary">
				<div class="model-info-panel__details-title">Runtime</div>
				<div class="model-info-panel__details-grid">
					<span>Status</span>
					<span>{friendlyReadinessLabel(status.readinessState)}</span>
					<span>Model</span>
					<span>{status.currentModel ?? 'None loaded'}</span>
					<span>Backend</span>
					<span>{friendlyBackendLabel(status.activeBackend, status.selectionState)}</span>
					{#if status.selectedDeviceName}
						<span>Device</span>
						<span>{status.selectedDeviceName}</span>
					{/if}
					<span>Mode</span>
					<span>{formatModeLabel(status.runtimeMode)}</span>
				</div>
			</section>

			<details class="model-info-panel__details">
				<summary class="model-info-panel__details-summary">Engine details</summary>
				<div class="model-info-panel__details-grid">
					<span>Readiness State</span>
					<span>{formatReadinessState(status.readinessState)}</span>
					<span>Attempt ID</span>
					<span title={status.readiness?.attempt_id ?? ''}
						>{status.readiness?.attempt_id ?? 'n/a'}</span
					>
					<span>State Since</span>
					<span title={status.readiness?.state_since ?? ''}
						>{status.readiness?.state_since ?? 'n/a'}</span
					>
					<span>Loaded Model</span>
					<span>{status.currentModel ?? 'none'}</span>
					<span>Backend</span>
					<span>{friendlyBackendLabel(status.activeBackend, status.selectionState)}</span>
					<span>Runtime Engine</span>
					<span>{status.runtimeEngine ?? 'n/a'}</span>
					<span>Mode</span>
					<span>{formatModeLabel(status.runtimeMode)}</span>
					<span>Device</span>
					<span>{status.selectedDeviceName ?? 'n/a'}</span>
					<span>Selection Reason</span>
					<span>{formatReason(status.selectionReason)}</span>
					<span>Persistence</span>
					<span>{formatReason(status.decisionPersistenceState)}</span>
					<span>DirectML Lane</span>
					<span>{formatReason(status.directmlPreflightState ?? status.directmlFailureClass)}</span>
					<span>NPU Lane</span>
					<span
						>{formatReason(
							inferenceStore.backendStatus?.lanes?.openvino_npu?.last_failure_class ??
								inferenceStore.backendStatus?.lanes?.openvino_npu?.preflight_state ??
								null
						)}</span
					>
					<span>Model Path</span>
					<span title={status.activeModelPath ?? ''}>{status.activeModelPath ?? 'n/a'}</span>
				</div>
			</details>

			{#if status.readinessState === 'failed' && status.startupRetryable}
				<Button variant="outline" onclick={retryStartup} disabled={controlsBusy}>
					Retry startup
				</Button>
			{/if}

			{#if status.error}
				<div class="model-info-panel__error">
					<Cpu class="h-3.5 w-3.5" />
					<span>{status.error}</span>
				</div>
			{/if}
		</div>
	</div>
{/if}

<style>
	.model-info-panel {
		position: fixed;
		right: 1rem;
		top: 5.75rem;
		z-index: 55;
		width: min(28rem, calc(100vw - 1.6rem));
		max-height: min(78vh, 42rem);
		overflow-y: auto;
		border-radius: calc(var(--radius-xl) + 6px);
		border: 1px solid var(--outline-soft);
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--surface-floating) 96%, black),
				color-mix(in srgb, var(--surface-subtle) 97%, black)
			),
			var(--surface-floating);
		box-shadow: var(--shadow-strong);
		backdrop-filter: blur(14px);
	}

	.model-info-panel__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.85rem 0.9rem;
		border-bottom: 1px solid var(--outline-soft);
	}

	.model-info-panel__title {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 0.95rem;
		font-weight: 700;
	}

	.model-info-panel__actions {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
	}

	.model-info-panel__content {
		display: grid;
		gap: 0.9rem;
		padding: 0.9rem;
	}

	.model-info-panel__control-block {
		display: grid;
		gap: 0.4rem;
	}

	.model-info-panel__control-label {
		font-size: 0.72rem;
		font-weight: 650;
		color: var(--color-muted-foreground);
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}

	.model-info-panel__helper {
		font-size: 0.68rem;
		color: var(--color-muted-foreground);
	}

	.model-info-panel__summary,
	.model-info-panel__details {
		border: 1px solid var(--outline-soft);
		border-radius: var(--radius-lg);
		padding: 0.72rem;
		background: color-mix(in srgb, var(--surface-widget) 96%, black);
	}

	.model-info-panel__details-summary {
		cursor: pointer;
		font-size: 0.75rem;
		font-weight: 650;
		list-style: none;
	}

	.model-info-panel__details-summary::-webkit-details-marker {
		display: none;
	}

	.model-info-panel__details-title {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		font-size: 0.75rem;
		font-weight: 650;
		margin-bottom: 0.5rem;
	}

	.model-info-panel__details[open] .model-info-panel__details-summary {
		margin-bottom: 0.5rem;
	}

	.model-info-panel__details-grid {
		display: grid;
		grid-template-columns: 7.6rem minmax(0, 1fr);
		row-gap: 0.35rem;
		column-gap: 0.5rem;
		font-size: 0.72rem;
	}

	.model-info-panel__details-grid span:nth-child(odd) {
		color: var(--color-muted-foreground);
	}

	.model-info-panel__details-grid span:nth-child(even) {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.model-info-panel__error {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		border: 1px solid color-mix(in srgb, var(--color-destructive) 45%, transparent);
		border-radius: var(--radius-md);
		padding: 0.5rem 0.6rem;
		font-size: 0.72rem;
		color: var(--color-destructive);
		background: color-mix(in srgb, var(--color-destructive) 8%, transparent);
	}

	:global(.model-info-panel .model-selector),
	:global(.model-info-panel .mode-selector) {
		width: 100%;
		min-width: 0;
	}

	:global(.model-info-panel .mode-selector__status) {
		margin-top: 0.4rem;
	}

	:global(.model-info-panel .model-selector__control),
	:global(.model-info-panel .mode-selector__control) {
		width: 100%;
	}

	@media (max-width: 900px) {
		.model-info-panel {
			right: 0.8rem;
			left: 0.8rem;
			top: auto;
			bottom: 0.8rem;
			width: auto;
			max-height: min(76vh, 40rem);
		}
	}
</style>
