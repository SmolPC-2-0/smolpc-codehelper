use super::super::runtime_loading::OpenVinoRuntimeBundle;
use super::openvino_ffi::{c_string_to_string, cstring, path_to_cstring};
use super::whisper_ffi::{
    check_whisper_status, whisper_api_for_bundle, OvGenAiWhisperDecodedResults,
    OvGenAiWhisperPipeline, WhisperApi,
};
use std::path::Path;
use std::ptr;
use std::sync::Arc;

// ── Inner struct with Drop ───────────────────────────────────────────

struct WhisperPipelineInner {
    api: Arc<WhisperApi>,
    pipeline: *mut OvGenAiWhisperPipeline,
}

unsafe impl Send for WhisperPipelineInner {}

impl Drop for WhisperPipelineInner {
    fn drop(&mut self) {
        if !self.pipeline.is_null() {
            unsafe { (self.api.free_pipeline)(self.pipeline) };
            self.pipeline = ptr::null_mut();
        }
    }
}

// ── Public API ───────────────────────────────────────────────────────

/// OpenVINO GenAI WhisperPipeline for speech-to-text transcription.
///
/// Always runs on CPU. Lazy-loaded on first transcription request.
/// Thread-safe via Mutex wrapping at the caller level.
pub struct WhisperPipeline {
    inner: WhisperPipelineInner,
}

impl WhisperPipeline {
    /// Create a WhisperPipeline from an OpenVINO IR model directory.
    ///
    /// The model directory must contain the flat layout from
    /// `OpenVINO/whisper-base.en-int8-ov`: `openvino_encoder_model.*`,
    /// `openvino_decoder_model.*`, tokenizer files, and configs.
    ///
    /// Always targets CPU device. This is a blocking call (~1s cold start).
    pub fn new(bundle: &OpenVinoRuntimeBundle, model_dir: &Path) -> Result<Self, String> {
        let api = whisper_api_for_bundle(bundle)?;
        let model_path_c = path_to_cstring(model_dir, "whisper model_dir")?;
        let device_c = cstring("CPU", "whisper device")?;

        let mut pipeline_ptr: *mut OvGenAiWhisperPipeline = ptr::null_mut();
        let status = unsafe {
            (api.create_pipeline)(
                model_path_c.as_ptr(),
                device_c.as_ptr(),
                0, // property_args_size = 0 (use defaults)
                &mut pipeline_ptr,
            )
        };
        check_whisper_status(&api, status, "whisper_pipeline_create")?;

        if pipeline_ptr.is_null() {
            return Err("whisper_pipeline_create returned null pipeline".to_string());
        }

        Ok(Self {
            inner: WhisperPipelineInner {
                api,
                pipeline: pipeline_ptr,
            },
        })
    }

    /// Transcribe 16kHz mono f32 audio samples.
    ///
    /// This is a **blocking** CPU-bound call — wrap in `spawn_blocking`.
    /// Returns the transcribed text (may be empty for silence).
    pub fn transcribe(&self, audio: &[f32]) -> Result<String, String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let api = &self.inner.api;
        let pipeline = self.inner.pipeline;

        log::info!("whisper: calling generate with {} samples (null config = defaults)", audio.len());

        let mut results_ptr: *mut OvGenAiWhisperDecodedResults = ptr::null_mut();
        let status = unsafe {
            (api.generate)(
                pipeline,
                audio.as_ptr(),
                audio.len(),
                ptr::null(), // null config = use defaults (confirmed safe by C API source)
                &mut results_ptr,
            )
        };

        log::info!("whisper: generate returned status {status}");
        check_whisper_status(api, status, "whisper_pipeline_generate")?;

        if results_ptr.is_null() {
            return Err("whisper_pipeline_generate returned null results".to_string());
        }

        // RAII guard ensures results_free is always called, even on early ? returns.
        struct ResultsGuard<'a> {
            api: &'a WhisperApi,
            ptr: *mut OvGenAiWhisperDecodedResults,
        }
        impl Drop for ResultsGuard<'_> {
            fn drop(&mut self) {
                if !self.ptr.is_null() {
                    unsafe { (self.api.results_free)(self.ptr) };
                }
            }
        }
        let _guard = ResultsGuard { api, ptr: results_ptr };

        // Extract text using the two-call buffer pattern:
        // 1) Call with null output to get required size
        // 2) Allocate buffer, call again to get the string
        log::info!("whisper: extracting text from results via get_string (two-call pattern)");

        let text = unsafe {
            let mut size: usize = 0;
            let status = (api.results_get_string)(results_ptr, ptr::null_mut(), &mut size);
            check_whisper_status(api, status, "whisper_results_get_string (size query)")?;

            if size == 0 {
                String::new()
            } else {
                let mut buf = vec![0u8; size];
                let status = (api.results_get_string)(results_ptr, buf.as_mut_ptr(), &mut size);
                check_whisper_status(api, status, "whisper_results_get_string (fill)")?;

                if let Some(nul_pos) = buf.iter().position(|&b| b == 0) {
                    buf.truncate(nul_pos);
                }
                String::from_utf8_lossy(&buf).trim().to_string()
            }
        };
        // _guard drops here, calling results_free

        log::info!("whisper: transcription complete, {} chars: {:?}",
            text.len(),
            if text.len() > 80 { &text[..80] } else { &text });
        Ok(text)
    }
}
