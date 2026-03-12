use super::super::runtime_loading::{
    OpenVinoRuntimeBundle, OpenVinoRuntimeLoader, RetainedLibrary,
};
use super::super::types::{GenerationConfig, GenerationMetrics, InferenceChatMessage};
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

type OvGenAiLlmPipeline = c_void;
type OvGenAiGenerationConfig = c_void;
type OvGenAiDecodedResults = c_void;
type OvGenAiPerfMetrics = c_void;
type OvGenAiChatHistory = c_void;
type OvGenAiJsonContainer = c_void;

type OvStatus = i32;
const OV_STATUS_OK: OvStatus = 0;

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum OvGenAiStreamingStatus {
    Running = 0,
    Stop = 1,
    Cancel = 2,
}

#[repr(C)]
struct StreamerCallback {
    callback_func:
        Option<unsafe extern "C" fn(*const c_char, *mut c_void) -> OvGenAiStreamingStatus>,
    args: *mut c_void,
}

struct OpenVinoGenAiApi {
    _openvino_c: RetainedLibrary,
    _openvino_genai_c: RetainedLibrary,
    get_error_info: unsafe extern "C" fn(OvStatus) -> *const c_char,
    get_last_err_msg: unsafe extern "C" fn() -> *const c_char,
    create_pipeline: unsafe extern "C" fn(
        *const c_char,
        *const c_char,
        usize,
        *mut *mut OvGenAiLlmPipeline,
        ...
    ) -> OvStatus,
    destroy_pipeline: unsafe extern "C" fn(*mut OvGenAiLlmPipeline),
    pipeline_generate: unsafe extern "C" fn(
        *mut OvGenAiLlmPipeline,
        *const c_char,
        *const OvGenAiGenerationConfig,
        *const StreamerCallback,
        *mut *mut OvGenAiDecodedResults,
    ) -> OvStatus,
    pipeline_generate_with_history: unsafe extern "C" fn(
        *mut OvGenAiLlmPipeline,
        *const OvGenAiChatHistory,
        *const OvGenAiGenerationConfig,
        *const StreamerCallback,
        *mut *mut OvGenAiDecodedResults,
    ) -> OvStatus,
    create_json_container_from_json_string:
        unsafe extern "C" fn(*mut *mut OvGenAiJsonContainer, *const c_char) -> OvStatus,
    destroy_json_container: unsafe extern "C" fn(*mut OvGenAiJsonContainer),
    create_chat_history_from_json_container:
        unsafe extern "C" fn(*mut *mut OvGenAiChatHistory, *const OvGenAiJsonContainer) -> OvStatus,
    set_chat_history_extra_context:
        unsafe extern "C" fn(*mut OvGenAiChatHistory, *const OvGenAiJsonContainer) -> OvStatus,
    destroy_chat_history: unsafe extern "C" fn(*mut OvGenAiChatHistory),
    create_generation_config: unsafe extern "C" fn(*mut *mut OvGenAiGenerationConfig) -> OvStatus,
    destroy_generation_config: unsafe extern "C" fn(*mut OvGenAiGenerationConfig),
    set_max_new_tokens: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, usize) -> OvStatus,
    set_min_new_tokens: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, usize) -> OvStatus,
    set_stop_token_ids:
        unsafe extern "C" fn(*mut OvGenAiGenerationConfig, *const i64, usize) -> OvStatus,
    set_stop_strings:
        unsafe extern "C" fn(*mut OvGenAiGenerationConfig, *const *const c_char, usize) -> OvStatus,
    set_ignore_eos: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, bool) -> OvStatus,
    set_echo: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, bool) -> OvStatus,
    set_do_sample: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, bool) -> OvStatus,
    set_temperature: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, f32) -> OvStatus,
    set_top_p: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, f32) -> OvStatus,
    set_top_k: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, usize) -> OvStatus,
    set_repetition_penalty: unsafe extern "C" fn(*mut OvGenAiGenerationConfig, f32) -> OvStatus,
    validate_generation_config: unsafe extern "C" fn(*mut OvGenAiGenerationConfig) -> OvStatus,
    destroy_decoded_results: unsafe extern "C" fn(*mut OvGenAiDecodedResults),
    get_perf_metrics: unsafe extern "C" fn(
        *const OvGenAiDecodedResults,
        *mut *mut OvGenAiPerfMetrics,
    ) -> OvStatus,
    get_num_generation_tokens:
        unsafe extern "C" fn(*const OvGenAiPerfMetrics, *mut usize) -> OvStatus,
    get_ttft: unsafe extern "C" fn(*const OvGenAiPerfMetrics, *mut f32, *mut f32) -> OvStatus,
    get_throughput: unsafe extern "C" fn(*const OvGenAiPerfMetrics, *mut f32, *mut f32) -> OvStatus,
    get_generate_duration:
        unsafe extern "C" fn(*const OvGenAiPerfMetrics, *mut f32, *mut f32) -> OvStatus,
}

