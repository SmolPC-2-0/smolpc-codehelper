<script lang="ts">
	import type { LauncherAppSummary } from '$lib/types/launcher';

	let {
		app,
		launching = false,
		installing = false,
		onclick
	}: {
		app: LauncherAppSummary;
		launching?: boolean;
		installing?: boolean;
		onclick: () => void;
	} = $props();

	function actionLabel() {
		if (launching) return 'Launching';
		if (installing) return 'Installing';
		if (app.manual_registration_required || app.install_state === 'broken') return 'Repair';
		if (app.install_state === 'not_installed') return 'Install';
		if (app.is_running) return 'Running';
		return 'Launch';
	}

	function canAct() {
		if (app.is_running) return false;
		return (
			app.install_state === 'installed' ||
			app.can_install ||
			app.manual_registration_required ||
			app.install_state === 'broken'
		);
	}

	function subtitle() {
		if (launching) return 'Starting engine and app...';
		if (installing) return 'Running installer...';
		if (app.manual_registration_required) return 'Manual registration required';
		if (app.install_state === 'not_installed') return 'Not installed yet';
		if (app.install_state === 'broken') return 'Install path is missing';
		if (app.is_running) return 'Running';
		return 'Ready to launch';
	}
</script>

<button
	class="group flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors duration-150 hover:bg-accent active:bg-accent/80 disabled:pointer-events-none disabled:opacity-60"
	onclick={onclick}
	disabled={launching || installing || !canAct()}
>
	<div
		class="relative flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-secondary text-base font-medium text-secondary-foreground"
	>
		{app.display_name.charAt(0)}
		{#if app.is_running}
			<span
				class="absolute -right-0.5 -top-0.5 h-2.5 w-2.5 rounded-full border-2 border-background bg-success"
			></span>
		{:else if app.install_state === 'not_installed'}
			<span
				class="absolute -right-0.5 -top-0.5 h-2.5 w-2.5 rounded-full border-2 border-background bg-amber-500"
			></span>
		{:else if app.install_state === 'broken'}
			<span
				class="absolute -right-0.5 -top-0.5 h-2.5 w-2.5 rounded-full border-2 border-background bg-destructive"
			></span>
		{/if}
	</div>

	<div class="min-w-0 flex-1">
		<div class="truncate text-sm font-medium text-foreground">
			{app.display_name}
		</div>
		<div class="truncate text-xs text-muted-foreground">{subtitle()}</div>
	</div>

	<div class="shrink-0 text-xs font-medium text-muted-foreground">
		{actionLabel()}
	</div>
</button>
