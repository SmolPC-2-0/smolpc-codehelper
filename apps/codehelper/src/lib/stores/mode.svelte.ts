import { getModeStatus, listModes } from '$lib/api/unified';
import { loadFromStorage, saveToStorage } from '$lib/utils/storage';
import type { AppMode, ModeConfigDto } from '$lib/types/mode';
import type { ModeStatusDto } from '$lib/types/provider';

const ACTIVE_MODE_KEY = 'smolpc_unified_active_mode_v1';

let activeMode = $state<AppMode>('code');
let modeConfigs = $state<ModeConfigDto[]>([]);
let statusByMode = $state<Partial<Record<AppMode, ModeStatusDto>>>({});
let loading = $state(false);
let error = $state<string | null>(null);
let initialized = $state(false);

function getModeConfig(mode: AppMode): ModeConfigDto | null {
	return modeConfigs.find((config) => config.id === mode) ?? null;
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
		return error;
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
		error = null;

		try {
			modeConfigs = await listModes();
			const storedMode = loadFromStorage<AppMode>(ACTIVE_MODE_KEY, 'code');
			const resolvedMode = getModeConfig(storedMode) ? storedMode : 'code';

			activeMode = resolvedMode;
			saveToStorage(ACTIVE_MODE_KEY, activeMode);

			await loadModeStatus(activeMode);
			initialized = true;
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		} finally {
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
		error = null;

		try {
			await loadModeStatus(mode);
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		} finally {
			loading = false;
		}
	}
};
