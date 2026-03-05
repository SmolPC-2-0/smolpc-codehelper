use super::super::types::{GenerationConfig, GenerationMetrics};
use half::f16;
use libloading::Library;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

type OgaResult = c_void;
type OgaConfig = c_void;
type OgaModel = c_void;
type OgaTokenizer = c_void;
type OgaTokenizerStream = c_void;
type OgaSequences = c_void;
type OgaGeneratorParams = c_void;
type OgaGenerator = c_void;
type OgaTensor = c_void;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Values are written by the GenAI C API through FFI, not constructed in Rust.
enum OgaElementType {
    Undefined = 0,
    Float32 = 1,
    Uint8 = 2,
    Int8 = 3,
    Uint16 = 4,
    Int16 = 5,
    Int32 = 6,
    Int64 = 7,
    String = 8,
    Bool = 9,
    Float16 = 10,
    Float64 = 11,
    Uint32 = 12,
    Uint64 = 13,
    Complex64 = 14,
    Complex128 = 15,
    Bfloat16 = 16,
}

struct GenAiApi {
    _lib: Library,
    result_get_error: unsafe extern "system" fn(*const OgaResult) -> *const c_char,
    destroy_result: unsafe extern "system" fn(*mut OgaResult),

    create_config: unsafe extern "system" fn(*const c_char, *mut *mut OgaConfig) -> *mut OgaResult,
    destroy_config: unsafe extern "system" fn(*mut OgaConfig),
    config_clear_providers: unsafe extern "system" fn(*mut OgaConfig) -> *mut OgaResult,
    config_append_provider:
        unsafe extern "system" fn(*mut OgaConfig, *const c_char) -> *mut OgaResult,
    config_set_hw_device_id:
        unsafe extern "system" fn(*mut OgaConfig, *const c_char, u32) -> *mut OgaResult,

    create_model_from_config:
        unsafe extern "system" fn(*const OgaConfig, *mut *mut OgaModel) -> *mut OgaResult,
    destroy_model: unsafe extern "system" fn(*mut OgaModel),

    create_tokenizer:
        unsafe extern "system" fn(*const OgaModel, *mut *mut OgaTokenizer) -> *mut OgaResult,
    destroy_tokenizer: unsafe extern "system" fn(*mut OgaTokenizer),
    tokenizer_get_eos_token_ids: unsafe extern "system" fn(
        *const OgaTokenizer,
        *mut *const i32,
        *mut usize,
    ) -> *mut OgaResult,
    tokenizer_encode: unsafe extern "system" fn(
        *const OgaTokenizer,
        *const c_char,
        *mut OgaSequences,
    ) -> *mut OgaResult,
    create_tokenizer_stream: unsafe extern "system" fn(
        *const OgaTokenizer,
        *mut *mut OgaTokenizerStream,
    ) -> *mut OgaResult,
    destroy_tokenizer_stream: unsafe extern "system" fn(*mut OgaTokenizerStream),
    tokenizer_stream_decode: unsafe extern "system" fn(
        *mut OgaTokenizerStream,
        i32,
        *mut *const c_char,
    ) -> *mut OgaResult,

    create_sequences: unsafe extern "system" fn(*mut *mut OgaSequences) -> *mut OgaResult,
    destroy_sequences: unsafe extern "system" fn(*mut OgaSequences),
    sequences_get_sequence_count: unsafe extern "system" fn(*const OgaSequences, usize) -> usize,

    create_generator_params:
        unsafe extern "system" fn(*const OgaModel, *mut *mut OgaGeneratorParams) -> *mut OgaResult,
    destroy_generator_params: unsafe extern "system" fn(*mut OgaGeneratorParams),
    generator_params_set_search_number:
        unsafe extern "system" fn(*mut OgaGeneratorParams, *const c_char, f64) -> *mut OgaResult,
    generator_params_set_search_bool:
        unsafe extern "system" fn(*mut OgaGeneratorParams, *const c_char, bool) -> *mut OgaResult,

