<script lang="ts">
	interface Props {
		archiveName: string;
		bytesDown: number;
		totalBytes: number;
		phase: string;
		oncancel: () => void;
	}

	let { archiveName, bytesDown, totalBytes, phase, oncancel }: Props = $props();
</script>

<div class="flex flex-col gap-4 rounded-xl border border-zinc-700 bg-zinc-800 p-6">
	<div class="flex items-start justify-between gap-4">
		<div class="min-w-0">
			<p class="truncate text-sm font-semibold text-zinc-100">{archiveName || 'Preparing…'}</p>
			<p class="mt-0.5 text-xs text-zinc-400 capitalize">{phase === 'verifying' ? 'Verifying checksum…' : 'Extracting…'}</p>
		</div>
		<button
			type="button"
			onclick={oncancel}
			class="shrink-0 rounded-lg border border-zinc-600 bg-zinc-700 px-3 py-1.5 text-xs font-medium text-zinc-200 hover:bg-zinc-600 focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-zinc-800 focus:outline-none"
		>
			Cancel
		</button>
	</div>

	<!-- Indeterminate spinner — extraction progress is unreliable (compressed vs decompressed size) -->
	<div class="flex items-center justify-center py-2">
		<div class="h-8 w-8 animate-spin rounded-full border-2 border-zinc-600 border-t-blue-500"></div>
	</div>
</div>
