use super::super::runtime_loading::{
    OpenVinoRuntimeBundle, OpenVinoRuntimeLoader, RetainedLibrary,
};
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::Path;
use std::ptr;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub(super) type OvGenAiLlmPipeline = c_void;
pub(super) type OvGenAiGenerationConfig = c_void;
pub(super) type OvGenAiDecodedResults = c_void;
pub(super) type OvGenAiPerfMetrics = c_void;
pub(super) type OvGenAiChatHistory = c_void;
pub(super) type OvGenAiJsonContainer = c_void;

pub(super) type OvStatus = i32;
pub(super) const OV_STATUS_OK: OvStatus = 0;
pub(super) static PRESENCE_PENALTY_SYMBOL_WARNING_EMITTED: AtomicBool = AtomicBool::new(false);

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum OvGenAiStreamingStatus {
    Running = 0,
    Stop = 1,
    Cancel = 2,
}

#[repr(C)]
pub(super) struct StreamerCallback {
    pub(super) callback_func:
        Option<unsafe extern "C" fn(*const c_char, *mut c_void) -> OvGenAiStreamingStatus>,
    pub(super) args: *mut c_void,
}

pub(super) struct OpenVinoGenAiApi {
    pub(super) _openvino_c: RetainedLibrary,
    pub(super) _openvino_genai_c: RetainedLibrary,
    pub(super) get_error_info: unsafe extern "C" fn(OvStatus) -> *const c_char,
    pub(super) get_last_err_msg: unsafe extern "C" fn() -> *const c_char,
    pub(super) create_pipeline: unsafe extern "C" fn(
        *const c_char,
        *const c_char,
        usize,
        *mut *mut OvGenAiLlmPipeline,
        ...
    ) -> OvStatus,
    pub(super) destroy_pipeline: unsafe extern "C" fn(*mut OvGenAiLlmPipeline),
    pub(super) pipeline_generate: unsafe extern "C" fn(
        *mut OvGenAiLlmPipeline,
        *const c_char,
        *const OvGenAiGenerationConfig,
        *const StreamerCallback,
        *mut *mut OvGenAiDecodedResults,
    ) -> OvStatus,
    pub(super) pipeline_generate_with_history: unsafe extern "C" fn(
        *mut OvGenAiLlmPipeline,
        *const OvGenAiChatHistory,
        *const OvGenAiGenerationConfig,
        *const StreamerCallback,
        *mut *mut OvGenAiDecodedResults,
    ) -> OvStatus,
    pub(super) pipeline_get_generation_config: unsafe extern "C" fn(
        *const OvGenAiLlmPipeline,
        *mut *mut OvGenAiGenerationConfig,
    ) -> OvStatus,
    pub(super) create_json_container_from_json_string:
        unsafe extern "C" fn(*mut *mut OvGenAiJsonContainer, *const c_char) -> OvStatus,
    pub(super) destroy_json_container: unsafe extern "C" fn(*mut OvGenAiJsonContainer),
    pub(super) create_chat_history_from_json_container:
        unsafe extern "C" fn(*mut *mut OvGenAiChatHistory, *const OvGenAiJsonContainer) -> OvStatus,
    pub(super) set_chat_history_extra_context:
        unsafe extern "C" fn(*mut OvGenAiChatHistory, *const OvGenAiJsonContainer) -> OvStatus,
    pub(super) destroy_chat_history: unsafe extern "C" fn(*mut OvGenAiChatHistory),
    pub(super) create_generation_config:
        unsafe extern "C" fn(*mut *mut OvGenAiGenerationConfig) -> OvStatus,
    pub(super) destroy_generation_config: unsafe extern "C" fn(*mut OvGenAiGenerationConfig),
    pub(super) set_max_new_tokens:
        unsafe extern "C" fn(*mut OvGenAiGenerationConfig, usize) -> OvStatus,
    pub(super) set_eos_token_id:
        unsafe extern "C" fn(*mut OvGenAiGenerationConfig, i64) -> OvStatus,
    pub(super) set_min_new_tokens:
        unsafe extern "C" fn(*mut OvGenAiGenerationConfig, usize) -> OvStatus,
    pub(super) set_stop_token_ids:
        unsafe extern "C" fn(*mut OvGenAiGenerationConfig, *const i64, usize) -> OvStatus,
    pub(super) set_stop_strings:
        unsafe extern "C" fn(*mut OvGenAiGenerationConfig, *const *const c_char, usize) -> OvStatus,
    pub(super) set_ignore_eos: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, bool) -> OvStatus,
    pub(super) set_echo: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, bool) -> OvStatus,
    pub(super) set_do_sample: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, bool) -> OvStatus,
    pub(super) set_temperature: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, f32) -> OvStatus,
    pub(super) set_top_p: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, f32) -> OvStatus,
    pub(super) set_top_k: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, usize) -> OvStatus,
    pub(super) set_repetition_penalty:
        unsafe extern "C" fn(*mut OvGenAiGenerationConfig, f32) -> OvStatus,
    pub(super) set_presence_penalty:
        Option<unsafe extern "C" fn(*mut OvGenAiGenerationConfig, f32) -> OvStatus>,
    pub(super) validate_generation_config:
        unsafe extern "C" fn(*mut OvGenAiGenerationConfig) -> OvStatus,
    pub(super) destroy_decoded_results: unsafe extern "C" fn(*mut OvGenAiDecodedResults),
    pub(super) get_perf_metrics: unsafe extern "C" fn(
        *const OvGenAiDecodedResults,
        *mut *mut OvGenAiPerfMetrics,
    ) -> OvStatus,
    pub(super) get_num_generation_tokens:
        unsafe extern "C" fn(*const OvGenAiPerfMetrics, *mut usize) -> OvStatus,
    pub(super) get_ttft:
        unsafe extern "C" fn(*const OvGenAiPerfMetrics, *mut f32, *mut f32) -> OvStatus,
    pub(super) get_throughput:
        unsafe extern "C" fn(*const OvGenAiPerfMetrics, *mut f32, *mut f32) -> OvStatus,
    pub(super) get_generate_duration:
        unsafe extern "C" fn(*const OvGenAiPerfMetrics, *mut f32, *mut f32) -> OvStatus,
}

