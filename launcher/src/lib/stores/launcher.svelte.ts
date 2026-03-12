import { invoke } from '@tauri-apps/api/core';
import type {
	EngineStatusSummary,
	LauncherAppSummary,
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

function dismissLaunchError() {
	launchError = null;
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
	refresh,
	launchOrFocus,
	dismissLaunchError,
	startPolling,
	stopPolling,
	handleVisibilityChange
};
