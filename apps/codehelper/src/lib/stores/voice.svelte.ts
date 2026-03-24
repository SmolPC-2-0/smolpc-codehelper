/**
 * Voice I/O store for mic recording (STT) and text-to-speech (TTS) playback.
 *
 * Centralized state machine — components read getters and call async actions.
 * Uses Svelte 5 runes ($state). No writable/readable stores.
 */
import { invoke } from '@tauri-apps/api/core';

// ── Types ────────────────────────────────────────────────────────────

export type MicState = 'idle' | 'recording' | 'processing' | 'disabled';
export type TtsState = 'idle' | 'loading' | 'playing' | 'unavailable';

// ── State ────────────────────────────────────────────────────────────

let micState = $state<MicState>('idle');
let ttsState = $state<TtsState>('idle');
let ttsActiveMessageId = $state<string | null>(null);
let micError = $state<string | null>(null);
let playbackPollInterval: ReturnType<typeof setInterval> | null = null;

// ── Mic actions ──────────────────────────────────────────────────────

async function startRecording(): Promise<void> {
	console.log('[voice] startRecording called, current micState:', micState);
	if (micState !== 'idle') {
		console.log('[voice] startRecording skipped — micState is not idle');
		return;
	}

	try {
		micError = null;
		micState = 'recording';
		console.log('[voice] micState set to recording, invoking start_recording');
		await invoke('start_recording');
		console.log('[voice] start_recording invoke returned successfully, micState:', micState);
	} catch (e) {
		const msg = String(e);
		console.error('[voice] start_recording failed:', msg);
		micError = msg;
		if (msg.includes('No microphone') || msg.includes('microphone')) {
			micState = 'disabled';
		} else {
			micState = 'idle';
		}
	}
}

async function stopRecording(): Promise<string> {
	console.log('[voice] stopRecording called, current micState:', micState);
	if (micState !== 'recording') {
		console.log('[voice] stopRecording skipped — micState is not recording, it is:', micState);
		return '';
	}

	micState = 'processing';
	console.log('[voice] micState set to processing, invoking stop_recording');
	try {
		const text = await invoke<string>('stop_recording');
		console.log('[voice] stop_recording returned:', text?.length, 'chars');
		return text;
	} catch (e) {
		const msg = String(e);
		console.error('[voice] stop_recording failed:', msg);
		micError = msg;
		return '';
	} finally {
		micState = 'idle';
		console.log('[voice] micState reset to idle');
	}
}

// ── TTS actions ──────────────────────────────────────────────────────

async function speakMessage(messageId: string, text: string): Promise<void> {
	// Stop any existing playback first.
	if (ttsState === 'playing' || ttsState === 'loading') {
		await stopPlayback();
	}

	ttsActiveMessageId = messageId;
	ttsState = 'loading';

	try {
		await invoke('speak_text', { text });
		ttsState = 'playing';
		startPlaybackPolling();
	} catch (e) {
		const msg = String(e);
		console.error('speak_text failed:', msg);
		if (msg.includes('503') || msg.includes('unavailable') || msg.includes('TTS service')) {
			ttsState = 'unavailable';
		} else {
			ttsState = 'idle';
		}
		ttsActiveMessageId = null;
	}
}

async function stopPlayback(): Promise<void> {
	clearPlaybackPolling();
	try {
		await invoke('stop_playback');
	} catch (e) {
		console.error('stop_playback failed:', e);
	}
	ttsState = 'idle';
	ttsActiveMessageId = null;
}

// ── Playback polling ─────────────────────────────────────────────────

function startPlaybackPolling(): void {
	clearPlaybackPolling();
	playbackPollInterval = setInterval(async () => {
		try {
			const playing = await invoke<boolean>('is_playing');
			if (!playing) {
				clearPlaybackPolling();
				ttsState = 'idle';
				ttsActiveMessageId = null;
			}
		} catch {
			// If the invoke fails, assume playback ended.
			clearPlaybackPolling();
			ttsState = 'idle';
			ttsActiveMessageId = null;
		}
	}, 200);
}

function clearPlaybackPolling(): void {
	if (playbackPollInterval !== null) {
		clearInterval(playbackPollInterval);
		playbackPollInterval = null;
	}
}

// ── Exported store ───────────────────────────────────────────────────

export const voiceStore = {
	get micState() {
		return micState;
	},
	get ttsState() {
		return ttsState;
	},
	get ttsActiveMessageId() {
		return ttsActiveMessageId;
	},
	get micError() {
		return micError;
	},
	get isMicBusy() {
		return micState === 'recording' || micState === 'processing';
	},
	startRecording,
	stopRecording,
	speakMessage,
	stopPlayback
};
