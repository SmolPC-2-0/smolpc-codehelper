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
	if (micState !== 'idle') return;

	try {
		micError = null;
		micState = 'recording';
		await invoke('start_recording');
	} catch (e) {
		const msg = String(e);
		console.error('start_recording failed:', msg);
		micError = msg;
		if (msg.includes('No microphone') || msg.includes('microphone')) {
			micState = 'disabled';
		} else {
			micState = 'idle';
		}
	}
}

async function stopRecording(): Promise<string> {
	if (micState !== 'recording') return '';

	micState = 'processing';
	try {
		const text = await invoke<string>('stop_recording');
		return text;
	} catch (e) {
		const msg = String(e);
		console.error('stop_recording failed:', msg);
		micError = msg;
		return '';
	} finally {
		micState = 'idle';
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