    create_generator: unsafe extern "system" fn(
        *const OgaModel,
        *const OgaGeneratorParams,
        *mut *mut OgaGenerator,
    ) -> *mut OgaResult,
    destroy_generator: unsafe extern "system" fn(*mut OgaGenerator),
    generator_append_token_sequences:
        unsafe extern "system" fn(*mut OgaGenerator, *const OgaSequences) -> *mut OgaResult,
    generator_generate_next_token: unsafe extern "system" fn(*mut OgaGenerator) -> *mut OgaResult,
    generator_get_next_tokens: unsafe extern "system" fn(
        *const OgaGenerator,
        *mut *const i32,
        *mut usize,
    ) -> *mut OgaResult,
    generator_is_done: unsafe extern "system" fn(*mut OgaGenerator) -> bool,
    generator_get_logits:
        unsafe extern "system" fn(*mut OgaGenerator, *mut *mut OgaTensor) -> *mut OgaResult,

    destroy_tensor: unsafe extern "system" fn(*mut OgaTensor),
    tensor_get_type:
        unsafe extern "system" fn(*mut OgaTensor, *mut OgaElementType) -> *mut OgaResult,
    tensor_get_shape_rank: unsafe extern "system" fn(*mut OgaTensor, *mut usize) -> *mut OgaResult,
    tensor_get_shape: unsafe extern "system" fn(*mut OgaTensor, *mut i64, usize) -> *mut OgaResult,
    tensor_get_data: unsafe extern "system" fn(*mut OgaTensor, *mut *mut c_void) -> *mut OgaResult,
}

// The API table is immutable after loading and function pointers are thread-safe to read.
unsafe impl Send for GenAiApi {}
unsafe impl Sync for GenAiApi {}

impl GenAiApi {
    fn load() -> Result<Arc<Self>, String> {
        let dll_path = find_genai_dll().ok_or_else(|| {
            "onnxruntime-genai.dll not found in deterministic runtime paths".to_string()
        })?;
        let lib = unsafe { Library::new(&dll_path) }
            .map_err(|e| format!("Failed to load {}: {e}", dll_path.display()))?;

        unsafe {
            Ok(Arc::new(Self {
                result_get_error: load_symbol(&lib, b"OgaResultGetError\0")?,
                destroy_result: load_symbol(&lib, b"OgaDestroyResult\0")?,

                create_config: load_symbol(&lib, b"OgaCreateConfig\0")?,
                destroy_config: load_symbol(&lib, b"OgaDestroyConfig\0")?,
                config_clear_providers: load_symbol(&lib, b"OgaConfigClearProviders\0")?,
                config_append_provider: load_symbol(&lib, b"OgaConfigAppendProvider\0")?,
                config_set_hw_device_id: load_symbol(
                    &lib,
                    b"OgaConfigSetDecoderProviderOptionsHardwareDeviceId\0",
                )?,

                create_model_from_config: load_symbol(&lib, b"OgaCreateModelFromConfig\0")?,
                destroy_model: load_symbol(&lib, b"OgaDestroyModel\0")?,

                create_tokenizer: load_symbol(&lib, b"OgaCreateTokenizer\0")?,
                destroy_tokenizer: load_symbol(&lib, b"OgaDestroyTokenizer\0")?,
                tokenizer_get_eos_token_ids: load_symbol(&lib, b"OgaTokenizerGetEosTokenIds\0")?,
                tokenizer_encode: load_symbol(&lib, b"OgaTokenizerEncode\0")?,
                create_tokenizer_stream: load_symbol(&lib, b"OgaCreateTokenizerStream\0")?,
                destroy_tokenizer_stream: load_symbol(&lib, b"OgaDestroyTokenizerStream\0")?,
                tokenizer_stream_decode: load_symbol(&lib, b"OgaTokenizerStreamDecode\0")?,

                create_sequences: load_symbol(&lib, b"OgaCreateSequences\0")?,
                destroy_sequences: load_symbol(&lib, b"OgaDestroySequences\0")?,
                sequences_get_sequence_count: load_symbol(&lib, b"OgaSequencesGetSequenceCount\0")?,

                create_generator_params: load_symbol(&lib, b"OgaCreateGeneratorParams\0")?,
                destroy_generator_params: load_symbol(&lib, b"OgaDestroyGeneratorParams\0")?,
                generator_params_set_search_number: load_symbol(
                    &lib,
                    b"OgaGeneratorParamsSetSearchNumber\0",
                )?,
                generator_params_set_search_bool: load_symbol(
                    &lib,
                    b"OgaGeneratorParamsSetSearchBool\0",
                )?,

                create_generator: load_symbol(&lib, b"OgaCreateGenerator\0")?,
                destroy_generator: load_symbol(&lib, b"OgaDestroyGenerator\0")?,
                generator_append_token_sequences: load_symbol(
                    &lib,
                    b"OgaGenerator_AppendTokenSequences\0",
                )?,
                generator_generate_next_token: load_symbol(
                    &lib,
                    b"OgaGenerator_GenerateNextToken\0",
                )?,
                generator_get_next_tokens: load_symbol(&lib, b"OgaGenerator_GetNextTokens\0")?,
                generator_is_done: load_symbol(&lib, b"OgaGenerator_IsDone\0")?,
                generator_get_logits: load_symbol(&lib, b"OgaGenerator_GetLogits\0")?,

                destroy_tensor: load_symbol(&lib, b"OgaDestroyTensor\0")?,
                tensor_get_type: load_symbol(&lib, b"OgaTensorGetType\0")?,
                tensor_get_shape_rank: load_symbol(&lib, b"OgaTensorGetShapeRank\0")?,
                tensor_get_shape: load_symbol(&lib, b"OgaTensorGetShape\0")?,
                tensor_get_data: load_symbol(&lib, b"OgaTensorGetData\0")?,

                _lib: lib,
            }))
        }
    }
}

