import { getModeStatus, listModes } from '$lib/api/unified';
import { loadFromStorage, saveToStorage } from '$lib/utils/storage';
import { FALLBACK_MODE_CONFIGS, type AppMode, type ModeConfigDto } from '$lib/types/mode';
import type { ModeStatusDto } from '$lib/types/provider';

const ACTIVE_MODE_KEY = 'smolpc_unified_active_mode_v1';

let activeMode = $state<AppMode>('code');
let modeConfigs = $state<ModeConfigDto[]>(FALLBACK_MODE_CONFIGS);
let statusByMode = $state<Partial<Record<AppMode, ModeStatusDto>>>({});
let loading = $state(false);
let configError = $state<string | null>(null);
let statusError = $state<string | null>(null);
let initialized = $state(false);

function getModeConfig(mode: AppMode): ModeConfigDto | null {
	return modeConfigs.find((config) => config.id === mode) ?? null;
}

function toErrorMessage(cause: unknown): string {
	return cause instanceof Error ? cause.message : String(cause);
}

async function loadModeStatus(mode: AppMode): Promise<void> {
	const status = await getModeStatus(mode);
	statusByMode = {
		...statusByMode,
		[mode]: status
	};
}

export const modeStore = {
	get activeMode() {
		return activeMode;
	},
	get modeConfigs() {
		return modeConfigs;
	},
	get statusByMode() {
		return statusByMode;
	},
	get loading() {
		return loading;
	},
	get error() {
		return (
			[configError, statusError].filter((value): value is string => Boolean(value)).join(' · ') ||
			null
		);
	},
	get initialized() {
		return initialized;
	},
	get activeConfig() {
		return getModeConfig(activeMode);
	},
	get activeStatus() {
		return statusByMode[activeMode] ?? null;
	},

	getConfig(mode: AppMode) {
		return getModeConfig(mode);
	},

	getStatus(mode: AppMode) {
		return statusByMode[mode] ?? null;
	},

	async initialize(): Promise<void> {
		if (initialized) {
			return;
		}

		loading = true;
		configError = null;
		statusError = null;
		const storedMode = loadFromStorage<AppMode>(ACTIVE_MODE_KEY, 'code');

		try {
			const remoteModes = await listModes();
			if (remoteModes.length === 0) {
				throw new Error('list_modes returned no modes');
			}
			modeConfigs = remoteModes;
		} catch (cause) {
			configError = `Mode list unavailable; using local fallback config. ${toErrorMessage(cause)}`;
			modeConfigs = FALLBACK_MODE_CONFIGS;
		}

		const resolvedMode = getModeConfig(storedMode) ? storedMode : 'code';
		activeMode = resolvedMode;
		saveToStorage(ACTIVE_MODE_KEY, activeMode);

		try {
			await loadModeStatus(activeMode);
		} catch (cause) {
			statusError = toErrorMessage(cause);
		} finally {
			initialized = true;
			loading = false;
		}
	},

	async setActiveMode(mode: AppMode): Promise<void> {
		if (activeMode === mode) {
			await this.refreshModeStatus(mode);
			return;
		}

		activeMode = mode;
		saveToStorage(ACTIVE_MODE_KEY, activeMode);
		await this.refreshModeStatus(mode);
	},

	async refreshModeStatus(mode: AppMode = activeMode): Promise<void> {
		loading = true;
		statusError = null;

		try {
			await loadModeStatus(mode);
		} catch (cause) {
			statusError = toErrorMessage(cause);
		} finally {
			loading = false;
		}
	}
};
