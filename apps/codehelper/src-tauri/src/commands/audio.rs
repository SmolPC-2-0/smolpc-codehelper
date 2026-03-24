use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Maximum recording length in samples (30 seconds at 48kHz stereo).
/// Prevents unbounded memory growth on 8GB target hardware.
const MAX_RECORDING_SAMPLES: usize = 48_000 * 2 * 30;

// ── AudioState ───────────────────────────────────────────────────────

/// Managed state for audio recording and playback.
///
/// Uses `std::sync::Mutex` (not tokio) because cpal audio callbacks are
/// synchronous and must lock the buffer without `.await`.
pub struct AudioState {
    recording: std::sync::Mutex<Option<RecordingSession>>,
    playback: std::sync::Mutex<Option<PlaybackHandle>>,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            recording: std::sync::Mutex::new(None),
            playback: std::sync::Mutex::new(None),
        }
    }
}

struct RecordingSession {
    _stream: cpal::Stream, // Held to keep recording alive; dropping stops capture.
    buffer: Arc<std::sync::Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

struct PlaybackHandle {
    stop_signal: Arc<AtomicBool>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

// ── Commands ─────────────────────────────────────────────────────────

/// [I1 fix] start_recording uses a narrow lock scope: check-then-build-then-set
/// to avoid holding the mutex across cpal device enumeration (which involves
/// Windows COM/WASAPI calls that can take 50-200ms).
#[tauri::command]
pub async fn start_recording(
    state: tauri::State<'_, AudioState>,
) -> Result<(), String> {
    // Quick check — don't hold lock during device enumeration.
    {
        let recording = state
            .recording
            .lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        if recording.is_some() {
            return Err("Already recording".to_string());
        }
    }

    // Build stream without holding the lock.
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("No microphone detected")?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("Failed to get input config: {e}"))?;

    let sample_rate = config.sample_rate();
    let channels = config.channels();
    let buffer: Arc<std::sync::Mutex<Vec<f32>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let buffer_clone = Arc::clone(&buffer);

    // [C2 fix] Cap recording buffer at MAX_RECORDING_SAMPLES to prevent
    // unbounded memory growth if a student leaves recording on.
    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Ok(mut buf) = buffer_clone.lock() {
                    let remaining = MAX_RECORDING_SAMPLES.saturating_sub(buf.len());
                    if remaining > 0 {
                        let to_copy = data.len().min(remaining);
                        buf.extend_from_slice(&data[..to_copy]);
                    }
                }
            },
            |err| {
                log::error!("Audio capture error: {err}");
            },
            None,
        )
        .map_err(|e| format!("Failed to build input stream: {e}"))?;

    stream
        .play()
        .map_err(|e| format!("Failed to start recording: {e}"))?;

    let session = RecordingSession {
        _stream: stream,
        buffer,
        sample_rate,
        channels,
    };

    // Re-acquire lock and set — TOCTOU double-check catches concurrent callers.
    {
        let mut recording = state
            .recording
            .lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        if recording.is_some() {
            // Another caller won the race — drop our session.
            return Err("Recording started by another caller".to_string());
        }
        *recording = Some(session);
    }

    log::info!("Recording started: {sample_rate}Hz, {channels}ch");
    Ok(())
}

