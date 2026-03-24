/**
 * Voice I/O store for mic recording (STT) and text-to-speech (TTS) playback.
 *
 * Centralized state machine — components read getters and call async actions.
 * Uses Svelte 5 runes ($state). No writable/readable stores.
 */
import { invoke } from '@tauri-apps/api/core';

// ── Types ────────────────────────────────────────────────────────────

export interface AudioRecordingReadyEvent {
	sessionId: string;
}

export type MicState = 'idle' | 'arming' | 'ready' | 'recording' | 'processing' | 'disabled';
export type TtsState = 'idle' | 'loading' | 'playing' | 'unavailable';

// ── State ────────────────────────────────────────────────────────────

let micState = $state<MicState>('idle');
let ttsState = $state<TtsState>('idle');
let ttsActiveMessageId = $state<string | null>(null);
let micError = $state<string | null>(null);
let playbackPollInterval: ReturnType<typeof setInterval> | null = null;
let activeRecordingSessionId = $state<string | null>(null);
let readyTransitionTimeout: ReturnType<typeof setTimeout> | null = null;
let readyFallbackTimeout: ReturnType<typeof setTimeout> | null = null;

function clearReadyTimers(): void {
	if (readyTransitionTimeout !== null) {
		clearTimeout(readyTransitionTimeout);
		readyTransitionTimeout = null;
	}

	if (readyFallbackTimeout !== null) {
		clearTimeout(readyFallbackTimeout);
		readyFallbackTimeout = null;
	}
}

function resetRecordingSession(): void {
	activeRecordingSessionId = null;
	clearReadyTimers();
}

function startReadyFallbackTimer(sessionId: string): void {
	if (readyFallbackTimeout !== null) {
		clearTimeout(readyFallbackTimeout);
	}

	readyFallbackTimeout = setTimeout(() => {
		if (activeRecordingSessionId !== sessionId || micState !== 'arming') {
			return;
		}

		console.warn(
			`audio-recording-ready event missing for session ${sessionId}; promoting mic to recording`
		);
		readyFallbackTimeout = null;
		micState = 'recording';
	}, 2000);
}

function startReadyTransitionTimer(sessionId: string): void {
	if (readyTransitionTimeout !== null) {
		clearTimeout(readyTransitionTimeout);
	}

	readyTransitionTimeout = setTimeout(() => {
		if (activeRecordingSessionId !== sessionId || micState !== 'ready') {
			return;
		}

		readyTransitionTimeout = null;
		micState = 'recording';
	}, 1200);
}

// ── Mic actions ──────────────────────────────────────────────────────

async function startRecording(): Promise<void> {
	if (micState !== 'idle') return;

	const sessionId = crypto.randomUUID();

	try {
		micError = null;
		activeRecordingSessionId = sessionId;
		clearReadyTimers();
		micState = 'arming';
		await invoke('start_recording', { sessionId });
		if (activeRecordingSessionId === sessionId && micState === 'arming') {
			startReadyFallbackTimer(sessionId);
		}
	} catch (e) {
		const msg = String(e);
		console.error('start_recording failed:', msg);
		micError = msg;
		resetRecordingSession();
		if (msg.includes('No microphone') || msg.includes('microphone')) {
			micState = 'disabled';
		} else {
			micState = 'idle';
		}
	}
}

async function stopRecording(): Promise<string> {
	if (micState !== 'ready' && micState !== 'recording') return '';

	clearReadyTimers();
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
		resetRecordingSession();
		micState = 'idle';
	}
}

function handleRecordingReady(event: AudioRecordingReadyEvent): void {
	if (micState !== 'arming') {
		return;
	}

	if (activeRecordingSessionId !== event.sessionId) {
		return;
	}

	clearReadyTimers();
	micState = 'ready';
	startReadyTransitionTimer(event.sessionId);
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
	get activeRecordingSessionId() {
		return activeRecordingSessionId;
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
		return (
			micState === 'arming' ||
			micState === 'ready' ||
			micState === 'recording' ||
			micState === 'processing'
		);
	},
	startRecording,
	stopRecording,
	handleRecordingReady,
	speakMessage,
	stopPlayback
};
