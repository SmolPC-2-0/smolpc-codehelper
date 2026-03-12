<script lang="ts">
	import { onMount } from 'svelte';
	import { launcherStore } from '$lib/stores/launcher.svelte';
	import AppList from '$lib/components/AppList.svelte';
	import EngineStatusBar from '$lib/components/EngineStatusBar.svelte';

	onMount(() => {
		launcherStore.startPolling();
		document.addEventListener('visibilitychange', launcherStore.handleVisibilityChange);
		return () => {
			launcherStore.stopPolling();
			document.removeEventListener('visibilitychange', launcherStore.handleVisibilityChange);
		};
	});
</script>

<div class="flex h-full flex-col">
	<header class="px-4 pt-4 pb-2">
		<h1 class="text-base font-semibold text-foreground">SmolPC Launcher</h1>
		<p class="text-xs text-muted-foreground">AI helpers for students</p>
	</header>

	<main class="flex-1 overflow-y-auto py-1">
		{#if launcherStore.error}
			<div class="mx-4 mb-2 rounded-md bg-destructive/10 px-3 py-2 text-xs text-destructive">
				{launcherStore.error}
			</div>
		{/if}

		{#if launcherStore.launchError}
			<div class="mx-4 mb-2 flex items-start gap-2 rounded-md bg-destructive/10 px-3 py-2">
				<span class="flex-1 text-xs text-destructive">{launcherStore.launchError}</span>
				<button
					class="shrink-0 text-xs text-destructive underline"
					onclick={() => launcherStore.dismissLaunchError()}
				>
					dismiss
				</button>
			</div>
		{/if}

		<AppList
			apps={launcherStore.apps}
			launching={launcherStore.launching}
			onlaunch={(appId) => launcherStore.launchOrFocus(appId)}
		/>
	</main>

	<EngineStatusBar status={launcherStore.engineStatus} />
</div>