unsafe fn load_symbol<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, String> {
    let symbol: libloading::Symbol<'_, T> = lib
        .get(name)
        .map_err(|e| format!("Missing symbol {}: {e}", String::from_utf8_lossy(name)))?;
    Ok(*symbol)
}

fn find_genai_dll() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SMOLPC_GENAI_DYLIB") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    if let Some(path) = std::env::var_os("SMOLPC_ORT_DYLIB_DIR") {
        let candidate = PathBuf::from(path).join("onnxruntime-genai.dll");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let mut candidates = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("onnxruntime-genai.dll"));
            candidates.push(exe_dir.join("libs").join("onnxruntime-genai.dll"));
        }
    }

    #[cfg(debug_assertions)]
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("libs")
            .join("onnxruntime-genai.dll"),
    );
    candidates.push(PathBuf::from("libs").join("onnxruntime-genai.dll"));

    candidates.into_iter().find(|path| path.exists())
}

fn cstring(value: &str, field: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| format!("{field} contains interior NUL byte"))
}

fn path_to_cstring(path: &Path) -> Result<CString, String> {
    let utf8 = path.to_str().ok_or_else(|| {
        format!(
            "Non-UTF8 path is unsupported for GenAI: {}. Move the model to an ASCII-only path.",
            path.display()
        )
    })?;
    cstring(utf8, "path")
}