unsafe impl Send for OpenVinoGenAiApi {}
unsafe impl Sync for OpenVinoGenAiApi {}

impl OpenVinoGenAiApi {
    fn load(bundle: &OpenVinoRuntimeBundle) -> Result<Arc<Self>, String> {
        OpenVinoRuntimeLoader::ensure_initialized(bundle)?;
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

unsafe fn load_symbol<T: Copy>(lib: &RetainedLibrary, name: &[u8]) -> Result<T, String> {
    lib.get(name)
}

fn cstring(value: &str, field: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| format!("{field} contains interior NUL byte"))
}

fn path_to_cstring(path: &Path, field: &str) -> Result<CString, String> {
    let utf8 = path.to_str().ok_or_else(|| {
        format!(
            "Non-UTF8 path is unsupported for OpenVINO GenAI: {}",
            path.display()
        )
    })?;
    cstring(utf8, field)
}

fn check_status(api: &OpenVinoGenAiApi, status: OvStatus, context: &str) -> Result<(), String> {
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

fn check_plain_status(status: OvStatus, context: &str) -> Result<(), String> {
    if status == OV_STATUS_OK {
        Ok(())
    } else {
        Err(format!("{context}: OpenVINO status code {status}"))
    }
}

fn c_string_to_string(value: *const c_char) -> String {
    unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .trim()
        .to_string()
}

fn c_string_to_string_verbatim(value: *const c_char) -> String {
    unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned()
}

struct OvOwned<T> {
    api: Arc<OpenVinoGenAiApi>,
    ptr: *mut T,
    destroy: unsafe extern "C" fn(*mut T),
}

impl<T> OvOwned<T> {
    fn new(api: Arc<OpenVinoGenAiApi>, ptr: *mut T, destroy: unsafe extern "C" fn(*mut T)) -> Self {
        Self { api, ptr, destroy }
    }

    fn as_ptr(&self) -> *mut T {
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

struct OpenVinoGenAiInner {
    api: Arc<OpenVinoGenAiApi>,
    pipeline: *mut OvGenAiLlmPipeline,
    /// NPU StaticLLMPipeline response budget (MIN_RESPONSE_LEN at pipeline creation).
    /// max_new_tokens must be clamped to this value to avoid KV cache overflow.
    max_new_tokens_cap: usize,
    generation_controls: OpenVinoGenerationControls,
    disable_thinking: bool,
}

unsafe impl Send for OpenVinoGenAiInner {}

impl Drop for OpenVinoGenAiInner {
    fn drop(&mut self) {
        if !self.pipeline.is_null() {
            unsafe { (self.api.destroy_pipeline)(self.pipeline) };
            self.pipeline = ptr::null_mut();
        }
    }
}

pub struct OpenVinoGenAiGenerator {
    inner: Arc<Mutex<OpenVinoGenAiInner>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenVinoGenerationControls {
    pub min_new_tokens: Option<usize>,
    pub stop_token_ids: Option<Vec<i64>>,
    pub stop_strings: Option<Vec<String>>,
    pub ignore_eos: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenVinoNpuPipelineConfig {
    pub cache_dir: PathBuf,
    pub max_prompt_len: usize,
    pub min_response_len: usize,
    pub generation_controls: OpenVinoGenerationControls,
    pub disable_thinking: bool,
}

impl OpenVinoNpuPipelineConfig {
    pub fn new(
        cache_dir: impl Into<PathBuf>,
        max_prompt_len: usize,
        min_response_len: usize,
    ) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            max_prompt_len,
            min_response_len,
            generation_controls: OpenVinoGenerationControls::default(),
            disable_thinking: true,
        }
    }

    pub fn with_generation_controls(mut self, controls: OpenVinoGenerationControls) -> Self {
        self.generation_controls = controls;
        self
    }

    pub fn with_disable_thinking(mut self, disable_thinking: bool) -> Self {
        self.disable_thinking = disable_thinking;
        self
    }
}

impl OpenVinoGenAiGenerator {
    pub fn runtime_available(bundle: &OpenVinoRuntimeBundle) -> Result<(), String> {
        let _ = openvino_genai_api(bundle)?;
        Ok(())
    }

    pub fn new(
        bundle: &OpenVinoRuntimeBundle,
        model_dir: &Path,
        config: &OpenVinoNpuPipelineConfig,
    ) -> Result<Self, String> {
        let api = openvino_genai_api(bundle)?;
        std::fs::create_dir_all(&config.cache_dir)
            .map_err(|error| format!("Failed to create OpenVINO cache dir: {error}"))?;

        let model_dir = path_to_cstring(model_dir, "models_path")?;
        let device = cstring("NPU", "device")?;
        let cache_key = cstring("CACHE_DIR", "property key")?;
        let cache_value = path_to_cstring(&config.cache_dir, "CACHE_DIR")?;
        let max_prompt_len_key = cstring("MAX_PROMPT_LEN", "property key")?;
        let max_prompt_len_value = cstring(&config.max_prompt_len.to_string(), "MAX_PROMPT_LEN")?;
        let min_response_len_key = cstring("MIN_RESPONSE_LEN", "property key")?;
        let min_response_len_value =
            cstring(&config.min_response_len.to_string(), "MIN_RESPONSE_LEN")?;

        let mut pipeline_ptr: *mut OvGenAiLlmPipeline = ptr::null_mut();
        let status = unsafe {
            (api.create_pipeline)(
                model_dir.as_ptr(),
                device.as_ptr(),
                6,
                &mut pipeline_ptr,
                cache_key.as_ptr(),
                cache_value.as_ptr(),
                max_prompt_len_key.as_ptr(),
                max_prompt_len_value.as_ptr(),
                min_response_len_key.as_ptr(),
                min_response_len_value.as_ptr(),
            )
        };
        check_status(&api, status, "ov_genai_llm_pipeline_create")?;

        Ok(Self {
            inner: Arc::new(Mutex::new(OpenVinoGenAiInner {
                api: Arc::clone(&api),
                pipeline: pipeline_ptr,
                max_new_tokens_cap: config.min_response_len,
                generation_controls: config.generation_controls.clone(),
                disable_thinking: config.disable_thinking,
            })),
        })
    }

    pub fn run_preflight(&self, prompt: &str) -> Result<GenerationMetrics, String> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| "OpenVINO GenAI state mutex poisoned".to_string())?;
        let config = create_generation_config(
            &guard.api,
            GenerationConfig {
                max_length: 1,
                temperature: 0.0,
                top_k: None,
                top_p: None,
                repetition_penalty: 1.0,
                repetition_penalty_last_n: 0,
            },
            &OpenVinoGenerationControls::default(),
        )?;
        let metrics = generate_once(&guard, prompt, config.as_ptr(), None, Instant::now())?;
        if metrics.total_tokens == 0 {
            return Err("OpenVINO preflight generated zero tokens".to_string());
        }
        Ok(metrics)
    }

    pub async fn generate_stream<F>(
        &self,
        prompt: &str,
        config: Option<GenerationConfig>,
        cancelled: Arc<AtomicBool>,
        mut on_token: F,
    ) -> Result<GenerationMetrics, String>
    where
        F: FnMut(String),
    {
        let gen_config = config.unwrap_or_default();
        let prompt_owned = prompt.to_string();
        let inner = Arc::clone(&self.inner);
        let cancelled_worker = Arc::clone(&cancelled);
        let (token_tx, mut token_rx) = unbounded_channel();
        let worker = tokio::task::spawn_blocking(move || {
            generate_stream_blocking(inner, prompt_owned, gen_config, cancelled_worker, token_tx)
        });

        while let Some(piece) = token_rx.recv().await {
            on_token(piece);
        }

        worker
            .await
            .map_err(|error| format!("OpenVINO generation worker join error: {error}"))?
    }

    pub async fn generate_stream_messages<F>(
        &self,
        messages: &[InferenceChatMessage],
        config: Option<GenerationConfig>,
        cancelled: Arc<AtomicBool>,
        mut on_token: F,
    ) -> Result<GenerationMetrics, String>
    where
        F: FnMut(String),
    {
        let gen_config = config.unwrap_or_default();
        let messages = messages.to_vec();
        let inner = Arc::clone(&self.inner);
        let cancelled_worker = Arc::clone(&cancelled);
        let (token_tx, mut token_rx) = unbounded_channel();
        let worker = tokio::task::spawn_blocking(move || {
            generate_stream_with_history_blocking(
                inner,
                messages,
                gen_config,
                cancelled_worker,
                token_tx,
            )
        });

        while let Some(piece) = token_rx.recv().await {
            on_token(piece);
        }

        worker
            .await
            .map_err(|error| format!("OpenVINO generation worker join error: {error}"))?
    }
}

/// Clamp max_new_tokens to the NPU pipeline's MIN_RESPONSE_LEN budget.
/// StaticLLMPipeline pre-allocates a fixed KV cache at construction time.
/// Requesting more tokens than the budget causes KV cache overflow, which
/// makes the pipeline wrap around and replay context tokens as output.
fn clamp_max_new_tokens(config: GenerationConfig, cap: usize) -> GenerationConfig {
    if config.max_length <= cap {
        return config;
    }
    log::info!(
        "Clamping max_new_tokens from {} to {} (NPU StaticLLMPipeline response budget)",
        config.max_length,
        cap
    );
    GenerationConfig {
        max_length: cap,
        ..config
    }
}

fn generate_stream_blocking(
    inner: Arc<Mutex<OpenVinoGenAiInner>>,
    prompt: String,
    gen_config: GenerationConfig,
    cancelled: Arc<AtomicBool>,
    token_tx: UnboundedSender<String>,
) -> Result<GenerationMetrics, String> {
    let guard = inner
        .lock()
        .map_err(|_| "OpenVINO GenAI state mutex poisoned".to_string())?;
    let gen_config = clamp_max_new_tokens(gen_config, guard.max_new_tokens_cap);
    let config = create_generation_config(&guard.api, gen_config, &guard.generation_controls)?;
    let start = Instant::now();
    let mut callback_state = StreamCallbackState {
        sender: token_tx,
        cancelled,
    };
    let streamer = StreamerCallback {
        callback_func: Some(stream_callback),
        args: &mut callback_state as *mut StreamCallbackState as *mut c_void,
    };
    generate_once(&guard, &prompt, config.as_ptr(), Some(&streamer), start)
}

fn generate_stream_with_history_blocking(
    inner: Arc<Mutex<OpenVinoGenAiInner>>,
    messages: Vec<InferenceChatMessage>,
    gen_config: GenerationConfig,
    cancelled: Arc<AtomicBool>,
    token_tx: UnboundedSender<String>,
) -> Result<GenerationMetrics, String> {
    let guard = inner
        .lock()
        .map_err(|_| "OpenVINO GenAI state mutex poisoned".to_string())?;
    let gen_config = clamp_max_new_tokens(gen_config, guard.max_new_tokens_cap);
    let config = create_generation_config(&guard.api, gen_config, &guard.generation_controls)?;
    let history = create_chat_history(&guard.api, &messages, guard.disable_thinking)?;
    let start = Instant::now();
    let mut callback_state = StreamCallbackState {
        sender: token_tx,
        cancelled,
    };
    let streamer = StreamerCallback {
        callback_func: Some(stream_callback),
        args: &mut callback_state as *mut StreamCallbackState as *mut c_void,
    };
    generate_with_history_once(
        &guard,
        history.as_ptr(),
        config.as_ptr(),
        Some(&streamer),
        start,
    )
}

fn create_generation_config(
    api: &Arc<OpenVinoGenAiApi>,
    config: GenerationConfig,
    controls: &OpenVinoGenerationControls,
) -> Result<OvOwned<OvGenAiGenerationConfig>, String> {
    let mut config_ptr: *mut OvGenAiGenerationConfig = ptr::null_mut();
    check_status(
        api,
        unsafe { (api.create_generation_config)(&mut config_ptr) },
        "ov_genai_generation_config_create",
    )?;
    let config_handle = OvOwned::new(Arc::clone(api), config_ptr, api.destroy_generation_config);

    check_status(
        api,
        unsafe { (api.set_echo)(config_handle.as_ptr(), false) },
        "ov_genai_generation_config_set_echo",
    )?;
    check_status(
        api,
        unsafe { (api.set_max_new_tokens)(config_handle.as_ptr(), config.max_length) },
        "ov_genai_generation_config_set_max_new_tokens",
    )?;
    if let Some(min_new_tokens) = controls.min_new_tokens {
        check_status(
            api,
            unsafe { (api.set_min_new_tokens)(config_handle.as_ptr(), min_new_tokens) },
            "ov_genai_generation_config_set_min_new_tokens",
        )?;
    }
    if let Some(stop_token_ids) = controls.stop_token_ids.as_ref() {
        if !stop_token_ids.is_empty() {
            check_status(
                api,
                unsafe {
                    (api.set_stop_token_ids)(
                        config_handle.as_ptr(),
                        stop_token_ids.as_ptr(),
                        stop_token_ids.len(),
                    )
                },
                "ov_genai_generation_config_set_stop_token_ids",
            )?;
        }
    }
    if let Some(stop_strings) = controls.stop_strings.as_ref() {
        if !stop_strings.is_empty() {
            let stop_cstrings = stop_strings
                .iter()
                .map(|value| cstring(value, "stop string"))
                .collect::<Result<Vec<_>, _>>()?;
            let stop_ptrs = stop_cstrings
                .iter()
                .map(|value| value.as_ptr())
                .collect::<Vec<_>>();
            check_status(
                api,
                unsafe {
                    (api.set_stop_strings)(
                        config_handle.as_ptr(),
                        stop_ptrs.as_ptr(),
                        stop_ptrs.len(),
                    )
                },
                "ov_genai_generation_config_set_stop_strings",
            )?;
        }
    }
    if let Some(ignore_eos) = controls.ignore_eos {
        check_status(
            api,
            unsafe { (api.set_ignore_eos)(config_handle.as_ptr(), ignore_eos) },
            "ov_genai_generation_config_set_ignore_eos",
        )?;
    }

    let do_sample = config.temperature > 0.0;
    check_status(
        api,
        unsafe { (api.set_do_sample)(config_handle.as_ptr(), do_sample) },
        "ov_genai_generation_config_set_do_sample",
    )?;

    if do_sample {
        check_status(
            api,
            unsafe { (api.set_temperature)(config_handle.as_ptr(), config.temperature) },
            "ov_genai_generation_config_set_temperature",
        )?;
        if let Some(top_p) = config.top_p {
            check_status(
                api,
                unsafe { (api.set_top_p)(config_handle.as_ptr(), top_p) },
                "ov_genai_generation_config_set_top_p",
            )?;
        }
        if let Some(top_k) = config.top_k {
            check_status(
                api,
                unsafe { (api.set_top_k)(config_handle.as_ptr(), top_k) },
                "ov_genai_generation_config_set_top_k",
            )?;
        }
    }

    if config.repetition_penalty.is_finite() && config.repetition_penalty > 0.0 {
        check_status(
            api,
            unsafe {
                (api.set_repetition_penalty)(config_handle.as_ptr(), config.repetition_penalty)
            },
            "ov_genai_generation_config_set_repetition_penalty",
        )?;
    }

    check_status(
        api,
        unsafe { (api.validate_generation_config)(config_handle.as_ptr()) },
        "ov_genai_generation_config_validate",
    )?;

    Ok(config_handle)
}

fn generate_once(
    guard: &OpenVinoGenAiInner,
    prompt: &str,
    config: *const OvGenAiGenerationConfig,
    streamer: Option<&StreamerCallback>,
    start: Instant,
) -> Result<GenerationMetrics, String> {
    let input = cstring(prompt, "prompt")?;
    let mut results_ptr: *mut OvGenAiDecodedResults = ptr::null_mut();
    let streamer_ptr = streamer
        .map(|value| value as *const StreamerCallback)
        .unwrap_or(ptr::null());
    check_status(
        &guard.api,
        unsafe {
            (guard.api.pipeline_generate)(
                guard.pipeline,
                input.as_ptr(),
                config,
                streamer_ptr,
                &mut results_ptr,
            )
        },
        "ov_genai_llm_pipeline_generate",
    )?;

    if results_ptr.is_null() {
        return Ok(fallback_metrics(start, 0));
    }

    let results = OvOwned::new(
        Arc::clone(&guard.api),
        results_ptr,
        guard.api.destroy_decoded_results,
    );
    read_generation_metrics(&guard.api, results.as_ptr(), start)
}

fn create_chat_history(
    api: &Arc<OpenVinoGenAiApi>,
    messages: &[InferenceChatMessage],
    disable_thinking: bool,
) -> Result<OvOwned<OvGenAiChatHistory>, String> {
    if messages.is_empty() {
        return Err("messages cannot be empty for OpenVINO structured generation".to_string());
    }

    let messages_json = serde_json::to_string(messages)
        .map_err(|error| format!("Failed to encode messages JSON: {error}"))?;
    let messages_json = cstring(&messages_json, "messages JSON")?;

    let mut messages_container_ptr: *mut OvGenAiJsonContainer = ptr::null_mut();
    check_plain_status(
        unsafe {
            (api.create_json_container_from_json_string)(
                &mut messages_container_ptr,
                messages_json.as_ptr(),
            )
        },
        "ov_genai_json_container_create_from_json_string(messages)",
    )?;
    let messages_container = OvOwned::new(
        Arc::clone(api),
        messages_container_ptr,
        api.destroy_json_container,
    );

    let mut history_ptr: *mut OvGenAiChatHistory = ptr::null_mut();
    check_plain_status(
        unsafe {
            (api.create_chat_history_from_json_container)(
                &mut history_ptr,
                messages_container.as_ptr(),
            )
        },
        "ov_genai_chat_history_create_from_json_container",
    )?;
    let history = OvOwned::new(Arc::clone(api), history_ptr, api.destroy_chat_history);

    if disable_thinking {
        let extra_context = serde_json::json!({ "enable_thinking": false }).to_string();
        let extra_context = cstring(&extra_context, "chat extra context JSON")?;
        let mut extra_context_ptr: *mut OvGenAiJsonContainer = ptr::null_mut();
        check_plain_status(
            unsafe {
                (api.create_json_container_from_json_string)(
                    &mut extra_context_ptr,
                    extra_context.as_ptr(),
                )
            },
            "ov_genai_json_container_create_from_json_string(extra_context)",
        )?;
        let extra_context = OvOwned::new(
            Arc::clone(api),
            extra_context_ptr,
            api.destroy_json_container,
        );
        check_plain_status(
            unsafe {
                (api.set_chat_history_extra_context)(history.as_ptr(), extra_context.as_ptr())
            },
            "ov_genai_chat_history_set_extra_context",
        )?;
    }

    Ok(history)
}

fn generate_with_history_once(
    guard: &OpenVinoGenAiInner,
    history: *const OvGenAiChatHistory,
    config: *const OvGenAiGenerationConfig,
    streamer: Option<&StreamerCallback>,
    start: Instant,
) -> Result<GenerationMetrics, String> {
    let mut results_ptr: *mut OvGenAiDecodedResults = ptr::null_mut();
    let streamer_ptr = streamer
        .map(|value| value as *const StreamerCallback)
        .unwrap_or(ptr::null());
    check_status(
        &guard.api,
        unsafe {
            (guard.api.pipeline_generate_with_history)(
                guard.pipeline,
                history,
                config,
                streamer_ptr,
                &mut results_ptr,
            )
        },
        "ov_genai_llm_pipeline_generate_with_history",
    )?;

    if results_ptr.is_null() {
        return Ok(fallback_metrics(start, 0));
    }

    let results = OvOwned::new(
        Arc::clone(&guard.api),
        results_ptr,
        guard.api.destroy_decoded_results,
    );
    read_generation_metrics(&guard.api, results.as_ptr(), start)
}

fn read_generation_metrics(
    api: &Arc<OpenVinoGenAiApi>,
    results: *const OvGenAiDecodedResults,
    started: Instant,
) -> Result<GenerationMetrics, String> {
    let mut metrics_ptr: *mut OvGenAiPerfMetrics = ptr::null_mut();
    check_status(
        api,
        unsafe { (api.get_perf_metrics)(results, &mut metrics_ptr) },
        "ov_genai_decoded_results_get_perf_metrics",
    )?;

    if metrics_ptr.is_null() {
        return Ok(fallback_metrics(started, 0));
    }

    // metrics_ptr is owned by DecodedResults (results) and freed by destroy_decoded_results;
    // there is no separate ov_genai_decoded_results_perf_metrics_free in the C API.
    let total_tokens = metric_usize(
        api,
        metrics_ptr,
        api.get_num_generation_tokens,
        "ov_genai_perf_metrics_get_num_generation_tokens",
    )?;
    let time_to_first_token_ms = metric_ms_pair(
        api,
        metrics_ptr,
        api.get_ttft,
        "ov_genai_perf_metrics_get_ttft",
    )?;
    let tokens_per_second = metric_f32_pair(
        api,
        metrics_ptr,
        api.get_throughput,
        "ov_genai_perf_metrics_get_throughput",
    )?;
    let total_time_ms = metric_ms_pair(
        api,
        metrics_ptr,
        api.get_generate_duration,
        "ov_genai_perf_metrics_get_generate_duration",
    )?;

    Ok(GenerationMetrics {
        total_tokens,
        time_to_first_token_ms: Some(time_to_first_token_ms),
        tokens_per_second: tokens_per_second as f64,
        total_time_ms,
    })
}

fn metric_usize(
    api: &Arc<OpenVinoGenAiApi>,
    metrics: *const OvGenAiPerfMetrics,
    getter: unsafe extern "C" fn(*const OvGenAiPerfMetrics, *mut usize) -> OvStatus,
    context: &str,
) -> Result<usize, String> {
    let mut value = 0usize;
    check_status(api, unsafe { getter(metrics, &mut value) }, context)?;
    Ok(value)
}

fn metric_ms_pair(
    api: &Arc<OpenVinoGenAiApi>,
    metrics: *const OvGenAiPerfMetrics,
    getter: unsafe extern "C" fn(*const OvGenAiPerfMetrics, *mut f32, *mut f32) -> OvStatus,
    context: &str,
) -> Result<u64, String> {
    Ok(metric_f32_pair(api, metrics, getter, context)?
        .max(0.0)
        .round() as u64)
}

fn metric_f32_pair(
    api: &Arc<OpenVinoGenAiApi>,
    metrics: *const OvGenAiPerfMetrics,
    getter: unsafe extern "C" fn(*const OvGenAiPerfMetrics, *mut f32, *mut f32) -> OvStatus,
    context: &str,
) -> Result<f32, String> {
    let mut mean = 0.0f32;
    let mut std = 0.0f32;
    check_status(
        api,
        unsafe { getter(metrics, &mut mean, &mut std) },
        context,
    )?;
    Ok(mean)
}

fn fallback_metrics(started: Instant, total_tokens: usize) -> GenerationMetrics {
    let total_time_ms = started.elapsed().as_millis() as u64;
    let tokens_per_second = if total_time_ms == 0 {
        0.0
    } else {
        total_tokens as f64 / (total_time_ms as f64 / 1_000.0)
    };
    GenerationMetrics {
        total_tokens,
        time_to_first_token_ms: None,
        tokens_per_second,
        total_time_ms,
    }
}

struct StreamCallbackState {
    sender: UnboundedSender<String>,
    cancelled: Arc<AtomicBool>,
}

unsafe extern "C" fn stream_callback(
    text: *const c_char,
    args: *mut c_void,
) -> OvGenAiStreamingStatus {
    if args.is_null() {
        return OvGenAiStreamingStatus::Cancel;
    }

    let state = &mut *(args as *mut StreamCallbackState);
    if state.cancelled.load(Ordering::SeqCst) {
        return OvGenAiStreamingStatus::Stop;
    }

    if !text.is_null() {
        let piece = c_string_to_string_verbatim(text);
        if !piece.is_empty() && state.sender.send(piece).is_err() {
            return OvGenAiStreamingStatus::Stop;
        }
    }

    OvGenAiStreamingStatus::Running
}

#[derive(Clone)]
enum CachedOpenVinoGenAiApi {
    Success(Arc<OpenVinoGenAiApi>),
    Failure(String),
}

#[derive(Default)]
struct OpenVinoGenAiApiState {
    active_fingerprint: Option<String>,
    results: HashMap<String, CachedOpenVinoGenAiApi>,
}

static OPENVINO_GENAI_API: std::sync::OnceLock<Mutex<OpenVinoGenAiApiState>> =
    std::sync::OnceLock::new();

fn openvino_genai_api(bundle: &OpenVinoRuntimeBundle) -> Result<Arc<OpenVinoGenAiApi>, String> {
    let state = OPENVINO_GENAI_API.get_or_init(|| Mutex::new(OpenVinoGenAiApiState::default()));
    let fingerprint = bundle.fingerprint.value.clone();

    {
        let guard = match state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("Recovering from poisoned OpenVINO GenAI API cache mutex");
                poisoned.into_inner()
            }
        };

        if let Some(cached) = guard.results.get(&fingerprint) {
            return match cached {
                CachedOpenVinoGenAiApi::Success(api) => Ok(Arc::clone(api)),
                CachedOpenVinoGenAiApi::Failure(error) => Err(error.clone()),
            };
        }

        if let Some(active) = guard.active_fingerprint.as_ref() {
            if active != &fingerprint {
                let error = format!(
                    "OpenVINO GenAI already initialized from bundle fingerprint '{active}'; restart the process to use '{}'",
                    fingerprint
                );
                drop(guard);
                let mut guard = match state.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                guard
                    .results
                    .insert(fingerprint, CachedOpenVinoGenAiApi::Failure(error.clone()));
                return Err(error);
            }
        }
    }

