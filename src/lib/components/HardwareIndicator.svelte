<script lang="ts">
	import { hardwareStore } from '$lib/stores/hardware.svelte';
	import { Cpu, Gpu, Zap } from '@lucide/svelte';

	interface Props {
		onclick?: () => void;
	}

	let { onclick }: Props = $props();

	const primaryGpu = $derived(hardwareStore.getPrimaryGpu());
	const hasNpu = $derived(hardwareStore.info?.npu?.detected ?? false);
</script>

<button
	onclick={onclick}
	class="flex items-center gap-2 rounded-md border border-border bg-background px-3 py-2 text-sm font-medium transition-colors hover:bg-muted cursor-pointer"
	aria-label="View hardware information"
>
	{#if primaryGpu}
		<Gpu class="h-3 w-3 text-primary" />
		<span class="max-w-[150px] truncate">{primaryGpu.name}</span>
		{#if hasNpu}
			<Zap class="h-3 w-3 text-yellow-500" />
		{/if}
	{:else if hardwareStore.info}
		<Cpu class="h-3 w-3 text-primary" />
		<span>CPU Only</span>
	{:else}
		<Cpu class="h-3 w-3 text-muted-foreground animate-pulse" />
		<span class="text-muted-foreground">Detecting...</span>
	{/if}
</button>