fn check_oga(api: &GenAiApi, result: *mut OgaResult, context: &str) -> Result<(), String> {
    if result.is_null() {
        return Ok(());
    }

    let message = unsafe {
        let ptr = (api.result_get_error)(result);
        if ptr.is_null() {
            "unknown GenAI error".to_string()
        } else {
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    };

    unsafe { (api.destroy_result)(result) };
    Err(format!("{context}: {message}"))
}

struct OgaOwned<T> {
    api: Arc<GenAiApi>,
    ptr: *mut T,
    destroy: unsafe extern "system" fn(*mut T),
}

impl<T> OgaOwned<T> {
    fn new(api: Arc<GenAiApi>, ptr: *mut T, destroy: unsafe extern "system" fn(*mut T)) -> Self {
        Self { api, ptr, destroy }
    }

    fn as_ptr(&self) -> *mut T {
        self.ptr
    }

    fn into_raw(mut self) -> *mut T {
        std::mem::replace(&mut self.ptr, ptr::null_mut())
    }
}

impl<T> Drop for OgaOwned<T> {
    fn drop(&mut self) {
        let ptr = std::mem::replace(&mut self.ptr, ptr::null_mut());
        if ptr.is_null() {
            return;
        }
        let destroy = self.destroy;
        unsafe { destroy(ptr) };
        let _ = &self.api;
    }
}

struct GenAiDirectMlInner {
    api: Arc<GenAiApi>,
    model: *mut OgaModel,
    tokenizer: *mut OgaTokenizer,
    eos_token_ids: Vec<i32>,
}

// ONNX Runtime GenAI C API is not thread-safe; calls are serialized through `Mutex`.
unsafe impl Send for GenAiDirectMlInner {}

impl Drop for GenAiDirectMlInner {
    fn drop(&mut self) {
        if !self.tokenizer.is_null() {
            unsafe { (self.api.destroy_tokenizer)(self.tokenizer) };
            self.tokenizer = ptr::null_mut();
        }
        if !self.model.is_null() {
            unsafe { (self.api.destroy_model)(self.model) };
            self.model = ptr::null_mut();
        }
    }
}

pub struct GenAiDirectMlGenerator {
    inner: Arc<Mutex<GenAiDirectMlInner>>,
}

impl GenAiDirectMlGenerator {
    pub fn new(model_dir: &Path, directml_device_id: Option<i32>) -> Result<Self, String> {
        let api = genai_api()?;
        let config_path = path_to_cstring(model_dir)?;
        let provider_dml = cstring("dml", "provider")?;

        let mut config_ptr: *mut OgaConfig = ptr::null_mut();
        check_oga(
            &api,
            unsafe { (api.create_config)(config_path.as_ptr(), &mut config_ptr) },
            "OgaCreateConfig",
        )?;
        let config = OgaOwned::new(Arc::clone(&api), config_ptr, api.destroy_config);

        check_oga(
            &api,
            unsafe { (api.config_clear_providers)(config.as_ptr()) },
            "OgaConfigClearProviders",
        )?;
        check_oga(
            &api,
            unsafe { (api.config_append_provider)(config.as_ptr(), provider_dml.as_ptr()) },
            "OgaConfigAppendProvider(dml)",
        )?;

        if let Some(device_id) = directml_device_id {
            if device_id < 0 {
                return Err(format!(
                    "Invalid DirectML device id {device_id}: expected non-negative"
                ));
            }
            check_oga(
                &api,
                unsafe {
                    (api.config_set_hw_device_id)(
                        config.as_ptr(),
                        provider_dml.as_ptr(),
                        device_id as u32,
                    )
                },
                "OgaConfigSetDecoderProviderOptionsHardwareDeviceId",
            )?;
        }

        let mut model_ptr: *mut OgaModel = ptr::null_mut();
        check_oga(
            &api,
            unsafe { (api.create_model_from_config)(config.as_ptr(), &mut model_ptr) },
            "OgaCreateModelFromConfig",
        )?;
        let model = OgaOwned::new(Arc::clone(&api), model_ptr, api.destroy_model);

        let mut tokenizer_ptr: *mut OgaTokenizer = ptr::null_mut();
        check_oga(
            &api,
            unsafe { (api.create_tokenizer)(model.as_ptr(), &mut tokenizer_ptr) },
            "OgaCreateTokenizer",
        )?;
        let tokenizer = OgaOwned::new(Arc::clone(&api), tokenizer_ptr, api.destroy_tokenizer);

        let eos_token_ids = read_eos_token_ids(&api, tokenizer.as_ptr())?;
        let model_ptr = model.into_raw();
        let tokenizer_ptr = tokenizer.into_raw();

        Ok(Self {
            inner: Arc::new(Mutex::new(GenAiDirectMlInner {
                api,
                model: model_ptr,
                tokenizer: tokenizer_ptr,
                eos_token_ids,
            })),
        })
    }

    pub fn run_preflight(&self, prompt: &str) -> Result<(), String> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| "GenAI DirectML state mutex poisoned".to_string())?;

        let mut sequences_ptr: *mut OgaSequences = ptr::null_mut();
        check_oga(
            &guard.api,
            unsafe { (guard.api.create_sequences)(&mut sequences_ptr) },
            "OgaCreateSequences",
        )?;
        let sequences = OgaOwned::new(
            Arc::clone(&guard.api),
            sequences_ptr,
            guard.api.destroy_sequences,
        );

        let prompt_cstr = cstring(prompt, "prompt")?;
        check_oga(
            &guard.api,
            unsafe {
                (guard.api.tokenizer_encode)(
                    guard.tokenizer,
                    prompt_cstr.as_ptr(),
                    sequences.as_ptr(),
                )
            },
            "OgaTokenizerEncode",
        )?;

        let prompt_tokens =
            unsafe { (guard.api.sequences_get_sequence_count)(sequences.as_ptr(), 0) };
        let max_length = prompt_tokens.saturating_add(1);

        let params = create_generator_params(&guard, max_length as f64, 0.0, Some(1), None, 1.0)?;
        let generator = create_generator(&guard, params.as_ptr(), sequences.as_ptr())?;

        check_oga(
            &guard.api,
            unsafe { (guard.api.generator_generate_next_token)(generator.as_ptr()) },
            "OgaGenerator_GenerateNextToken(preflight)",
        )?;

        ensure_finite_logits(&guard, generator.as_ptr(), "DirectML preflight")
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
            .map_err(|e| format!("DirectML generation worker join error: {e}"))?
    }
}

