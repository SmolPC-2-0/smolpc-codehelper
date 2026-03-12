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

	// Map app IDs to distinct gradient colors
	const APP_GRADIENTS: Record<string, string> = {
		codehelper: 'linear-gradient(135deg, oklch(0.55 0.18 250), oklch(0.45 0.2 280))',
		'blender-helper': 'linear-gradient(135deg, oklch(0.6 0.16 45), oklch(0.5 0.18 25))',
		'gimp-helper': 'linear-gradient(135deg, oklch(0.55 0.16 155), oklch(0.45 0.18 140))',
		'libreoffice-helper': 'linear-gradient(135deg, oklch(0.55 0.16 120), oklch(0.45 0.18 105))'
	};

	const APP_ICONS: Record<string, string> = {
		codehelper: 'terminal',
		'blender-helper': 'cube',
		'gimp-helper': 'image',
		'libreoffice-helper': 'file-text'
	};

	function getGradient() {
		return APP_GRADIENTS[app.app_id] ?? 'linear-gradient(135deg, oklch(0.5 0.12 260), oklch(0.4 0.14 280))';
	}

	function getIconType() {
		return APP_ICONS[app.app_id] ?? 'box';
	}

	function actionLabel() {
		if (launching) return 'Launching...';
		if (installing) return 'Installing...';
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

	function actionStyle() {
		if (launching || installing) return 'loading';
		if (app.is_running) return 'running';
		if (app.install_state === 'not_installed') return 'install';
		if (app.install_state === 'broken' || app.manual_registration_required) return 'repair';
		return 'launch';
	}
</script>

<button
	class="group relative flex w-full items-center gap-3.5 rounded-xl border border-transparent px-3 py-3 text-left transition-all duration-200 hover:border-border hover:bg-card active:scale-[0.99] disabled:pointer-events-none disabled:opacity-50"
	onclick={onclick}
	disabled={launching || installing || !canAct()}
>
	<!-- App icon -->
	<div class="relative shrink-0">
		<div
			class="flex h-11 w-11 items-center justify-center rounded-xl shadow-md transition-transform duration-200 group-hover:scale-105"
			style="background: {getGradient()};"
		>
			{#if getIconType() === 'terminal'}
				<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-white/90">
					<polyline points="4 17 10 11 4 5"></polyline>
					<line x1="12" y1="19" x2="20" y2="19"></line>
				</svg>
			{:else if getIconType() === 'cube'}
				<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-white/90">
					<path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path>
					<polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline>
					<line x1="12" y1="22.08" x2="12" y2="12"></line>
				</svg>
			{:else if getIconType() === 'image'}
				<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-white/90">
					<rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
					<circle cx="8.5" cy="8.5" r="1.5"></circle>
					<polyline points="21 15 16 10 5 21"></polyline>
				</svg>
			{:else if getIconType() === 'file-text'}
				<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-white/90">
					<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
					<polyline points="14 2 14 8 20 8"></polyline>
					<line x1="16" y1="13" x2="8" y2="13"></line>
					<line x1="16" y1="17" x2="8" y2="17"></line>
				</svg>
			{:else}
				<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-white/90">
					<path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path>
				</svg>
			{/if}
		</div>

		<!-- Status indicator -->
		{#if app.is_running}
			<span
				class="animate-pulse-glow absolute -right-0.5 -top-0.5 h-3 w-3 rounded-full border-2 border-background bg-success"
			></span>
		{:else if app.install_state === 'not_installed'}
			<span
				class="absolute -right-0.5 -top-0.5 flex h-3 w-3 items-center justify-center rounded-full border-2 border-background bg-warning"
			>
				<span class="h-1 w-1 rounded-full bg-white/80"></span>
			</span>
		{:else if app.install_state === 'broken'}
			<span
				class="absolute -right-0.5 -top-0.5 h-3 w-3 rounded-full border-2 border-background bg-destructive"
			></span>
		{/if}
	</div>

	<!-- Text content -->
	<div class="min-w-0 flex-1">
		<div class="truncate text-[13px] font-semibold text-foreground">
			{app.display_name}
		</div>
		<div class="mt-0.5 truncate text-[11px] text-muted-foreground">{subtitle()}</div>
	</div>

	<!-- Action badge -->
	<div class="shrink-0">
		{#if actionStyle() === 'loading'}
			<div class="flex items-center gap-1.5 rounded-lg bg-primary/10 px-2.5 py-1">
				<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" class="animate-spin-slow text-primary">
					<path d="M21 12a9 9 0 1 1-6.219-8.56"></path>
				</svg>
				<span class="text-[11px] font-medium text-primary">{actionLabel()}</span>
			</div>
		{:else if actionStyle() === 'running'}
			<div class="flex items-center gap-1.5 rounded-lg bg-success/10 px-2.5 py-1">
				<span class="h-1.5 w-1.5 rounded-full bg-success"></span>
				<span class="text-[11px] font-medium text-success">Running</span>
			</div>
		{:else if actionStyle() === 'install'}
			<div class="rounded-lg bg-primary/10 px-2.5 py-1 text-[11px] font-medium text-primary transition-colors group-hover:bg-primary/20">
				Install
			</div>
		{:else if actionStyle() === 'repair'}
			<div class="rounded-lg bg-warning/10 px-2.5 py-1 text-[11px] font-medium text-warning transition-colors group-hover:bg-warning/20">
				Repair
			</div>
		{:else}
			<div class="rounded-lg bg-secondary px-2.5 py-1 text-[11px] font-medium text-secondary-foreground transition-colors group-hover:bg-primary/15 group-hover:text-primary">
				Launch
			</div>
		{/if}
	</div>
</button>
