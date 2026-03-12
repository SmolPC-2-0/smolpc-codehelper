<script lang="ts">
	import type { LauncherAppSummary } from '$lib/types/launcher';

	let {
		app,
		launching = false,
		onclick
	}: {
		app: LauncherAppSummary;
		launching?: boolean;
		onclick: () => void;
	} = $props();
</script>

<button
	class="group flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors duration-150 hover:bg-accent active:bg-accent/80 disabled:pointer-events-none disabled:opacity-60"
	onclick={onclick}
	disabled={launching}
>
	<div
		class="relative flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-secondary text-base font-medium text-secondary-foreground"
	>
		{app.display_name.charAt(0)}
		{#if app.is_running}
			<span
				class="absolute -right-0.5 -top-0.5 h-2.5 w-2.5 rounded-full border-2 border-background bg-success"
			></span>
		{/if}
	</div>

	<div class="min-w-0 flex-1">
		<div class="truncate text-sm font-medium text-foreground">
			{app.display_name}
		</div>
		<div class="truncate text-xs text-muted-foreground">
			{#if launching}
				Starting engine & app...
			{:else if app.is_running}
				Running — click to focus
			{:else}
				Click to launch
			{/if}
		</div>
	</div>

	<div class="shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100">
		{#if launching}
			<!-- spinner -->
			<svg
				class="h-4 w-4 animate-spin"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
			>
				<circle cx="12" cy="12" r="10" stroke-opacity="0.25" />
				<path d="M12 2a10 10 0 0 1 10 10" stroke-linecap="round" />
			</svg>
		{:else}
			<!-- chevron right -->
			<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M9 18l6-6-6-6" stroke-linecap="round" stroke-linejoin="round" />
			</svg>
		{/if}
	</div>
</button>