fn generate_stream_blocking(
    inner: Arc<Mutex<GenAiDirectMlInner>>,
    prompt: String,
    gen_config: GenerationConfig,
    cancelled: Arc<AtomicBool>,
    token_tx: UnboundedSender<String>,
) -> Result<GenerationMetrics, String> {
    let guard = inner
        .lock()
        .map_err(|_| "GenAI DirectML state mutex poisoned".to_string())?;
    let start = Instant::now();

    let mut sequences_ptr: *mut OgaSequences = ptr::null_mut();
    check_oga(
        &guard.api,
        unsafe { (guard.api.create_sequences)(&mut sequences_ptr) },
        "OgaCreateSequences",
    )?;
    let sequences = OgaOwned::new(
        Arc::clone(&guard.api),
        sequences_ptr,
        guard.api.destroy_sequences,
    );

    let prompt_cstr = cstring(&prompt, "prompt")?;
    check_oga(
        &guard.api,
        unsafe {
            (guard.api.tokenizer_encode)(guard.tokenizer, prompt_cstr.as_ptr(), sequences.as_ptr())
        },
        "OgaTokenizerEncode",
    )?;

    let prompt_tokens = unsafe { (guard.api.sequences_get_sequence_count)(sequences.as_ptr(), 0) };
    let max_length = prompt_tokens.saturating_add(gen_config.max_length);

    let params = create_generator_params(
        &guard,
        max_length as f64,
        gen_config.temperature,
        gen_config.top_k,
        gen_config.top_p,
        gen_config.repetition_penalty,
    )?;
    let generator = create_generator(&guard, params.as_ptr(), sequences.as_ptr())?;

    let mut stream_ptr: *mut OgaTokenizerStream = ptr::null_mut();
    check_oga(
        &guard.api,
        unsafe { (guard.api.create_tokenizer_stream)(guard.tokenizer, &mut stream_ptr) },
        "OgaCreateTokenizerStream",
    )?;
    let stream = OgaOwned::new(
        Arc::clone(&guard.api),
        stream_ptr,
        guard.api.destroy_tokenizer_stream,
    );

    let mut total_tokens = 0usize;
    let mut first_token_time_ms: Option<u64> = None;

    while total_tokens < gen_config.max_length {
        if cancelled.load(Ordering::SeqCst) {
            break;
        }
        let done = unsafe { (guard.api.generator_is_done)(generator.as_ptr()) };
        if done {
            break;
        }

        check_oga(
            &guard.api,
            unsafe { (guard.api.generator_generate_next_token)(generator.as_ptr()) },
            "OgaGenerator_GenerateNextToken",
        )?;

        let mut next_tokens_ptr: *const i32 = ptr::null();
        let mut next_tokens_count = 0usize;
        check_oga(
            &guard.api,
            unsafe {
                (guard.api.generator_get_next_tokens)(
                    generator.as_ptr(),
                    &mut next_tokens_ptr,
                    &mut next_tokens_count,
                )
            },
            "OgaGenerator_GetNextTokens",
        )?;

        if next_tokens_count == 0 || next_tokens_ptr.is_null() {
            break;
        }

        let next_tokens = unsafe { std::slice::from_raw_parts(next_tokens_ptr, next_tokens_count) };
        let mut should_stop = false;

        for &token in next_tokens {
            if guard.eos_token_ids.contains(&token) {
                should_stop = true;
                break;
            }

            if first_token_time_ms.is_none() {
                first_token_time_ms = Some(start.elapsed().as_millis() as u64);
            }

            let mut decoded_ptr: *const c_char = ptr::null();
            check_oga(
                &guard.api,
                unsafe {
                    (guard.api.tokenizer_stream_decode)(stream.as_ptr(), token, &mut decoded_ptr)
                },
                "OgaTokenizerStreamDecode",
            )?;

            if !decoded_ptr.is_null() {
                let piece = unsafe { CStr::from_ptr(decoded_ptr) }
                    .to_string_lossy()
                    .into_owned();
                if !piece.is_empty() && token_tx.send(piece).is_err() {
                    should_stop = true;
                    break;
                }
            }

            total_tokens += 1;
            if total_tokens >= gen_config.max_length {
                should_stop = true;
                break;
            }
        }

        if should_stop {
            break;
        }
    }

    let total_time_ms = start.elapsed().as_millis() as u64;
    let tokens_per_second = if total_time_ms == 0 {
        0.0
    } else {
        total_tokens as f64 / (total_time_ms as f64 / 1_000.0)
    };

    Ok(GenerationMetrics {
        total_tokens,
        time_to_first_token_ms: first_token_time_ms,
        tokens_per_second,
        total_time_ms,
    })
}

