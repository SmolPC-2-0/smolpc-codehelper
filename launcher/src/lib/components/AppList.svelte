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
				installing={installing === app.app_id}
				onclick={() => onprimary(app)}
			/>
		{/each}
	{/if}
</div>
