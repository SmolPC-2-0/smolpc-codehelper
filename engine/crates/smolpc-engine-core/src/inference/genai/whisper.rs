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

        let mut results_ptr: *mut OvGenAiWhisperDecodedResults = ptr::null_mut();
        let status = unsafe {
            (api.generate)(
                pipeline,
                audio.as_ptr(),
                audio.len(),
                ptr::null(), // config = null → use defaults (English, transcribe)
                &mut results_ptr,
            )
        };
        check_whisper_status(api, status, "whisper_pipeline_generate")?;

        if results_ptr.is_null() {
            return Err("whisper_pipeline_generate returned null results".to_string());
        }

        // Extract transcribed text from results.
        let text = unsafe { extract_text(api, results_ptr) };

        // Always free results, even if extraction failed.
        unsafe { (api.results_free)(results_ptr) };

        text
    }
}

/// Extract the first text segment from WhisperDecodedResults.
unsafe fn extract_text(
    api: &WhisperApi,
    results: *const OvGenAiWhisperDecodedResults,
) -> Result<String, String> {
    let mut count: usize = 0;
    let status = (api.results_get_texts_size)(results, &mut count);
    check_whisper_status(api, status, "whisper_results_get_texts_size")?;

    if count == 0 {
        return Ok(String::new());
    }

    // Concatenate all text segments (typically just one for short audio).
    let mut full_text = String::new();
    for i in 0..count {
        let mut text_ptr: *const std::ffi::c_char = ptr::null();
        let status = (api.results_get_text)(results, i, &mut text_ptr);
        check_whisper_status(api, status, &format!("whisper_results_get_text[{i}]"))?;

        if !text_ptr.is_null() {
            let segment = c_string_to_string(text_ptr);
            if !segment.is_empty() {
                if !full_text.is_empty() {
                    full_text.push(' ');
                }
                full_text.push_str(&segment);
            }
        }
    }

    Ok(full_text)
}