fn create_generator_params(
    guard: &GenAiDirectMlInner,
    max_length: f64,
    temperature: f32,
    top_k: Option<usize>,
    top_p: Option<f32>,
    repetition_penalty: f32,
) -> Result<OgaOwned<OgaGeneratorParams>, String> {
    let mut params_ptr: *mut OgaGeneratorParams = ptr::null_mut();
    check_oga(
        &guard.api,
        unsafe { (guard.api.create_generator_params)(guard.model, &mut params_ptr) },
        "OgaCreateGeneratorParams",
    )?;
    let params = OgaOwned::new(
        Arc::clone(&guard.api),
        params_ptr,
        guard.api.destroy_generator_params,
    );

    set_search_number(&guard.api, params.as_ptr(), "max_length", max_length)?;

    let do_sample = temperature > 0.0;
    set_search_bool(&guard.api, params.as_ptr(), "do_sample", do_sample)?;

    if do_sample {
        set_search_number(
            &guard.api,
            params.as_ptr(),
            "temperature",
            temperature as f64,
        )?;
    }

    if let Some(top_k) = top_k {
        set_search_number(&guard.api, params.as_ptr(), "top_k", top_k as f64)?;
    }

    if let Some(top_p) = top_p {
        set_search_number(&guard.api, params.as_ptr(), "top_p", top_p as f64)?;
    }

    if repetition_penalty.is_finite() && repetition_penalty > 0.0 {
        set_search_number(
            &guard.api,
            params.as_ptr(),
            "repetition_penalty",
            repetition_penalty as f64,
        )?;
    }

    Ok(params)
}

fn create_generator(
    guard: &GenAiDirectMlInner,
    params: *const OgaGeneratorParams,
    sequences: *const OgaSequences,
) -> Result<OgaOwned<OgaGenerator>, String> {
    let mut generator_ptr: *mut OgaGenerator = ptr::null_mut();
    check_oga(
        &guard.api,
        unsafe { (guard.api.create_generator)(guard.model, params, &mut generator_ptr) },
        "OgaCreateGenerator",
    )?;
    let generator = OgaOwned::new(
        Arc::clone(&guard.api),
        generator_ptr,
        guard.api.destroy_generator,
    );

    check_oga(
        &guard.api,
        unsafe { (guard.api.generator_append_token_sequences)(generator.as_ptr(), sequences) },
        "OgaGenerator_AppendTokenSequences",
    )?;

    Ok(generator)
}

