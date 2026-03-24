use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

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

#[tauri::command]
pub async fn start_recording(
    state: tauri::State<'_, AudioState>,
) -> Result<(), String> {
    let mut recording = state
        .recording
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;

    if recording.is_some() {
        return Err("Already recording".to_string());
    }

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

    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Ok(mut buf) = buffer_clone.lock() {
                    buf.extend_from_slice(data);
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

    *recording = Some(RecordingSession {
        _stream: stream,
        buffer,
        sample_rate,
        channels,
    });

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
    let client = supervisor
        .get_client(Duration::from_secs(60))
        .await?;
    let text = client
        .transcribe(&resampled)
        .await
        .map_err(|e| format!("Transcription failed: {e}"))?;

    log::info!("Transcription result: {} chars", text.len());
    Ok(text)
}

#[tauri::command]
pub async fn speak_text(
    text: String,
    voice: Option<String>,
    audio_state: tauri::State<'_, AudioState>,
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<(), String> {
    // Stop any existing playback.
    stop_playback_inner(&audio_state)?;

    // Get WAV bytes from engine.
    let client = supervisor
        .get_client(Duration::from_secs(60))
        .await?;
    let wav_bytes = client
        .speak(&text, voice.as_deref())
        .await
        .map_err(|e| format!("TTS failed: {e}"))?;

    // Play on a dedicated thread (rodio::OutputStream is !Send on some platforms).
    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop_signal);

    let join_handle = std::thread::spawn(move || {
        let Ok((_stream, stream_handle)) = rodio::OutputStream::try_default() else {
            log::error!("Failed to open audio output");
            return;
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
pub fn stop_playback(
    state: tauri::State<'_, AudioState>,
) -> Result<(), String> {
    stop_playback_inner(&state)
}

fn stop_playback_inner(state: &AudioState) -> Result<(), String> {
    let mut playback = state
        .playback
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    if let Some(mut handle) = playback.take() {
        handle.stop_signal.store(true, Ordering::Relaxed);
        if let Some(jh) = handle.join_handle.take() {
            let _ = jh.join();
        }
    }
    Ok(())
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

    // Allocate output buffer — estimate needed size with margin.
    let ratio = TARGET_RATE as f64 / source as f64;
    let estimated_output = (samples.len() as f64 * ratio) as usize + chunk_size;
    let mut output = vec![0.0f32; estimated_output];

    // Use process_all_into_buffer for simplicity.
    use audioadapter_buffers::direct::InterleavedSlice;
    let input_adapter =
        InterleavedSlice::new(samples, channels, samples.len()).map_err(|e| format!("{e}"))?;
    let output_len = output.len();
    let mut output_adapter =
        InterleavedSlice::new_mut(&mut output, channels, output_len).map_err(|e| format!("{e}"))?;

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
}
