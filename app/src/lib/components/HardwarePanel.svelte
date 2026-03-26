<script lang="ts">
	import { onMount } from 'svelte';
	import { hardwareStore } from '$lib/stores/hardware.svelte';
	import { Cpu, Gpu, Zap, RefreshCw, X, MemoryStick, HardDrive } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';

	interface Props {
		visible: boolean;
		onClose?: () => void;
	}

	let { visible, onClose }: Props = $props();

	onMount(() => {
		// Load cached hardware info on mount
		hardwareStore.getCached();

		// Handle ESC key to close panel
		function handleKeydown(event: KeyboardEvent) {
			if (event.key === 'Escape' && visible) {
				closePanel();
			}
		}

		window.addEventListener('keydown', handleKeydown);

		return () => {
			window.removeEventListener('keydown', handleKeydown);
		};
	});

	function closePanel() {
		onClose?.();
	}

	async function refreshHardware() {
		await hardwareStore.detect();
	}

	function formatFrequency(mhz: number | undefined): string {
		if (!mhz) return 'N/A';
		if (mhz >= 1000) {
			return `${(mhz / 1000).toFixed(2)} GHz`;
		}
		return `${mhz} MHz`;
	}

	function formatCache(kb: number | undefined): string {
		if (!kb) return 'N/A';
		if (kb >= 1024) {
			return `${(kb / 1024).toFixed(1)} MB`;
		}
		return `${kb} KB`;
	}
</script>