unsafe impl Send for OpenVinoGenAiApi {}
unsafe impl Sync for OpenVinoGenAiApi {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OpenVinoDeviceTarget {
    Cpu,
    Npu,
}

impl OpenVinoDeviceTarget {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Npu => "NPU",
        }
    }
}

impl OpenVinoGenAiApi {
    pub(super) fn load(
        bundle: &OpenVinoRuntimeBundle,
        target: OpenVinoDeviceTarget,
    ) -> Result<Arc<Self>, String> {
        match target {
            OpenVinoDeviceTarget::Cpu => OpenVinoRuntimeLoader::ensure_initialized_for_cpu(bundle)?,
            OpenVinoDeviceTarget::Npu => OpenVinoRuntimeLoader::ensure_initialized_for_npu(bundle)?,
        };
        let openvino_c = RetainedLibrary::load(&bundle.openvino_c_dll)?;
        let openvino_genai_c = RetainedLibrary::load(&bundle.openvino_genai_c_dll)?;

        unsafe {
            Ok(Arc::new(Self {
                get_error_info: load_symbol(&openvino_c, b"ov_get_error_info\0")?,
                get_last_err_msg: load_symbol(&openvino_c, b"ov_get_last_err_msg\0")?,
                create_pipeline: load_symbol(&openvino_genai_c, b"ov_genai_llm_pipeline_create\0")?,
                destroy_pipeline: load_symbol(&openvino_genai_c, b"ov_genai_llm_pipeline_free\0")?,
                pipeline_generate: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_llm_pipeline_generate\0",
                )?,
                pipeline_generate_with_history: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_llm_pipeline_generate_with_history\0",
                )?,
                pipeline_get_generation_config: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_llm_pipeline_get_generation_config\0",
                )?,
                create_json_container_from_json_string: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_json_container_create_from_json_string\0",
                )?,
                destroy_json_container: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_json_container_free\0",
                )?,
                create_chat_history_from_json_container: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_chat_history_create_from_json_container\0",
                )?,
                set_chat_history_extra_context: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_chat_history_set_extra_context\0",
                )?,
                destroy_chat_history: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_chat_history_free\0",
                )?,
                create_generation_config: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_create\0",
                )?,
                destroy_generation_config: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_free\0",
                )?,
                set_max_new_tokens: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_set_max_new_tokens\0",
                )?,
                set_eos_token_id: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_set_eos_token_id\0",
                )?,
                set_min_new_tokens: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_set_min_new_tokens\0",
                )?,
                set_stop_token_ids: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_set_stop_token_ids\0",
                )?,
                set_stop_strings: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_set_stop_strings\0",
                )?,
                set_ignore_eos: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_set_ignore_eos\0",
                )?,
                set_echo: load_symbol(&openvino_genai_c, b"ov_genai_generation_config_set_echo\0")?,
                set_do_sample: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_set_do_sample\0",
                )?,
                set_temperature: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_set_temperature\0",
                )?,
                set_top_p: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_set_top_p\0",
                )?,
                set_top_k: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_set_top_k\0",
                )?,
                set_repetition_penalty: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_set_repetition_penalty\0",
                )?,
                set_presence_penalty: try_load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_set_presence_penalty\0",
                ),
                validate_generation_config: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_generation_config_validate\0",
                )?,
                destroy_decoded_results: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_decoded_results_free\0",
                )?,
                get_perf_metrics: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_decoded_results_get_perf_metrics\0",
                )?,
                get_num_generation_tokens: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_perf_metrics_get_num_generation_tokens\0",
                )?,
                get_ttft: load_symbol(&openvino_genai_c, b"ov_genai_perf_metrics_get_ttft\0")?,
                get_throughput: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_perf_metrics_get_throughput\0",
                )?,
                get_generate_duration: load_symbol(
                    &openvino_genai_c,
                    b"ov_genai_perf_metrics_get_generate_duration\0",
                )?,
                _openvino_c: openvino_c,
                _openvino_genai_c: openvino_genai_c,
            }))
        }
    }
}

