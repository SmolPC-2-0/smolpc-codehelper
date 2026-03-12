<script lang="ts">
	import type { EngineStatusSummary } from '$lib/types/launcher';

	let { status }: { status: EngineStatusSummary } = $props();

	let dotColor = $derived(
		status.ready ? 'bg-success' : status.reachable ? 'bg-warning' : 'bg-muted-foreground/40'
	);

	let label = $derived(
		status.ready
			? `Engine ready${status.active_model ? ` \u00B7 ${status.active_model}` : ''}`
			: status.reachable
				? `Engine ${status.state ?? 'starting'}...`
				: 'Engine offline'
	);

	let isStarting = $derived(status.reachable && !status.ready);
</script>

<footer class="flex items-center gap-2.5 border-t border-border/60 bg-card/40 px-5 py-2.5">
	<span class="relative flex h-2 w-2 shrink-0">
		{#if isStarting}
			<span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-warning opacity-50"></span>
		{/if}
		<span class="relative inline-flex h-2 w-2 rounded-full {dotColor}"></span>
	</span>
	<span class="truncate text-[11px] text-muted-foreground">{label}</span>
</footer>
