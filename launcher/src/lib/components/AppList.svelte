<script lang="ts">
	import type { LauncherAppSummary } from '$lib/types/launcher';
	import AppCard from './AppCard.svelte';

	let {
		apps,
		launching,
		installing,
		onprimary
	}: {
		apps: LauncherAppSummary[];
		launching: string | null;
		installing: string | null;
		onprimary: (app: LauncherAppSummary) => void;
	} = $props();
</script>

<div class="flex flex-col gap-1">
	{#if apps.length === 0}
		<div class="flex flex-col items-center justify-center gap-2 py-12 text-center">
			<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-muted-foreground/40">
				<path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path>
			</svg>
			<p class="text-sm text-muted-foreground">No apps configured</p>
		</div>
	{:else}
		{#each apps as app (app.app_id)}
			<AppCard
				{app}
				launching={launching === app.app_id}
				installing={installing === app.app_id}
				onclick={() => onprimary(app)}
			/>
		{/each}
	{/if}
</div>
