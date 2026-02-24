<script lang="ts">
	import { hardwareStore } from '$lib/stores/hardware.svelte';
	import { Cpu, Gpu, Zap } from '@lucide/svelte';

	interface Props {
		onclick?: () => void;
		active?: boolean;
	}

	let { onclick, active = false }: Props = $props();

	const primaryGpu = $derived(hardwareStore.getPrimaryGpu());
	const hasNpu = $derived(hardwareStore.info?.npu?.detected ?? false);
</script>

<button
	onclick={onclick}
	class={`hardware-indicator ${active ? 'hardware-indicator--active' : ''}`}
	aria-label="View hardware information"
>
	{#if primaryGpu}
		<Gpu class="h-3.5 w-3.5" />
		<span class="hardware-indicator__label">{primaryGpu.name}</span>
		{#if hasNpu}
			<Zap class="h-3.5 w-3.5 hardware-indicator__npu" />
		{/if}
	{:else if hardwareStore.info}
		<Cpu class="h-3.5 w-3.5" />
		<span class="hardware-indicator__label">CPU Only</span>
	{:else}
		<Cpu class="h-3.5 w-3.5 hardware-indicator__loading" />
		<span class="hardware-indicator__label hardware-indicator__label--muted">Detecting...</span>
	{/if}
</button>

<style>
	.hardware-indicator {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		border: 1px solid var(--outline-soft);
		border-radius: var(--radius-lg);
		padding: 0.45rem 0.7rem;
		background: color-mix(in srgb, var(--surface-widget) 95%, black);
		font-size: 0.74rem;
		font-weight: 620;
		color: var(--color-foreground);
		cursor: pointer;
		box-shadow: var(--glow-subtle);
		transition:
			background var(--motion-fast),
			border-color var(--motion-fast),
			transform var(--motion-fast);
	}

	.hardware-indicator:hover {
		transform: translateY(-0.5px);
		border-color: var(--outline-strong);
		background: var(--surface-active);
	}

	.hardware-indicator--active {
		border-color: var(--outline-strong);
		background: var(--brand-soft-strong);
	}

	.hardware-indicator__label {
		max-width: 11rem;
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}

	.hardware-indicator__label--muted {
		color: var(--color-muted-foreground);
	}

	.hardware-indicator__loading {
		color: var(--color-muted-foreground);
		animation: pulse-icon 1.4s ease-in-out infinite;
	}

	.hardware-indicator__npu {
		color: var(--color-warning);
	}

	@keyframes pulse-icon {
		0%,
		100% {
			opacity: 0.45;
		}
		50% {
			opacity: 1;
		}
	}
</style>
