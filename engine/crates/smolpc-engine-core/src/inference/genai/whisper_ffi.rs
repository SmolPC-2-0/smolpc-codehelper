use super::super::runtime_loading::{
    OpenVinoRuntimeBundle, OpenVinoRuntimeLoader, RetainedLibrary,
};
use super::openvino_ffi::{c_string_to_string, load_symbol, OvStatus, OV_STATUS_OK};
use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::sync::{Arc, Mutex, OnceLock};

// ── Opaque pointer types ─────────────────────────────────────────────

pub(super) type OvGenAiWhisperPipeline = c_void;
pub(super) type OvGenAiWhisperGenerationConfig = c_void;
pub(super) type OvGenAiWhisperDecodedResults = c_void;

// ── WhisperApi struct ────────────────────────────────────────────────

pub(super) struct WhisperApi {
    _openvino_c: RetainedLibrary,
    _openvino_genai_c: RetainedLibrary,

    // Error reporting (from openvino_c.dll)
    pub(super) get_error_info: unsafe extern "C" fn(OvStatus) -> *const c_char,
    pub(super) get_last_err_msg: unsafe extern "C" fn() -> *const c_char,

    // Pipeline lifecycle (from openvino_genai_c.dll)
    pub(super) create_pipeline: unsafe extern "C" fn(
        *const c_char,
        *const c_char,
        usize,
        *mut *mut OvGenAiWhisperPipeline,
        ...
    ) -> OvStatus,
    pub(super) free_pipeline: unsafe extern "C" fn(*mut OvGenAiWhisperPipeline),

    // Generation
    pub(super) generate: unsafe extern "C" fn(
        *mut OvGenAiWhisperPipeline,
        *const f32,
        usize,
        *const OvGenAiWhisperGenerationConfig,
        *mut *mut OvGenAiWhisperDecodedResults,
    ) -> OvStatus,

    // Results
    pub(super) results_free: unsafe extern "C" fn(*mut OvGenAiWhisperDecodedResults),
    pub(super) results_get_texts_size:
        unsafe extern "C" fn(*const OvGenAiWhisperDecodedResults, *mut usize) -> OvStatus,
    pub(super) results_get_text: unsafe extern "C" fn(
        *const OvGenAiWhisperDecodedResults,
        usize,
        *mut *const c_char,
    ) -> OvStatus,
}

unsafe impl Send for WhisperApi {}
unsafe impl Sync for WhisperApi {}

// ── Load ─────────────────────────────────────────────────────────────

impl WhisperApi {
    pub(super) fn load(bundle: &OpenVinoRuntimeBundle) -> Result<Arc<Self>, String> {
        // Whisper always runs on CPU.
        OpenVinoRuntimeLoader::ensure_initialized_for_cpu(bundle)?;

        let openvino_c = RetainedLibrary::load(&bundle.openvino_c_dll)?;
        let openvino_genai_c = RetainedLibrary::load(&bundle.openvino_genai_c_dll)?;

        unsafe {
            Ok(Arc::new(Self {
                get_error_info: load_symbol(&openvino_c, b"ov_get_error_info\0")?,
                get_last_err_msg: load_symbol(&openvino_c, b"ov_get_last_err_msg\0")?,

                create_pipeline: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_whisper_pipeline_create\0",
                )?,
                free_pipeline: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_whisper_pipeline_free\0",
                )?,
                generate: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_whisper_pipeline_generate\0",
                )?,
                results_free: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_whisper_decoded_results_free\0",
                )?,
                results_get_texts_size: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_whisper_decoded_results_get_texts_size\0",
                )?,
                results_get_text: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_whisper_decoded_results_get_text\0",
                )?,

                _openvino_c: openvino_c,
                _openvino_genai_c: openvino_genai_c,
            }))
        }
    }
}

// ── Error helper ─────────────────────────────────────────────────────

pub(super) fn check_whisper_status(
    api: &WhisperApi,
    status: OvStatus,
    context: &str,
) -> Result<(), String> {
    if status == OV_STATUS_OK {
        return Ok(());
    }

    let mut details = String::new();
    unsafe {
        let error_info = (api.get_error_info)(status);
        if !error_info.is_null() {
            details.push_str(&c_string_to_string(error_info));
        }

        let last_error = (api.get_last_err_msg)();
        if !last_error.is_null() {
            let last_error = c_string_to_string(last_error);
            if !last_error.is_empty() {
                if !details.is_empty() {
                    details.push_str(": ");
                }
                details.push_str(&last_error);
            }
        }
    }

    if details.is_empty() {
        details = format!("OpenVINO status code {status}");
    }

    Err(format!("{context}: {details}"))
}

// ── Caching ──────────────────────────────────────────────────────────

enum CachedWhisperApi {
    Success(Arc<WhisperApi>),
    Failure(String),
}

#[derive(Default)]
struct WhisperApiState {
    active_fingerprint: Option<String>,
    results: HashMap<String, CachedWhisperApi>,
}

static WHISPER_API: OnceLock<Mutex<WhisperApiState>> = OnceLock::new();

pub(super) fn whisper_api_for_bundle(
    bundle: &OpenVinoRuntimeBundle,
) -> Result<Arc<WhisperApi>, String> {
    let state = WHISPER_API.get_or_init(|| Mutex::new(WhisperApiState::default()));
    let fingerprint = bundle.fingerprint.value.clone();

    {
        let guard = match state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("Recovering from poisoned Whisper API cache mutex");
                poisoned.into_inner()
            }
        };

        if let Some(cached) = guard.results.get(&fingerprint) {
            return match cached {
                CachedWhisperApi::Success(api) => Ok(Arc::clone(api)),
                CachedWhisperApi::Failure(error) => Err(error.clone()),
            };
        }

        if let Some(active) = guard.active_fingerprint.as_ref() {
            if active != &fingerprint {
                let error = format!(
                    "Whisper API already initialized from bundle fingerprint '{active}'; \
                     restart the process to use '{fingerprint}'"
                );
                drop(guard);
                let mut guard = match state.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                guard
                    .results
                    .insert(fingerprint, CachedWhisperApi::Failure(error.clone()));
                return Err(error);
            }
        }
    }

    // Validate bundle has CPU support.
    if let Some(failure) = bundle.cpu_validation_failure {
        let error = format!(
            "OpenVINO runtime bundle CPU is not validated ({}) at {}",
            failure.code(),
            bundle.display_root().display()
        );
        let mut guard = match state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard
            .results
            .insert(fingerprint, CachedWhisperApi::Failure(error.clone()));
        return Err(error);
    }

    let api = WhisperApi::load(bundle)?;
    let mut guard = match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.active_fingerprint = Some(fingerprint.clone());
    guard
        .results
        .insert(fingerprint, CachedWhisperApi::Success(Arc::clone(&api)));
    Ok(api)
}

