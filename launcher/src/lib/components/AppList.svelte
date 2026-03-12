<script lang="ts">
	import type { LauncherAppSummary } from '$lib/types/launcher';
	import AppCard from './AppCard.svelte';

	let {
		apps,
		launching,
		onlaunch
	}: {
		apps: LauncherAppSummary[];
		launching: string | null;
		onlaunch: (appId: string) => void;
	} = $props();
</script>

<div class="flex flex-col gap-1 px-2">
	{#if apps.length === 0}
		<div class="py-8 text-center text-sm text-muted-foreground">
			No apps configured
		</div>
	{:else}
		{#each apps as app (app.app_id)}
			<AppCard
				{app}
				launching={launching === app.app_id}
				onclick={() => onlaunch(app.app_id)}
			/>
		{/each}
	{/if}
</div>