#[tauri::command]
pub async fn stop_recording(
    audio_state: tauri::State<'_, AudioState>,
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<String, String> {
    // Take the recording session (dropping the stream stops capture).
    let session = {
        let mut recording = audio_state
            .recording
            .lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        recording.take().ok_or("Not recording")?
    };

    let raw_samples = {
        let buf = session
            .buffer
            .lock()
            .map_err(|e| format!("Buffer lock poisoned: {e}"))?;
        buf.clone()
    };

    if raw_samples.is_empty() {
        return Ok(String::new());
    }

    // Mono mix if stereo.
    let mono_samples = if session.channels > 1 {
        mono_mix(&raw_samples, session.channels)
    } else {
        raw_samples
    };

    // Resample to 16kHz.
    let resampled = resample_to_16k(&mono_samples, session.sample_rate)?;

    // POST to engine.
    let client = supervisor.get_client(Duration::from_secs(60)).await?;
    let text = client
        .transcribe(&resampled)
        .await
        .map_err(|e| format!("Transcription failed: {e}"))?;

    log::info!("Transcription result: {} chars", text.len());
    Ok(text)
}

/// [I4 fix] speak_text uses a oneshot channel to report playback initialization
/// failures from the dedicated thread back to the async caller. The frontend
/// gets a proper error instead of a silent Ok(()) followed by is_playing()=false.
#[tauri::command]
pub async fn speak_text(
    text: String,
    voice: Option<String>,
    audio_state: tauri::State<'_, AudioState>,
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<(), String> {
    // [C1 fix] Use async stop to avoid blocking tokio with jh.join().
    stop_playback_async(&audio_state).await?;

    // Get WAV bytes from engine.
    let client = supervisor.get_client(Duration::from_secs(60)).await?;
    let wav_bytes = client
        .speak(&text, voice.as_deref())
        .await
        .map_err(|e| format!("TTS failed: {e}"))?;

    // [I4 fix] Oneshot channel: playback thread reports init success/failure.
    let (init_tx, init_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();

    // Play on a dedicated thread (rodio::OutputStream is !Send on some platforms).
    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop_signal);

    let join_handle = std::thread::spawn(move || {
        let (_stream, stream_handle) = match rodio::OutputStream::try_default() {
            Ok(s) => {
                // Signal success before starting playback.
                let _ = init_tx.send(Ok(()));
                s
            }
            Err(e) => {
                let _ = init_tx.send(Err(format!("Failed to open audio output: {e}")));
                return;
            }
        };
        let sink = match rodio::Sink::try_new(&stream_handle) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to create audio sink: {e}");
                return;
            }
        };

        let cursor = std::io::Cursor::new(wav_bytes);
        match rodio::Decoder::new(cursor) {
            Ok(source) => sink.append(source),
            Err(e) => {
                log::error!("Failed to decode WAV: {e}");
                return;
            }
        }

        // Wait for playback to finish or stop signal.
        while !sink.empty() && !stop_clone.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(50));
        }
        if stop_clone.load(Ordering::Relaxed) {
            sink.stop();
        }
    });

    // Wait for the init result from the playback thread.
    init_rx
        .await
        .map_err(|_| "Playback thread exited before initialization".to_string())??;

    let mut playback = audio_state
        .playback
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    *playback = Some(PlaybackHandle {
        stop_signal,
        join_handle: Some(join_handle),
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_playback(
    state: tauri::State<'_, AudioState>,
) -> Result<(), String> {
    stop_playback_async(&state).await
}

#[tauri::command]
pub fn is_playing(
    state: tauri::State<'_, AudioState>,
) -> Result<bool, String> {
    let playback = state
        .playback
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    match &*playback {
        Some(handle) => {
            // If the thread has finished, playback is done.
            let thread_alive = handle
                .join_handle
                .as_ref()
                .map(|jh| !jh.is_finished())
                .unwrap_or(false);
            Ok(thread_alive && !handle.stop_signal.load(Ordering::Relaxed))
        }
        None => Ok(false),
    }
}

/// [C1 fix] Async-safe stop that uses spawn_blocking for the thread join,
/// preventing tokio worker starvation. The stop signal is set immediately
/// (fast), then the blocking join runs on the tokio blocking thread pool.
async fn stop_playback_async(state: &AudioState) -> Result<(), String> {
    let handle = {
        let mut playback = state
            .playback
            .lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        playback.take()
    };
    if let Some(mut handle) = handle {
        // Signal stop immediately — the playback loop checks every 50ms.
        handle.stop_signal.store(true, Ordering::Relaxed);
        if let Some(jh) = handle.join_handle.take() {
            // Join on the blocking pool so we don't stall an async worker.
            let _ = tokio::task::spawn_blocking(move || jh.join()).await;
        }
    }
    Ok(())
}

/// Synchronous stop for use in non-async contexts (e.g., app exit cleanup).
pub fn stop_playback_sync(state: &AudioState) {
    let handle = {
        let Ok(mut playback) = state.playback.lock() else {
            return;
        };
        playback.take()
    };
    if let Some(mut handle) = handle {
        handle.stop_signal.store(true, Ordering::Relaxed);
        if let Some(jh) = handle.join_handle.take() {
            let _ = jh.join();
        }
    }
}

/// Stop any active recording. For use in app exit cleanup.
pub fn stop_recording_sync(state: &AudioState) {
    let Ok(mut recording) = state.recording.lock() else {
        return;
    };
    // Dropping the RecordingSession stops the cpal stream.
    recording.take();
}

// ── Resampling helpers ───────────────────────────────────────────────

fn mono_mix(samples: &[f32], channels: u16) -> Vec<f32> {
    let ch = channels as usize;
    samples
        .chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

fn resample_to_16k(samples: &[f32], source_rate: u32) -> Result<Vec<f32>, String> {
    const TARGET_RATE: usize = 16_000;
    let source = source_rate as usize;

    if source == TARGET_RATE {
        return Ok(samples.to_vec());
    }

    use rubato::{Fft, FixedSync, Resampler};

    let chunk_size = 1024;
    let sub_chunks = 2;
    let channels = 1; // mono

    let mut resampler = Fft::<f32>::new(
        source,
        TARGET_RATE,
        chunk_size,
        sub_chunks,
        channels,
        FixedSync::Input,
    )
    .map_err(|e| format!("Failed to create resampler: {e}"))?;

    // Allocate output buffer using rubato's official sizing API.
    let output_len = resampler.process_all_needed_output_len(samples.len());
    let mut output = vec![0.0f32; output_len];

    // Use process_all_into_buffer for simplicity.
    use audioadapter_buffers::direct::InterleavedSlice;
    let input_adapter =
        InterleavedSlice::new(samples, channels, samples.len()).map_err(|e| format!("{e}"))?;
    let output_len = output.len();
    let mut output_adapter = InterleavedSlice::new_mut(&mut output, channels, output_len)
        .map_err(|e| format!("{e}"))?;

    let (_nbr_in, nbr_out) = resampler
        .process_all_into_buffer(&input_adapter, &mut output_adapter, samples.len(), None)
        .map_err(|e| format!("Resample error: {e}"))?;

    output.truncate(nbr_out);
    Ok(output)
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_mix_stereo_to_mono() {
        // Stereo: L=1.0, R=0.0, L=0.5, R=0.5
        let stereo = vec![1.0f32, 0.0, 0.5, 0.5];
        let mono = mono_mix(&stereo, 2);
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.5).abs() < 1e-6);
        assert!((mono[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn resample_passthrough_at_16k() {
        let samples = vec![0.1f32; 1600]; // 0.1s at 16kHz
        let result = resample_to_16k(&samples, 16000).unwrap();
        assert_eq!(result.len(), 1600);
        assert_eq!(result, samples);
    }

    #[test]
    fn resample_48k_to_16k_reduces_samples() {
        // 4800 samples at 48kHz = 0.1s → should produce ~1600 samples at 16kHz
        let samples = vec![0.0f32; 4800];
        let result = resample_to_16k(&samples, 48000).unwrap();
        // Allow some tolerance for resampler edge effects
        assert!(
            result.len() >= 1500 && result.len() <= 1700,
            "Expected ~1600 samples, got {}",
            result.len()
        );
    }

    #[test]
    fn resample_44100_to_16k_reduces_samples() {
        // 4410 samples at 44.1kHz = 0.1s → should produce ~1600 samples at 16kHz
        let samples = vec![0.0f32; 4410];
        let result = resample_to_16k(&samples, 44100).unwrap();
        assert!(
            result.len() >= 1500 && result.len() <= 1700,
            "Expected ~1600 samples, got {}",
            result.len()
        );
    }

    #[test]
    fn recording_buffer_cap_constant_is_sane() {
        // 30s at 48kHz stereo = 2,880,000 samples * 4 bytes = ~11 MB
        assert_eq!(MAX_RECORDING_SAMPLES, 2_880_000);
        let bytes = MAX_RECORDING_SAMPLES * std::mem::size_of::<f32>();
        assert!(bytes < 12_000_000, "Cap should be under 12 MB");
    }
}