{#if visible}
	<div class="hardware-panel">
		<div class="mb-4 flex items-center justify-between">
			<h3 class="flex items-center gap-2 text-lg font-semibold">
				<Cpu class="h-5 w-5" />
				Hardware Information
			</h3>
			<div class="flex items-center gap-2">
				<Button
					variant="ghost"
					size="icon"
					onclick={refreshHardware}
					disabled={hardwareStore.loading}
					aria-label="Refresh hardware detection"
				>
					<RefreshCw class={`h-4 w-4 ${hardwareStore.loading ? 'animate-spin' : ''}`} />
				</Button>
				<button
					onclick={closePanel}
					class="text-muted-foreground hover:text-foreground"
					aria-label="Close hardware panel"
				>
					<X class="h-5 w-5" />
				</button>
			</div>
		</div>

		{#if hardwareStore.loading}
			<div class="flex items-center justify-center py-8">
				<div class="text-primary animate-spin">
					<RefreshCw class="h-8 w-8" />
				</div>
			</div>
		{:else if hardwareStore.error}
			<div class="rounded-md border border-red-500 bg-red-50 p-4 dark:bg-red-950">
				<p class="text-sm text-red-600 dark:text-red-400">
					Error detecting hardware: {hardwareStore.error}
				</p>
			</div>
		{:else if hardwareStore.info}
			<div class="space-y-4">
				<!-- CPU Information -->
				<div class="border-border rounded-lg border p-4">
					<div class="mb-3 flex items-center gap-2">
						<Cpu class="text-primary h-4 w-4" />
						<h4 class="font-semibold">CPU</h4>
					</div>
					<div class="space-y-2 text-sm">
						<div class="grid grid-cols-3 gap-2">
							<span class="text-muted-foreground">Brand:</span>
							<span class="col-span-2 font-medium">{hardwareStore.info.cpu.brand}</span>
						</div>
						<div class="grid grid-cols-3 gap-2">
							<span class="text-muted-foreground">Vendor:</span>
							<span class="col-span-2">{hardwareStore.info.cpu.vendor}</span>
						</div>
						<div class="grid grid-cols-3 gap-2">
							<span class="text-muted-foreground">Architecture:</span>
							<span class="col-span-2">{hardwareStore.info.cpu.architecture}</span>
						</div>
						<div class="grid grid-cols-3 gap-2">
							<span class="text-muted-foreground">Cores:</span>
							<span class="col-span-2"
								>{hardwareStore.info.cpu.cores_physical} physical / {hardwareStore.info.cpu
									.cores_logical} logical</span
							>
						</div>
						<div class="grid grid-cols-3 gap-2">
							<span class="text-muted-foreground">Frequency:</span>
							<span class="col-span-2">{formatFrequency(hardwareStore.info.cpu.frequency_mhz)}</span
							>
						</div>
						<div class="grid grid-cols-3 gap-2">
							<span class="text-muted-foreground">Cache:</span>
							<span class="col-span-2">
								L1: {formatCache(hardwareStore.info.cpu.cache_l1_kb)}, L2: {formatCache(
									hardwareStore.info.cpu.cache_l2_kb
								)}, L3: {formatCache(hardwareStore.info.cpu.cache_l3_kb)}
							</span>
						</div>
						<div class="border-border mt-3 border-t pt-3">
							<div class="text-muted-foreground mb-1 text-xs">
								{hardwareStore.info.cpu.architecture.includes('aarch64')
									? 'SIMD Extensions:'
									: 'Instruction Sets:'}
							</div>
							<div class="flex flex-wrap gap-1.5">
								{#each hardwareStore.info.cpu.features.filter((feature) => {
									// Filter features based on architecture
									const isArm = hardwareStore.info!.cpu.architecture.includes('aarch64') || hardwareStore.info!.cpu.architecture.includes('arm');
									const armFeatures = ['NEON', 'SVE'];
									const x86Features = ['SSE42', 'AVX', 'AVX2', 'AVX512', 'FMA'];

									if (isArm) {
										return armFeatures.includes(feature.toUpperCase());
									} else {
										return x86Features.includes(feature.toUpperCase());
									}
								}) as feature (feature)}
									<span
										class="rounded-md border border-green-200 bg-green-100 px-2 py-0.5 text-xs text-green-700 dark:border-green-900 dark:bg-green-950 dark:text-green-400"
									>
										{feature}
									</span>
								{/each}
							</div>
						</div>
					</div>
				</div>

				<!-- Memory Information -->
				<div class="border-border rounded-lg border p-4">
					<div class="mb-3 flex items-center gap-2">
						<MemoryStick class="text-primary h-4 w-4" />
						<h4 class="font-semibold">Memory</h4>
					</div>
					<div class="space-y-2 text-sm">
						<div class="grid grid-cols-3 gap-2">
							<span class="text-muted-foreground">Total:</span>
							<span class="col-span-2 font-medium"
								>{hardwareStore.info.memory.total_gb.toFixed(1)} GB</span
							>
						</div>
						<div class="grid grid-cols-3 gap-2">
							<span class="text-muted-foreground">Available:</span>
							<span class="col-span-2">{hardwareStore.info.memory.available_gb.toFixed(1)} GB</span>
						</div>
					</div>
				</div>

				<!-- Storage Information -->
				<div class="border-border rounded-lg border p-4">
					<div class="mb-3 flex items-center gap-2">
						<HardDrive class="text-primary h-4 w-4" />
						<h4 class="font-semibold">Primary Storage</h4>
					</div>
					<div class="space-y-2 text-sm">
						<div class="grid grid-cols-3 gap-2">
							<span class="text-muted-foreground">Device:</span>
							<span class="col-span-2 font-medium">{hardwareStore.info.storage.device_name}</span>
						</div>
						<div class="grid grid-cols-3 gap-2">
							<span class="text-muted-foreground">Type:</span>
							<span class="col-span-2">{hardwareStore.info.storage.is_ssd ? 'SSD' : 'HDD'}</span>
						</div>
						<div class="grid grid-cols-3 gap-2">
							<span class="text-muted-foreground">Capacity:</span>
							<span class="col-span-2">{hardwareStore.info.storage.total_gb.toFixed(0)} GB</span>
						</div>
						<div class="grid grid-cols-3 gap-2">
							<span class="text-muted-foreground">Available:</span>
							<span class="col-span-2">{hardwareStore.info.storage.available_gb.toFixed(0)} GB</span
							>
						</div>
					</div>
				</div>

				<!-- GPU Information -->
				<div class="border-border rounded-lg border p-4">
					<div class="mb-3 flex items-center gap-2">
						<Gpu class="text-primary h-4 w-4" />
						<h4 class="font-semibold">
							GPU{hardwareStore.info.gpus.length > 1 ? 's' : ''}
							<span class="text-muted-foreground ml-1 text-xs">
								({hardwareStore.info.gpus.length})
							</span>
						</h4>
					</div>
					{#if hardwareStore.info.gpus.length === 0}
						<p class="text-muted-foreground text-sm">No GPUs detected</p>
					{:else}
						<div class="space-y-3">
							{#each hardwareStore.info.gpus as gpu, i (gpu.name)}
								<div class={`space-y-2 text-sm ${i > 0 ? 'border-border border-t pt-3' : ''}`}>
									<div class="grid grid-cols-3 gap-2">
										<span class="text-muted-foreground">Name:</span>
										<span class="col-span-2 font-medium">{gpu.name}</span>
									</div>
									<div class="grid grid-cols-3 gap-2">
										<span class="text-muted-foreground">Vendor:</span>
										<span class="col-span-2">{gpu.vendor}</span>
									</div>
									<div class="grid grid-cols-3 gap-2">
										<span class="text-muted-foreground">Type:</span>
										<span class="col-span-2">{gpu.device_type}</span>
									</div>
									<div class="grid grid-cols-3 gap-2">
										<span class="text-muted-foreground">Backend:</span>
										<span class="col-span-2">{gpu.backend}</span>
									</div>
									{#if gpu.vram_mb}
										<div class="grid grid-cols-3 gap-2">
											<span class="text-muted-foreground">VRAM:</span>
											<span class="col-span-2">{gpu.vram_mb} MB</span>
										</div>
									{/if}
									{#if gpu.cuda_compute_capability}
										<div class="grid grid-cols-3 gap-2">
											<span class="text-muted-foreground">CUDA:</span>
											<span class="col-span-2">{gpu.cuda_compute_capability}</span>
										</div>
									{/if}
								</div>
							{/each}
						</div>
					{/if}
				</div>

				<!-- NPU Information -->
				{#if hardwareStore.info.npu}
					<div class="border-border rounded-lg border p-4">
						<div class="mb-3 flex items-center gap-2">
							<Zap class="text-primary h-4 w-4" />
							<h4 class="font-semibold">NPU</h4>
							{#if hardwareStore.info.npu.confidence !== 'High'}
								<span
									class={`ml-auto rounded-md px-2 py-0.5 text-xs ${
										hardwareStore.info.npu.confidence === 'Medium'
											? 'bg-yellow-100 text-yellow-700 dark:bg-yellow-950 dark:text-yellow-400'
											: 'bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400'
									}`}
								>
									{hardwareStore.info.npu.confidence} Confidence
								</span>
							{/if}
						</div>
						<div class="space-y-2 text-sm">
							<div class="grid grid-cols-3 gap-2">
								<span class="text-muted-foreground">Status:</span>
								<span class="col-span-2 font-medium">
									{hardwareStore.info.npu.detected ? 'Detected' : 'Not Detected'}
								</span>
							</div>
							<div class="grid grid-cols-3 gap-2">
								<span class="text-muted-foreground">Identifier:</span>
								<span class="col-span-2 font-mono text-xs">{hardwareStore.info.npu.identifier}</span
								>
							</div>
							{#if hardwareStore.info.npu.driver_version}
								<div class="grid grid-cols-3 gap-2">
									<span class="text-muted-foreground">Driver:</span>
									<span class="col-span-2">{hardwareStore.info.npu.driver_version}</span>
								</div>
							{/if}
							<div class="grid grid-cols-3 gap-2">
								<span class="text-muted-foreground">Summary:</span>
								<span class="col-span-2">{hardwareStore.info.npu.details}</span>
							</div>
							<div class="grid grid-cols-3 gap-2">
								<span class="text-muted-foreground">Method:</span>
								<span class="col-span-2 text-xs">{hardwareStore.info.npu.method}</span>
							</div>
						</div>
					</div>
				{:else}
					<div class="border-border rounded-lg border border-dashed p-4">
						<div class="text-muted-foreground flex items-center gap-2">
							<Zap class="h-4 w-4" />
							<span class="text-sm">No NPU detected</span>
						</div>
					</div>
				{/if}

				<!-- Detection Info -->
				<div class="border-border text-muted-foreground border-t pt-2 text-center text-xs">
					Detected at {new Date(hardwareStore.info.detected_at).toLocaleString()}
				</div>
			</div>
		{:else}
			<div class="flex flex-col items-center justify-center py-8 text-center">
				<Cpu class="text-muted-foreground mb-3 h-12 w-12" />
				<p class="text-muted-foreground mb-3 text-sm">No hardware information available</p>
				<Button onclick={refreshHardware}>Detect Hardware</Button>
			</div>
		{/if}
	</div>
{/if}

<style>
	.hardware-panel {
		position: fixed;
		right: 1rem;
		bottom: 1rem;
		z-index: 50;
		width: min(28rem, calc(100vw - 1.6rem));
		max-height: min(80vh, 44rem);
		overflow-y: auto;
		border-radius: calc(var(--radius-xl) + 6px);
		border: 1px solid var(--outline-soft);
		padding: 1rem;
		background:
			linear-gradient(
				180deg,
				color-mix(in srgb, var(--surface-floating) 95%, black),
				color-mix(in srgb, var(--surface-subtle) 96%, black)
			),
			var(--surface-floating);
		box-shadow: var(--shadow-strong);
		backdrop-filter: blur(14px);
	}

	.hardware-panel .grid.grid-cols-3 {
		grid-template-columns: minmax(6.25rem, 34%) minmax(0, 1fr) minmax(0, 1fr);
	}

	.hardware-panel .grid.grid-cols-3 > .col-span-2 {
		min-width: 0;
		overflow-wrap: anywhere;
		word-break: break-word;
	}

	.hardware-panel .grid.grid-cols-3 > .font-mono {
		word-break: break-all;
	}

	@media (max-width: 560px) {
		.hardware-panel .grid.grid-cols-3 {
			grid-template-columns: minmax(0, 1fr);
			gap: 0.2rem;
		}

		.hardware-panel .grid.grid-cols-3 > .col-span-2 {
			grid-column: 1 / -1;
		}
	}

	@media (max-width: 920px) {
		.hardware-panel {
			right: 0.8rem;
			left: 0.8rem;
			bottom: 0.8rem;
			width: auto;
		}
	}
</style>
