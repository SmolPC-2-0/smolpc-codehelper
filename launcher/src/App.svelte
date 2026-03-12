<script lang="ts">
	import { onMount } from 'svelte';
	import { launcherStore } from '$lib/stores/launcher.svelte';
	import type { LauncherAppSummary } from '$lib/types/launcher';
	import AppList from '$lib/components/AppList.svelte';
	import EngineStatusBar from '$lib/components/EngineStatusBar.svelte';

	function handlePrimaryAction(app: LauncherAppSummary) {
		if (app.is_running) {
			return;
		}

		if (app.install_state === 'installed' && !app.manual_registration_required) {
			void launcherStore.launchOrFocus(app.app_id);
			return;
		}

		void launcherStore.installApp(app.app_id);
	}

	function handleManualBrowse() {
		if (!launcherStore.manualRegistrationAppId) return;
		void launcherStore.browseAndRegisterManualPath(launcherStore.manualRegistrationAppId);
	}

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
	<!-- Header with drag region -->
	<header
		data-tauri-drag-region
		class="relative flex items-center gap-3 px-5 pt-5 pb-3"
	>
		<!-- Logo mark -->
		<div
			class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg"
			style="background: linear-gradient(135deg, oklch(0.65 0.18 250), oklch(0.55 0.2 280));"
		>
			<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="text-white">
				<polyline points="4 17 10 11 4 5"></polyline>
				<line x1="12" y1="19" x2="20" y2="19"></line>
			</svg>
		</div>
		<div>
			<h1 class="text-sm font-semibold tracking-tight text-foreground">SmolPC Launcher</h1>
			<p class="text-[11px] text-muted-foreground">Offline AI helpers for students</p>
		</div>
	</header>

	<!-- Notifications area -->
	<div class="px-4">
		{#if launcherStore.error}
			<div
				class="mb-2 flex items-center gap-2 rounded-lg border border-destructive/20 bg-destructive/8 px-3 py-2"
			>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="shrink-0 text-destructive">
					<circle cx="12" cy="12" r="10"></circle>
					<line x1="15" y1="9" x2="9" y2="15"></line>
					<line x1="9" y1="9" x2="15" y2="15"></line>
				</svg>
				<span class="flex-1 text-xs text-destructive">{launcherStore.error}</span>
			</div>
		{/if}

		{#if launcherStore.launchError}
			<div
				class="mb-2 flex items-center gap-2 rounded-lg border border-destructive/20 bg-destructive/8 px-3 py-2"
			>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="shrink-0 text-destructive">
					<path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
					<line x1="12" y1="9" x2="12" y2="13"></line>
					<line x1="12" y1="17" x2="12.01" y2="17"></line>
				</svg>
				<span class="flex-1 text-xs text-destructive">{launcherStore.launchError}</span>
				<button
					class="shrink-0 rounded-md px-2 py-0.5 text-[11px] font-medium text-destructive transition-colors hover:bg-destructive/15"
					onclick={() => launcherStore.dismissLaunchError()}
				>
					Dismiss
				</button>
			</div>
		{/if}

		{#if launcherStore.installError}
			<div
				class="mb-2 rounded-lg border border-destructive/20 bg-destructive/8 px-3 py-2"
			>
				<div class="flex items-center gap-2">
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="shrink-0 text-destructive">
						<circle cx="12" cy="12" r="10"></circle>
						<line x1="12" y1="8" x2="12" y2="12"></line>
						<line x1="12" y1="16" x2="12.01" y2="16"></line>
					</svg>
					<span class="flex-1 text-xs text-destructive">{launcherStore.installError}</span>
				</div>
				<div class="mt-2 flex items-center gap-2 pl-[22px]">
					{#if launcherStore.manualRegistrationAppId}
						<button
							class="rounded-md bg-destructive/15 px-2.5 py-1 text-[11px] font-medium text-destructive transition-colors hover:bg-destructive/25"
							onclick={handleManualBrowse}
						>
							Browse .exe
						</button>
					{/if}
					<button
						class="rounded-md px-2.5 py-1 text-[11px] font-medium text-destructive transition-colors hover:bg-destructive/15"
						onclick={() => launcherStore.dismissInstallError()}
					>
						Dismiss
					</button>
				</div>
			</div>
		{/if}

		{#if launcherStore.installMessage}
			<div
				class="mb-2 flex items-center gap-2 rounded-lg border border-success/20 bg-success/8 px-3 py-2"
			>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="shrink-0 text-success">
					<path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
					<polyline points="22 4 12 14.01 9 11.01"></polyline>
				</svg>
				<span class="flex-1 text-xs text-success">{launcherStore.installMessage}</span>
				<button
					class="shrink-0 rounded-md px-2 py-0.5 text-[11px] font-medium text-success transition-colors hover:bg-success/15"
					onclick={() => launcherStore.dismissInstallMessage()}
				>
					Dismiss
				</button>
			</div>
		{/if}
	</div>

	<!-- App list -->
	<main class="flex-1 overflow-y-auto px-4 py-1">
		<AppList
			apps={launcherStore.apps}
			launching={launcherStore.launching}
			installing={launcherStore.installing}
			onprimary={handlePrimaryAction}
		/>
	</main>

	<!-- Engine status footer -->
	<EngineStatusBar status={launcherStore.engineStatus} />
</div>
