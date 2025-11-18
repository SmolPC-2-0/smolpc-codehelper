import { invoke } from '@tauri-apps/api/core';
import type { HardwareInfo } from '$lib/types/hardware';

// State
let hardware = $state<HardwareInfo | null>(null);
let loading = $state(false);
let error = $state<string | undefined>(undefined);

export const hardwareStore = {
	// Getters
	get info() {
		return hardware;
	},
	get loading() {
		return loading;
	},
	get error() {
		return error;
	},

	// Actions
	async detect(): Promise<void> {
		loading = true;
		error = undefined;

		try {
			hardware = await invoke<HardwareInfo>('detect_hardware');
		} catch (e) {
			error = String(e);
			console.error('Failed to detect hardware:', e);
		} finally {
			loading = false;
		}
	},

	async getCached(): Promise<void> {
		try {
			const cached = await invoke<HardwareInfo | null>('get_cached_hardware');
			if (cached) {
				hardware = cached;
			}
		} catch (e) {
			console.error('Failed to get cached hardware:', e);
		}
	},

	// Helper to get primary GPU (first discrete GPU or first GPU)
	getPrimaryGpu() {
		if (!hardware || hardware.gpus.length === 0) {
			return null;
		}

		// Prefer discrete GPU
		const discrete = hardware.gpus.find((gpu) =>
			gpu.device_type.toLowerCase().includes('discrete')
		);
		return discrete || hardware.gpus[0];
	}
};
