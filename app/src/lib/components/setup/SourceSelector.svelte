<script lang="ts">
	import type { ModelSource, ModelRecommendation } from '$lib/stores/provisioning.svelte';

	interface Props {
		sources: ModelSource[];
		recommendation: ModelRecommendation | null;
		onselect: (source: ModelSource) => void;
		onretry?: () => void;
		onskip?: () => void;
	}

	let { sources, recommendation, onselect, onretry, onskip }: Props = $props();

	function formatBytes(bytes: number): string {
		if (bytes >= 1_073_741_824) {
			return `${(bytes / 1_073_741_824).toFixed(1)} GB`;
		}
		return `${(bytes / 1_048_576).toFixed(0)} MB`;
	}

	function sourceLabel(source: ModelSource): string {
		if (source.kind === 'Local') {
			return source.path ?? 'Local folder';
		}
		return source.base_url ?? 'Internet';
	}
</script>

<div class="flex flex-col gap-3">
	{#each sources as source (source.kind === 'Local' ? source.path : source.base_url)}
		<button
			type="button"
			onclick={() => onselect(source)}
			class="flex w-full flex-col gap-1.5 rounded-xl border border-zinc-700 bg-zinc-800 p-4 text-left transition-colors hover:border-blue-500/60 hover:bg-zinc-700 focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-zinc-900 focus:outline-none"
		>
			<div class="flex items-center justify-between gap-3">
				<span class="text-xs font-semibold tracking-wider text-blue-400 uppercase">
					{source.kind === 'Local' ? 'Local' : 'Download'}
				</span>
				{#if source.kind === 'Internet' && recommendation}
					<span class="text-xs text-zinc-400"
						>{formatBytes(recommendation.download_size_bytes)}</span
					>
				{/if}
			</div>

			{#if source.kind === 'Local'}
				<p class="text-sm font-medium break-all text-zinc-100">{sourceLabel(source)}</p>
				<p class="text-xs text-zinc-400">Install AI model from local archive</p>
			{:else if recommendation}
				<p class="text-sm font-medium text-zinc-100">{recommendation.display_name}</p>
				<p class="text-xs text-zinc-400">{recommendation.reason}</p>
			{:else}
				<p class="text-sm font-medium text-zinc-100">Download from internet</p>
				<p class="text-xs text-zinc-400">{sourceLabel(source)}</p>
			{/if}
		</button>
	{/each}

	{#if sources.length === 0}
		<div
			class="flex flex-col items-center gap-4 rounded-xl border border-amber-500/30 bg-amber-500/10 p-6"
		>
			<p class="text-sm text-amber-200/80">
				No installation sources found. Connect a USB drive with SmolPC models, check your internet
				connection, or skip setup to use the app without AI features.
			</p>
			<div class="flex gap-3">
				{#if onretry}
					<button
						type="button"
						onclick={onretry}
						class="rounded-lg border border-amber-500/40 bg-amber-500/20 px-4 py-2 text-sm font-medium text-amber-200 hover:bg-amber-500/30 focus:ring-2 focus:ring-amber-500 focus:ring-offset-2 focus:ring-offset-zinc-900 focus:outline-none"
					>
						Retry
					</button>
				{/if}
				{#if onskip}
					<button
						type="button"
						onclick={onskip}
						class="rounded-lg border border-zinc-600 bg-zinc-800 px-4 py-2 text-sm font-medium text-zinc-300 hover:bg-zinc-700 focus:ring-2 focus:ring-zinc-500 focus:ring-offset-2 focus:ring-offset-zinc-900 focus:outline-none"
					>
						Skip for now
					</button>
				{/if}
			</div>
		</div>
	{/if}
</div>
