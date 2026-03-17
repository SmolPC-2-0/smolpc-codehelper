import { Channel, invoke } from '@tauri-apps/api/core';

import type {
	AssistantResponseDto,
	AssistantSendRequestDto,
	AssistantStreamEvent
} from '$lib/types/assistant';
import type { AppMode, ModeConfigDto } from '$lib/types/mode';
import type { ModeStatusDto } from '$lib/types/provider';
import type { SetupStatusDto } from '$lib/types/setup';

export async function listModes(): Promise<ModeConfigDto[]> {
	return invoke<ModeConfigDto[]>('list_modes');
}

export async function getModeStatus(mode: AppMode): Promise<ModeStatusDto> {
	return invoke<ModeStatusDto>('mode_status', { mode });
}

export async function refreshModeTools(mode: AppMode): Promise<ModeStatusDto> {
	return invoke<ModeStatusDto>('mode_refresh_tools', { mode });
}

export async function assistantSend(
	request: AssistantSendRequestDto,
	onEvent: (event: AssistantStreamEvent) => void
): Promise<AssistantResponseDto> {
	const channel = new Channel<AssistantStreamEvent>();
	channel.onmessage = (event) => onEvent(event);

	return invoke<AssistantResponseDto>('assistant_send', {
		request,
		onEvent: channel
	});
}

export async function assistantCancel(): Promise<void> {
	return invoke<void>('assistant_cancel');
}

export async function undoModeAction(mode: AppMode): Promise<void> {
	return invoke<void>('mode_undo', { mode });
}

export async function getSetupStatus(): Promise<SetupStatusDto> {
	return invoke<SetupStatusDto>('setup_status');
}

export async function prepareSetup(): Promise<SetupStatusDto> {
	return invoke<SetupStatusDto>('setup_prepare');
}