    if let Some(failure) = bundle.npu_validation_failure {
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
            .insert(fingerprint, CachedOpenVinoGenAiApi::Failure(error.clone()));
        return Err(error);
    }

    let api = OpenVinoGenAiApi::load(bundle)?;
    let mut guard = match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.active_fingerprint = Some(fingerprint.clone());
    guard.results.insert(
        fingerprint,
        CachedOpenVinoGenAiApi::Success(Arc::clone(&api)),
    );
    Ok(api)
}

#[cfg(test)]
mod tests {
    use super::{metric_ms_pair, OpenVinoGenAiGenerator};
    use crate::inference::runtime_loading::{
        BundleValidationFailureClass, OpenVinoRuntimeBundle, RequiredRuntimeFile,
        RuntimeBundleFingerprint, RuntimeFamily, RuntimeVersionMetadata,
    };
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn metric_ms_pair_rounds_float_values() {
        unsafe extern "C" fn getter(
            _metrics: *const std::ffi::c_void,
            mean: *mut f32,
            std: *mut f32,
        ) -> i32 {
            unsafe {
                *mean = 12.6;
                *std = 0.0;
            }
            0
        }

        let temp = tempdir().expect("temp dir");
        let bundle = build_bundle(temp.path(), None);
        let api = match super::openvino_genai_api(&bundle) {
            Ok(api) => api,
            Err(_) => return,
        };

        let value =
            metric_ms_pair(&api, std::ptr::null(), getter, "metric").expect("metric rounding");
        assert_eq!(value, 13);
    }

