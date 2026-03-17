import { getSetupStatus, prepareSetup } from '$lib/api/unified';
import type { SetupStatusDto } from '$lib/types/setup';

function toErrorMessage(cause: unknown): string {
	return cause instanceof Error ? cause.message : String(cause);
}

let status = $state<SetupStatusDto | null>(null);
let loading = $state(false);
let preparing = $state(false);
let initialized = $state(false);
let error = $state<string | null>(null);

export const setupStore = {
	get status() {
		return status;
	},
	get items() {
		return status?.items ?? [];
	},
	get loading() {
		return loading;
	},
	get preparing() {
		return preparing;
	},
	get initialized() {
		return initialized;
	},
	get error() {
		return error ?? status?.lastError ?? null;
	},
	get overallState() {
		return status?.overallState ?? (error ? 'error' : null);
	},
	get needsAttention() {
		return this.overallState !== null && this.overallState !== 'ready';
	},

	async initialize(): Promise<void> {
		if (initialized || loading) {
			return;
		}

		loading = true;
		error = null;
		try {
			status = await getSetupStatus();
		} catch (cause) {
			error = toErrorMessage(cause);
		} finally {
			initialized = true;
			loading = false;
		}
	},

	async refresh(): Promise<void> {
		loading = true;
		error = null;
		try {
			status = await getSetupStatus();
		} catch (cause) {
			error = toErrorMessage(cause);
		} finally {
			loading = false;
		}
	},

	async prepare(): Promise<void> {
		preparing = true;
		error = null;
		try {
			status = await prepareSetup();
		} catch (cause) {
			error = toErrorMessage(cause);
		} finally {
			preparing = false;
		}
	}
};
