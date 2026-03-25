import { invoke, Channel } from '@tauri-apps/api/core';

export interface ProvisioningEvent {
	kind:
		| 'ArchiveStarted'
		| 'Progress'
		| 'Verifying'
		| 'ArchiveComplete'
		| 'Error'
		| 'Complete';
	name?: string;
	total_bytes?: number;
	bytes_done?: number;
	code?: string;
	message?: string;
	retryable?: boolean;
	models_installed?: string[];
}

export interface ModelSource {
	kind: 'Local' | 'Internet';
	path?: string;
	base_url?: string;
}

export interface ModelRecommendation {
	model_id: string;
	backend: string;
	display_name: string;
	download_size_bytes: number;
	reason: string;
}

// State
let sources = $state<ModelSource[]>([]);
let recommendation = $state<ModelRecommendation | null>(null);
let currentArchive = $state<string>('');
let bytesDown = $state<number>(0);
let totalBytes = $state<number>(0);
let phase = $state<'detecting' | 'ready' | 'provisioning' | 'verifying' | 'complete' | 'error'>(
	'detecting'
);
let errorMessage = $state<string>('');
let errorRetryable = $state<boolean>(false);
let modelsInstalled = $state<string[]>([]);

let progress = $derived(totalBytes > 0 ? bytesDown / totalBytes : 0);

export const provisioningStore = {
	// Getters
	get sources() {
		return sources;
	},
	get recommendation() {
		return recommendation;
	},
	get currentArchive() {
		return currentArchive;
	},
	get bytesDown() {
		return bytesDown;
	},
	get totalBytes() {
		return totalBytes;
	},
	get progress() {
		return progress;
	},
	get phase() {
		return phase;
	},
	get errorMessage() {
		return errorMessage;
	},
	get errorRetryable() {
		return errorRetryable;
	},
	get modelsInstalled() {
		return modelsInstalled;
	},

	// Actions
	async detectSources(): Promise<void> {
		phase = 'detecting';
		errorMessage = '';
		errorRetryable = false;

		try {
			const [detected, rec] = await Promise.all([
				invoke<ModelSource[]>('detect_model_sources'),
				invoke<ModelRecommendation | null>('get_recommended_model').catch(() => null)
			]);
			sources = detected;
			recommendation = rec;
			phase = 'ready';
		} catch (e) {
			errorMessage = String(e);
			errorRetryable = true;
			phase = 'error';
			console.error('Failed to detect model sources:', e);
		}
	},

	async startProvisioning(source: ModelSource, modelIds: string[]): Promise<void> {
		phase = 'provisioning';
		currentArchive = '';
		bytesDown = 0;
		totalBytes = 0;
		errorMessage = '';
		errorRetryable = false;
		modelsInstalled = [];

		let channelReportedError = false;
		const channel = new Channel<ProvisioningEvent>();

		channel.onmessage = (event: ProvisioningEvent) => {
			switch (event.kind) {
				case 'ArchiveStarted':
					currentArchive = event.name ?? '';
					bytesDown = 0;
					totalBytes = event.total_bytes ?? 0;
					break;

				case 'Progress':
					bytesDown = event.bytes_done ?? bytesDown;
					if (event.total_bytes != null) {
						totalBytes = event.total_bytes;
					}
					break;

				case 'Verifying':
					phase = 'verifying';
					currentArchive = event.name ?? currentArchive;
					break;

				case 'ArchiveComplete':
					currentArchive = event.name ?? currentArchive;
					break;

				case 'Error':
					channelReportedError = true;
					errorMessage = event.message ?? 'Unknown provisioning error';
					errorRetryable = event.retryable ?? false;
					phase = 'error';
					console.error('Provisioning error:', event.code, event.message);
					break;

				case 'Complete':
					modelsInstalled = event.models_installed ?? [];
					phase = 'complete';
					break;
			}
		};

		try {
			await invoke('provision_models', { source, modelIds, channel });
		} catch (e) {
			// Only override if no error was already reported via the channel
			if (!channelReportedError) {
				errorMessage = String(e);
				errorRetryable = false;
				phase = 'error';
				console.error('Provisioning invoke failed:', e);
			}
		}
	},

	async cancel(): Promise<void> {
		try {
			await invoke('cancel_provisioning');
		} catch (e) {
			console.error('Failed to cancel provisioning:', e);
		}
	}
};