    #[test]
    fn runtime_available_rejects_invalid_bundle() {
        let temp = tempdir().expect("temp dir");
        let bundle = build_bundle(
            temp.path(),
            Some(BundleValidationFailureClass::OpenVinoGenAiMissing),
        );

        let err = OpenVinoGenAiGenerator::runtime_available(&bundle)
            .expect_err("invalid bundle should fail");
        assert!(err.contains("openvino_genai_missing"));
    }

    fn build_bundle(
        root: &Path,
        failure: Option<BundleValidationFailureClass>,
    ) -> OpenVinoRuntimeBundle {
        let root = root.to_path_buf();
        let openvino_dll = root.join("openvino.dll");
        let openvino_c_dll = root.join("openvino_c.dll");
        let npu_plugin = root.join("openvino_intel_npu_plugin.dll");
        let npu_compiler = root.join("openvino_intel_npu_compiler.dll");
        let cpu_plugin = root.join("openvino_intel_cpu_plugin.dll");
        let ir_frontend = root.join("openvino_ir_frontend.dll");
        let openvino_genai_dll = root.join("openvino_genai.dll");
        let openvino_genai_c_dll = root.join("openvino_genai_c.dll");
        let tokenizers_dll = root.join("openvino_tokenizers.dll");
        let tbb_dll = root.join("tbb12.dll");
        let tbbbind_dll = root.join("tbbbind_2_5.dll");
        let tbbmalloc_dll = root.join("tbbmalloc.dll");
        let tbbmalloc_proxy_dll = root.join("tbbmalloc_proxy.dll");
        let icudt_dll = root.join("icudt70.dll");
        let icuuc_dll = root.join("icuuc70.dll");

        for file in [
            &openvino_dll,
            &openvino_c_dll,
            &npu_plugin,
            &npu_compiler,
            &cpu_plugin,
            &ir_frontend,
            &openvino_genai_dll,
            &openvino_genai_c_dll,
            &tokenizers_dll,
            &tbb_dll,
            &tbbbind_dll,
            &tbbmalloc_dll,
            &tbbmalloc_proxy_dll,
            &icudt_dll,
            &icuuc_dll,
        ] {
            fs::write(file, []).expect("write runtime file");
        }

        let required_files = vec![
            RequiredRuntimeFile::new("openvino.dll", openvino_dll.clone()),
            RequiredRuntimeFile::new("openvino_c.dll", openvino_c_dll.clone()),
            RequiredRuntimeFile::new("openvino_intel_npu_plugin.dll", npu_plugin.clone()),
            RequiredRuntimeFile::new("openvino_intel_npu_compiler.dll", npu_compiler.clone()),
            RequiredRuntimeFile::new("openvino_intel_cpu_plugin.dll", cpu_plugin.clone()),
            RequiredRuntimeFile::new("openvino_ir_frontend.dll", ir_frontend.clone()),
            RequiredRuntimeFile::new("openvino_genai.dll", openvino_genai_dll.clone()),
            RequiredRuntimeFile::new("openvino_genai_c.dll", openvino_genai_c_dll.clone()),
            RequiredRuntimeFile::new("openvino_tokenizers.dll", tokenizers_dll.clone()),
            RequiredRuntimeFile::new("tbb12.dll", tbb_dll.clone()),
            RequiredRuntimeFile::new("tbbbind_2_5.dll", tbbbind_dll.clone()),
            RequiredRuntimeFile::new("tbbmalloc.dll", tbbmalloc_dll.clone()),
            RequiredRuntimeFile::new("tbbmalloc_proxy.dll", tbbmalloc_proxy_dll.clone()),
            RequiredRuntimeFile::new("icudt70.dll", icudt_dll.clone()),
            RequiredRuntimeFile::new("icuuc70.dll", icuuc_dll.clone()),
        ];
        let version_metadata = vec![
            RuntimeVersionMetadata::new("openvino-runtime", "2026.0.0"),
            RuntimeVersionMetadata::new("openvino-genai", "2026.0.0"),
            RuntimeVersionMetadata::new("openvino-tokenizers", "2026.0.0"),
        ];
        let fingerprint = RuntimeBundleFingerprint::new(
            RuntimeFamily::OpenVino,
            Some(root.clone()),
            &root,
            &required_files,
            &version_metadata,
        );

        OpenVinoRuntimeBundle {
            bundle_root: root.clone(),
            canonical_root: Some(root),
            openvino_dll,
            openvino_c_dll,
            openvino_intel_npu_plugin_dll: npu_plugin,
            openvino_intel_npu_compiler_dll: npu_compiler,
            openvino_intel_cpu_plugin_dll: cpu_plugin,
            openvino_ir_frontend_dll: ir_frontend,
            openvino_genai_dll,
            openvino_genai_c_dll,
            openvino_tokenizers_dll: tokenizers_dll,
            tbb_dll,
            tbbbind_dll,
            tbbmalloc_dll,
            tbbmalloc_proxy_dll,
            icudt_dll,
            icuuc_dll,
            required_files,
            version_metadata,
            npu_validation_failure: failure,
            fingerprint,
        }
    }
}
