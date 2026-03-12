import { invoke } from '@tauri-apps/api/core';
import type {
	EngineStatusSummary,
	LauncherAppSummary,
	LauncherInstallResult,
	LauncherLaunchResult
} from '$lib/types/launcher';

const POLL_INTERVAL_MS = 3000;

let apps = $state<LauncherAppSummary[]>([]);
let engineStatus = $state<EngineStatusSummary>({
	reachable: false,
	ready: false,
	state: null,
	active_model: null
});
let error = $state<string | null>(null);
let launching = $state<string | null>(null);
let launchError = $state<string | null>(null);

let installing = $state<string | null>(null);
let installError = $state<string | null>(null);
let installMessage = $state<string | null>(null);
let manualRegistrationAppId = $state<string | null>(null);

let pollTimer: ReturnType<typeof setInterval> | null = null;

async function refreshApps() {
	try {
		apps = await invoke<LauncherAppSummary[]>('launcher_list_apps');
		error = null;
	} catch (e) {
		error = e instanceof Error ? e.message : String(e);
	}
}

async function refreshEngineStatus() {
	try {
		engineStatus = await invoke<EngineStatusSummary>('engine_status');
	} catch {
		engineStatus = { reachable: false, ready: false, state: null, active_model: null };
	}
}

async function refresh() {
	await Promise.all([refreshApps(), refreshEngineStatus()]);
}

async function launchOrFocus(appId: string): Promise<LauncherLaunchResult | null> {
	launching = appId;
	launchError = null;
	try {
		const result = await invoke<LauncherLaunchResult>('launcher_launch_or_focus', {
			appId
		});
		await refresh();
		return result;
	} catch (e) {
		launchError = e instanceof Error ? e.message : String(e);
		return null;
	} finally {
		launching = null;
	}
}

async function installApp(appId: string): Promise<LauncherInstallResult | null> {
	installing = appId;
	installError = null;
	installMessage = null;
	try {
		const result = await invoke<LauncherInstallResult>('launcher_install_app', { appId });
		if (result.outcome === 'installed') {
			installMessage = result.message;
			manualRegistrationAppId = null;
		} else if (result.outcome === 'retry_required') {
			installError = result.message;
			manualRegistrationAppId = null;
		} else {
			installError = result.message;
			manualRegistrationAppId = appId;
		}
		await refresh();
		return result;
	} catch (e) {
		installError = e instanceof Error ? e.message : String(e);
		return null;
	} finally {
		installing = null;
	}
}

async function browseAndRegisterManualPath(appId: string): Promise<LauncherInstallResult | null> {
	installing = appId;
	installError = null;
	installMessage = null;
	try {
		const exePath = await invoke<string | null>('launcher_pick_manual_exe');
		if (!exePath) {
			return null;
		}

		const result = await invoke<LauncherInstallResult>('launcher_register_manual_path', {
			appId,
			exePath
		});
		installMessage = result.message;
		manualRegistrationAppId = null;
		await refresh();
		return result;
	} catch (e) {
		installError = e instanceof Error ? e.message : String(e);
		return null;
	} finally {
		installing = null;
	}
}

function dismissLaunchError() {
	launchError = null;
}

function dismissInstallError() {
	installError = null;
}

function dismissInstallMessage() {
	installMessage = null;
}

function startPolling() {
	if (pollTimer) return;
	refresh();
	pollTimer = setInterval(refresh, POLL_INTERVAL_MS);
}

function stopPolling() {
	if (pollTimer) {
		clearInterval(pollTimer);
		pollTimer = null;
	}
}

function handleVisibilityChange() {
	if (document.hidden) {
		stopPolling();
	} else {
		startPolling();
	}
}

export const launcherStore = {
	get apps() {
		return apps;
	},
	get engineStatus() {
		return engineStatus;
	},
	get error() {
		return error;
	},
	get launching() {
		return launching;
	},
	get launchError() {
		return launchError;
	},
	get installing() {
		return installing;
	},
	get installError() {
		return installError;
	},
	get installMessage() {
		return installMessage;
	},
	get manualRegistrationAppId() {
		return manualRegistrationAppId;
	},
	refresh,
	launchOrFocus,
	installApp,
	browseAndRegisterManualPath,
	dismissLaunchError,
	dismissInstallError,
	dismissInstallMessage,
	startPolling,
	stopPolling,
	handleVisibilityChange
};