fn set_search_number(
    api: &GenAiApi,
    params: *mut OgaGeneratorParams,
    key: &str,
    value: f64,
) -> Result<(), String> {
    let key_cstr = cstring(key, "search option key")?;
    check_oga(
        api,
        unsafe { (api.generator_params_set_search_number)(params, key_cstr.as_ptr(), value) },
        &format!("OgaGeneratorParamsSetSearchNumber({key})"),
    )
}

fn set_search_bool(
    api: &GenAiApi,
    params: *mut OgaGeneratorParams,
    key: &str,
    value: bool,
) -> Result<(), String> {
    let key_cstr = cstring(key, "search option key")?;
    check_oga(
        api,
        unsafe { (api.generator_params_set_search_bool)(params, key_cstr.as_ptr(), value) },
        &format!("OgaGeneratorParamsSetSearchBool({key})"),
    )
}

fn ensure_finite_logits(
    guard: &GenAiDirectMlInner,
    generator: *mut OgaGenerator,
    context: &str,
) -> Result<(), String> {
    let mut tensor_ptr: *mut OgaTensor = ptr::null_mut();
    check_oga(
        &guard.api,
        unsafe { (guard.api.generator_get_logits)(generator, &mut tensor_ptr) },
        "OgaGenerator_GetLogits",
    )?;
    let tensor = OgaOwned::new(Arc::clone(&guard.api), tensor_ptr, guard.api.destroy_tensor);

    let mut dtype = OgaElementType::Undefined;
    check_oga(
        &guard.api,
        unsafe { (guard.api.tensor_get_type)(tensor.as_ptr(), &mut dtype) },
        "OgaTensorGetType",
    )?;

    let mut rank = 0usize;
    check_oga(
        &guard.api,
        unsafe { (guard.api.tensor_get_shape_rank)(tensor.as_ptr(), &mut rank) },
        "OgaTensorGetShapeRank",
    )?;

    let mut dims = vec![0i64; rank];
    if rank > 0 {
        check_oga(
            &guard.api,
            unsafe { (guard.api.tensor_get_shape)(tensor.as_ptr(), dims.as_mut_ptr(), rank) },
            "OgaTensorGetShape",
        )?;
    }

    let (last_row_start, last_row_end) = last_logits_row_bounds(&dims)?;

    let mut data_ptr: *mut c_void = ptr::null_mut();
    check_oga(
        &guard.api,
        unsafe { (guard.api.tensor_get_data)(tensor.as_ptr(), &mut data_ptr) },
        "OgaTensorGetData",
    )?;

    if data_ptr.is_null() {
        return Err(format!("{context}: logits data pointer is null"));
    }

    match dtype {
        OgaElementType::Float32 => {
            let data = unsafe { std::slice::from_raw_parts(data_ptr as *const f32, last_row_end) };
            validate_finite_slice_f32(&data[last_row_start..last_row_end], context)
        }
        OgaElementType::Float16 => {
            let raw = unsafe { std::slice::from_raw_parts(data_ptr as *const u16, last_row_end) };
            validate_finite_slice_f16(&raw[last_row_start..last_row_end], context)
        }
        other => Err(format!(
            "{context}: unsupported logits tensor element type {other:?}"
        )),
    }
}