pub(super) unsafe fn load_symbol<T: Copy>(lib: &RetainedLibrary, name: &[u8]) -> Result<T, String> {
    lib.get(name)
}

pub(super) unsafe fn try_load_symbol<T: Copy>(lib: &RetainedLibrary, name: &[u8]) -> Option<T> {
    lib.get(name).ok()
}

pub(super) fn cstring(value: &str, field: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| format!("{field} contains interior NUL byte"))
}

pub(super) fn path_to_cstring(path: &Path, field: &str) -> Result<CString, String> {
    let utf8 = path.to_str().ok_or_else(|| {
        format!(
            "Non-UTF8 path is unsupported for OpenVINO GenAI: {}",
            path.display()
        )
    })?;
    cstring(utf8, field)
}

pub(super) fn check_status(
    api: &OpenVinoGenAiApi,
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

pub(super) fn check_plain_status(status: OvStatus, context: &str) -> Result<(), String> {
    if status == OV_STATUS_OK {
        Ok(())
    } else {
        Err(format!("{context}: OpenVINO status code {status}"))
    }
}

pub(super) fn c_string_to_string(value: *const c_char) -> String {
    unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .trim()
        .to_string()
}

pub(super) fn c_string_to_string_verbatim(value: *const c_char) -> String {
    unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned()
}

pub(super) struct OvOwned<T> {
    pub(super) api: Arc<OpenVinoGenAiApi>,
    pub(super) ptr: *mut T,
    pub(super) destroy: unsafe extern "C" fn(*mut T),
}

impl<T> OvOwned<T> {
    pub(super) fn new(
        api: Arc<OpenVinoGenAiApi>,
        ptr: *mut T,
        destroy: unsafe extern "C" fn(*mut T),
    ) -> Self {
        Self { api, ptr, destroy }
    }

    pub(super) fn as_ptr(&self) -> *mut T {
        self.ptr
    }
}

impl<T> Drop for OvOwned<T> {
    fn drop(&mut self) {
        let ptr = std::mem::replace(&mut self.ptr, ptr::null_mut());
        if ptr.is_null() {
            return;
        }
        unsafe { (self.destroy)(ptr) };
        let _ = &self.api;
    }
}

#[derive(Clone)]
pub(super) enum CachedOpenVinoGenAiApi {
    Success(Arc<OpenVinoGenAiApi>),
    Failure(String),
}

#[derive(Default)]
pub(super) struct OpenVinoGenAiApiState {
    pub(super) active_fingerprint: Option<String>,
    pub(super) results: HashMap<String, CachedOpenVinoGenAiApi>,
}

pub(super) static OPENVINO_GENAI_API: std::sync::OnceLock<Mutex<OpenVinoGenAiApiState>> =
    std::sync::OnceLock::new();

pub(super) fn openvino_genai_api_for_target(
    bundle: &OpenVinoRuntimeBundle,
    target: OpenVinoDeviceTarget,
) -> Result<Arc<OpenVinoGenAiApi>, String> {
    let state = OPENVINO_GENAI_API.get_or_init(|| Mutex::new(OpenVinoGenAiApiState::default()));
    let fingerprint = bundle.fingerprint.value.clone();
    let cache_key = format!("{fingerprint}:{}", target.as_str().to_ascii_lowercase());

    {
        let guard = match state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("Recovering from poisoned OpenVINO GenAI API cache mutex");
                poisoned.into_inner()
            }
        };

        if let Some(cached) = guard.results.get(&cache_key) {
            return match cached {
                CachedOpenVinoGenAiApi::Success(api) => Ok(Arc::clone(api)),
                CachedOpenVinoGenAiApi::Failure(error) => Err(error.clone()),
            };
        }

        if let Some(active) = guard.active_fingerprint.as_ref() {
            if active != &fingerprint {
                let error = format!(
                    "OpenVINO GenAI already initialized from bundle fingerprint '{active}'; restart the process to use '{fingerprint}'"
                );
                drop(guard);
                let mut guard = match state.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                guard
                    .results
                    .insert(cache_key, CachedOpenVinoGenAiApi::Failure(error.clone()));
                return Err(error);
            }
        }
    }

    let validation_failure = match target {
        OpenVinoDeviceTarget::Cpu => bundle.cpu_validation_failure,
        OpenVinoDeviceTarget::Npu => bundle.npu_validation_failure,
    };
    if let Some(failure) = validation_failure {
        let error = format!(
            "OpenVINO runtime bundle is not validated ({}) at {}",
            failure.code(),
            bundle.display_root().display()
        );
        let mut guard = match state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard
            .results
            .insert(cache_key, CachedOpenVinoGenAiApi::Failure(error.clone()));
        return Err(error);
    }

    let api = OpenVinoGenAiApi::load(bundle, target)?;
    let mut guard = match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.active_fingerprint = Some(fingerprint.clone());
    guard
        .results
        .insert(cache_key, CachedOpenVinoGenAiApi::Success(Arc::clone(&api)));
    Ok(api)
}
