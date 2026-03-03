<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from '$lib/components/ui/button';
	import { Cpu, RefreshCw, Settings2, X } from '@lucide/svelte';
	import { inferenceStore } from '$lib/stores/inference.svelte';
	import ModelSelector from '$lib/components/ModelSelector.svelte';
	import InferenceModeSelector from '$lib/components/InferenceModeSelector.svelte';

	interface Props {
		visible: boolean;
		onClose?: () => void;
	}

	let { visible, onClose }: Props = $props();
	let refreshing = $state(false);
	const status = $derived(inferenceStore.status);

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

	async function refreshPanel() {
		refreshing = true;
		try {
			await inferenceStore.listModels();
			await inferenceStore.syncStatus();
		} finally {
			refreshing = false;
		}
	}

	function formatBackendLabel(backend: string | null): string {
		if (!backend) {
			return 'Unknown';
		}
		if (backend === 'directml') {
			return 'DirectML';
		}
		if (backend === 'cpu') {
			return 'CPU';
		}
		return backend.toUpperCase();
	}

	function formatReason(value: string | null): string {
		if (!value) {
			return 'n/a';
		}
		return value.replaceAll('_', ' ');
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
					onclick={refreshPanel}
					disabled={refreshing}
					aria-label="Refresh model info"
					title="Refresh model info"
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
				<ModelSelector />
			</div>

			<div class="model-info-panel__control-block">
				<div class="model-info-panel__control-label">Inference Mode</div>
				<InferenceModeSelector />
			</div>

			<div class="model-info-panel__details">
				<div class="model-info-panel__details-title">Active Runtime</div>
				<div class="model-info-panel__details-grid">
					<span>Loaded Model</span>
					<span>{status.currentModel ?? 'none'}</span>
					<span>Backend</span>
					<span>{formatBackendLabel(status.activeBackend)}</span>
					<span>Runtime Engine</span>
					<span>{status.runtimeEngine ?? 'n/a'}</span>
					<span>Mode</span>
					<span>{status.runtimeMode.toUpperCase()}</span>
					<span>Device</span>
					<span>{status.selectedDeviceName ?? 'n/a'}</span>
					<span>Selection Reason</span>
					<span>{formatReason(status.selectionReason)}</span>
					<span>Gate State</span>
					<span>{formatReason(status.dmlGateState)}</span>
					<span>Model Path</span>
					<span title={status.activeModelPath ?? ''}>{status.activeModelPath ?? 'n/a'}</span>
				</div>
			</div>

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

	.model-info-panel__details {
		border: 1px solid var(--outline-soft);
		border-radius: var(--radius-lg);
		padding: 0.72rem;
		background: color-mix(in srgb, var(--surface-widget) 96%, black);
	}

	.model-info-panel__details-title {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		font-size: 0.75rem;
		font-weight: 650;
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
