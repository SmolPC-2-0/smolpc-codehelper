<script lang="ts">
	import type { EngineStatusSummary } from '$lib/types/launcher';

	let { status }: { status: EngineStatusSummary } = $props();

	let dotColor = $derived(
		status.ready ? 'bg-success' : status.reachable ? 'bg-warning' : 'bg-muted-foreground/50'
	);

	let label = $derived(
		status.ready
			? `Engine ready${status.active_model ? ` — ${status.active_model}` : ''}`
			: status.reachable
				? `Engine ${status.state ?? 'starting'}...`
				: 'Engine starting...'
	);
</script>

<footer class="flex items-center gap-2 border-t border-border px-4 py-2">
	<span class="h-2 w-2 shrink-0 rounded-full {dotColor}"></span>
	<span class="truncate text-xs text-muted-foreground">{label}</span>
</footer>
