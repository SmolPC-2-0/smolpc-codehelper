<script lang="ts">
	interface Props {
		archiveName: string;
		bytesDown: number;
		totalBytes: number;
		phase: string;
		oncancel: () => void;
	}

	let { archiveName, bytesDown, totalBytes, phase, oncancel }: Props = $props();

	const mbDone = $derived((bytesDown / 1_048_576).toFixed(1));
	const mbTotal = $derived((totalBytes / 1_048_576).toFixed(1));
	const pct = $derived(
		totalBytes > 0 ? Math.min(100, Math.round((bytesDown / totalBytes) * 100)) : 0
	);
	const barWidth = $derived(`${pct}%`);
</script>

<div class="flex flex-col gap-4 rounded-xl border border-zinc-700 bg-zinc-800 p-6">
	<div class="flex items-start justify-between gap-4">
		<div class="min-w-0">
			<p class="truncate text-sm font-semibold text-zinc-100">{archiveName || 'Preparing…'}</p>
			<p class="mt-0.5 text-xs text-zinc-400 capitalize">{phase}</p>
		</div>
		<button
			type="button"
			onclick={oncancel}
			class="shrink-0 rounded-lg border border-zinc-600 bg-zinc-700 px-3 py-1.5 text-xs font-medium text-zinc-200 hover:bg-zinc-600 focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-zinc-800 focus:outline-none"
		>
			Cancel
		</button>
	</div>

	<div class="flex flex-col gap-1.5">
		<div class="h-2 w-full overflow-hidden rounded-full bg-zinc-700">
			<div
				class="h-full rounded-full bg-blue-500 transition-all duration-200"
				style="width: {barWidth}"
				role="progressbar"
				aria-valuenow={pct}
				aria-valuemin={0}
				aria-valuemax={100}
			></div>
		</div>
		<div class="flex items-center justify-between text-xs text-zinc-400">
			<span>{mbDone} MB / {mbTotal} MB</span>
			<span>{pct}%</span>
		</div>
	</div>
</div>