fn last_logits_row_bounds(dims: &[i64]) -> Result<(usize, usize), String> {
    if dims.is_empty() {
        return Err("Logits tensor rank must be at least 1".to_string());
    }

    let mut dims_usize = Vec::with_capacity(dims.len());
    for &dim in dims {
        if dim <= 0 {
            return Err(format!("Non-positive tensor dim in logits: {dim}"));
        }
        dims_usize.push(dim as usize);
    }

    let row_width = *dims_usize
        .last()
        .ok_or_else(|| "Logits tensor missing final dimension".to_string())?;
    let row_count = dims_usize[..dims_usize.len().saturating_sub(1)]
        .iter()
        .try_fold(1usize, |acc, &dim| {
            acc.checked_mul(dim)
                .ok_or_else(|| "Overflow while calculating logits row count".to_string())
        })?;
    let last_row_index = row_count
        .checked_sub(1)
        .ok_or_else(|| "Logits tensor has zero rows".to_string())?;
    let start = last_row_index
        .checked_mul(row_width)
        .ok_or_else(|| "Overflow while calculating logits last-row offset".to_string())?;
    let end = row_count
        .checked_mul(row_width)
        .ok_or_else(|| "Overflow while calculating logits element count".to_string())?;
    Ok((start, end))
}

fn validate_finite_slice_f32(data: &[f32], context: &str) -> Result<(), String> {
    if let Some((idx, &value)) = data
        .iter()
        .enumerate()
        .find(|(_, value)| !value.is_finite())
    {
        let non_finite = data.iter().filter(|value| !value.is_finite()).count();
        return Err(format!(
            "{context}: Non-finite logits detected (count={non_finite}, first_index={idx}, first_value={value})"
        ));
    }
    Ok(())
}

fn validate_finite_slice_f16(data: &[u16], context: &str) -> Result<(), String> {
    let mut non_finite = 0usize;
    let mut first: Option<(usize, f32)> = None;

    for (idx, &bits) in data.iter().enumerate() {
        let value = f16::from_bits(bits).to_f32();
        if !value.is_finite() {
            non_finite += 1;
            if first.is_none() {
                first = Some((idx, value));
            }
        }
    }

    if let Some((idx, value)) = first {
        return Err(format!(
            "{context}: Non-finite logits detected (count={non_finite}, first_index={idx}, first_value={value})"
        ));
    }

    Ok(())
}

fn read_eos_token_ids(api: &GenAiApi, tokenizer: *mut OgaTokenizer) -> Result<Vec<i32>, String> {
    let mut eos_ptr: *const i32 = ptr::null();
    let mut eos_count = 0usize;
    check_oga(
        api,
        unsafe { (api.tokenizer_get_eos_token_ids)(tokenizer, &mut eos_ptr, &mut eos_count) },
        "OgaTokenizerGetEosTokenIds",
    )?;

    if eos_ptr.is_null() || eos_count == 0 {
        return Ok(Vec::new());
    }

    let eos = unsafe { std::slice::from_raw_parts(eos_ptr, eos_count) };
    Ok(eos.to_vec())
}

static GENAI_API: Mutex<Option<Arc<GenAiApi>>> = Mutex::new(None);

fn genai_api() -> Result<Arc<GenAiApi>, String> {
    let mut guard = match GENAI_API.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::warn!("Recovering from poisoned GenAI API cache mutex");
            poisoned.into_inner()
        }
    };

    if let Some(api) = guard.as_ref() {
        return Ok(Arc::clone(api));
    }

    let api = GenAiApi::load()?;
    *guard = Some(Arc::clone(&api));
    Ok(api)
}

#[cfg(test)]
mod tests {
    use super::last_logits_row_bounds;

    #[test]
    fn last_logits_row_bounds_rank1_tensor() {
        let (start, end) = last_logits_row_bounds(&[8]).expect("rank-1 logits bounds");
        assert_eq!(start, 0);
        assert_eq!(end, 8);
    }

    #[test]
    fn last_logits_row_bounds_rank3_tensor() {
        let (start, end) = last_logits_row_bounds(&[1, 3, 5]).expect("rank-3 logits bounds");
        assert_eq!(start, 10);
        assert_eq!(end, 15);
    }

    #[test]
    fn last_logits_row_bounds_rejects_non_positive_dims() {
        let err = last_logits_row_bounds(&[2, 0, 4]).expect_err("non-positive dim should fail");
        assert!(err.contains("Non-positive"));
    }

    #[test]
    fn last_logits_row_bounds_rejects_empty_dims() {
        let err = last_logits_row_bounds(&[]).expect_err("empty dims should fail");
        assert!(err.contains("rank"));
    }
}
