use super::super::runtime_loading::{OrtRuntimeBundle, OrtRuntimeLoader, RetainedLibrary};
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::Path;
use std::ptr;
use std::sync::{Arc, Mutex};

pub(super) type OgaResult = c_void;
pub(super) type OgaConfig = c_void;
pub(super) type OgaModel = c_void;
pub(super) type OgaTokenizer = c_void;
pub(super) type OgaTokenizerStream = c_void;
pub(super) type OgaSequences = c_void;
pub(super) type OgaGeneratorParams = c_void;
pub(super) type OgaGenerator = c_void;
pub(super) type OgaTensor = c_void;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Values are written by the GenAI C API through FFI, not constructed in Rust.
pub(super) enum OgaElementType {
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

pub(super) struct GenAiApi {
    pub(super) _genai: RetainedLibrary,
    pub(super) _directml: RetainedLibrary,
    pub(super) result_get_error: unsafe extern "system" fn(*const OgaResult) -> *const c_char,
    pub(super) destroy_result: unsafe extern "system" fn(*mut OgaResult),

    pub(super) create_config:
        unsafe extern "system" fn(*const c_char, *mut *mut OgaConfig) -> *mut OgaResult,
    pub(super) destroy_config: unsafe extern "system" fn(*mut OgaConfig),
    pub(super) config_clear_providers: unsafe extern "system" fn(*mut OgaConfig) -> *mut OgaResult,
    pub(super) config_append_provider:
        unsafe extern "system" fn(*mut OgaConfig, *const c_char) -> *mut OgaResult,
    pub(super) config_set_hw_device_id:
        unsafe extern "system" fn(*mut OgaConfig, *const c_char, u32) -> *mut OgaResult,

    pub(super) create_model_from_config:
        unsafe extern "system" fn(*const OgaConfig, *mut *mut OgaModel) -> *mut OgaResult,
    pub(super) destroy_model: unsafe extern "system" fn(*mut OgaModel),

    pub(super) create_tokenizer:
        unsafe extern "system" fn(*const OgaModel, *mut *mut OgaTokenizer) -> *mut OgaResult,
    pub(super) destroy_tokenizer: unsafe extern "system" fn(*mut OgaTokenizer),
    pub(super) tokenizer_get_eos_token_ids: unsafe extern "system" fn(
        *const OgaTokenizer,
        *mut *const i32,
        *mut usize,
    ) -> *mut OgaResult,
    pub(super) tokenizer_encode: unsafe extern "system" fn(
        *const OgaTokenizer,
        *const c_char,
        *mut OgaSequences,
    ) -> *mut OgaResult,
    pub(super) create_tokenizer_stream: unsafe extern "system" fn(
        *const OgaTokenizer,
        *mut *mut OgaTokenizerStream,
    ) -> *mut OgaResult,
    pub(super) destroy_tokenizer_stream: unsafe extern "system" fn(*mut OgaTokenizerStream),
    pub(super) tokenizer_stream_decode: unsafe extern "system" fn(
        *mut OgaTokenizerStream,
        i32,
        *mut *const c_char,
    ) -> *mut OgaResult,

    pub(super) create_sequences:
        unsafe extern "system" fn(*mut *mut OgaSequences) -> *mut OgaResult,
    pub(super) destroy_sequences: unsafe extern "system" fn(*mut OgaSequences),
    pub(super) sequences_get_sequence_count:
        unsafe extern "system" fn(*const OgaSequences, usize) -> usize,

    pub(super) create_generator_params:
        unsafe extern "system" fn(*const OgaModel, *mut *mut OgaGeneratorParams) -> *mut OgaResult,
    pub(super) destroy_generator_params: unsafe extern "system" fn(*mut OgaGeneratorParams),
    pub(super) generator_params_set_search_number:
        unsafe extern "system" fn(*mut OgaGeneratorParams, *const c_char, f64) -> *mut OgaResult,
    pub(super) generator_params_set_search_bool:
        unsafe extern "system" fn(*mut OgaGeneratorParams, *const c_char, bool) -> *mut OgaResult,

    pub(super) create_generator: unsafe extern "system" fn(
        *const OgaModel,
        *const OgaGeneratorParams,
        *mut *mut OgaGenerator,
    ) -> *mut OgaResult,
    pub(super) destroy_generator: unsafe extern "system" fn(*mut OgaGenerator),
    pub(super) generator_append_token_sequences:
        unsafe extern "system" fn(*mut OgaGenerator, *const OgaSequences) -> *mut OgaResult,
    pub(super) generator_generate_next_token:
        unsafe extern "system" fn(*mut OgaGenerator) -> *mut OgaResult,
    pub(super) generator_get_next_tokens: unsafe extern "system" fn(
        *const OgaGenerator,
        *mut *const i32,
        *mut usize,
    ) -> *mut OgaResult,
    pub(super) generator_is_done: unsafe extern "system" fn(*mut OgaGenerator) -> bool,
    pub(super) generator_get_logits:
        unsafe extern "system" fn(*mut OgaGenerator, *mut *mut OgaTensor) -> *mut OgaResult,

    pub(super) destroy_tensor: unsafe extern "system" fn(*mut OgaTensor),
    pub(super) tensor_get_type:
        unsafe extern "system" fn(*mut OgaTensor, *mut OgaElementType) -> *mut OgaResult,
    pub(super) tensor_get_shape_rank:
        unsafe extern "system" fn(*mut OgaTensor, *mut usize) -> *mut OgaResult,
    pub(super) tensor_get_shape:
        unsafe extern "system" fn(*mut OgaTensor, *mut i64, usize) -> *mut OgaResult,
    pub(super) tensor_get_data:
        unsafe extern "system" fn(*mut OgaTensor, *mut *mut c_void) -> *mut OgaResult,
}

// The API table is immutable after loading and function pointers are thread-safe to read.
unsafe impl Send for GenAiApi {}
unsafe impl Sync for GenAiApi {}

impl GenAiApi {
    pub(super) fn load(bundle: &OrtRuntimeBundle) -> Result<Arc<Self>, String> {
        OrtRuntimeLoader::ensure_initialized(bundle)?;
        let directml = RetainedLibrary::load(&bundle.directml_dll)?;
        let genai = RetainedLibrary::load(&bundle.onnxruntime_genai_dll)?;

        unsafe {
            Ok(Arc::new(Self {
                result_get_error: load_symbol(&genai, b"OgaResultGetError\0")?,
                destroy_result: load_symbol(&genai, b"OgaDestroyResult\0")?,

                create_config: load_symbol(&genai, b"OgaCreateConfig\0")?,
                destroy_config: load_symbol(&genai, b"OgaDestroyConfig\0")?,
                config_clear_providers: load_symbol(&genai, b"OgaConfigClearProviders\0")?,
                config_append_provider: load_symbol(&genai, b"OgaConfigAppendProvider\0")?,
                config_set_hw_device_id: load_symbol(
                    &genai,
                    b"OgaConfigSetDecoderProviderOptionsHardwareDeviceId\0",
                )?,

                create_model_from_config: load_symbol(&genai, b"OgaCreateModelFromConfig\0")?,
                destroy_model: load_symbol(&genai, b"OgaDestroyModel\0")?,

                create_tokenizer: load_symbol(&genai, b"OgaCreateTokenizer\0")?,
                destroy_tokenizer: load_symbol(&genai, b"OgaDestroyTokenizer\0")?,
                tokenizer_get_eos_token_ids: load_symbol(&genai, b"OgaTokenizerGetEosTokenIds\0")?,
                tokenizer_encode: load_symbol(&genai, b"OgaTokenizerEncode\0")?,
                create_tokenizer_stream: load_symbol(&genai, b"OgaCreateTokenizerStream\0")?,
                destroy_tokenizer_stream: load_symbol(&genai, b"OgaDestroyTokenizerStream\0")?,
                tokenizer_stream_decode: load_symbol(&genai, b"OgaTokenizerStreamDecode\0")?,

                create_sequences: load_symbol(&genai, b"OgaCreateSequences\0")?,
                destroy_sequences: load_symbol(&genai, b"OgaDestroySequences\0")?,
                sequences_get_sequence_count: load_symbol(
                    &genai,
                    b"OgaSequencesGetSequenceCount\0",
                )?,

                create_generator_params: load_symbol(&genai, b"OgaCreateGeneratorParams\0")?,
                destroy_generator_params: load_symbol(&genai, b"OgaDestroyGeneratorParams\0")?,
                generator_params_set_search_number: load_symbol(
                    &genai,
                    b"OgaGeneratorParamsSetSearchNumber\0",
                )?,
                generator_params_set_search_bool: load_symbol(
                    &genai,
                    b"OgaGeneratorParamsSetSearchBool\0",
                )?,

                create_generator: load_symbol(&genai, b"OgaCreateGenerator\0")?,
                destroy_generator: load_symbol(&genai, b"OgaDestroyGenerator\0")?,
                generator_append_token_sequences: load_symbol(
                    &genai,
                    b"OgaGenerator_AppendTokenSequences\0",
                )?,
                generator_generate_next_token: load_symbol(
                    &genai,
                    b"OgaGenerator_GenerateNextToken\0",
                )?,
                generator_get_next_tokens: load_symbol(&genai, b"OgaGenerator_GetNextTokens\0")?,
                generator_is_done: load_symbol(&genai, b"OgaGenerator_IsDone\0")?,
                generator_get_logits: load_symbol(&genai, b"OgaGenerator_GetLogits\0")?,

                destroy_tensor: load_symbol(&genai, b"OgaDestroyTensor\0")?,
                tensor_get_type: load_symbol(&genai, b"OgaTensorGetType\0")?,
                tensor_get_shape_rank: load_symbol(&genai, b"OgaTensorGetShapeRank\0")?,
                tensor_get_shape: load_symbol(&genai, b"OgaTensorGetShape\0")?,
                tensor_get_data: load_symbol(&genai, b"OgaTensorGetData\0")?,

                _directml: directml,
                _genai: genai,
            }))
        }
    }
}

pub(super) unsafe fn load_symbol<T: Copy>(lib: &RetainedLibrary, name: &[u8]) -> Result<T, String> {
    lib.get(name)
}

pub(super) fn cstring(value: &str, field: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| format!("{field} contains interior NUL byte"))
}

pub(super) fn path_to_cstring(path: &Path) -> Result<CString, String> {
    let utf8 = path.to_str().ok_or_else(|| {
        format!(
            "Non-UTF8 path is unsupported for GenAI: {}. Move the model to an ASCII-only path.",
            path.display()
        )
    })?;
    cstring(utf8, "path")
}

pub(super) fn check_oga(
    api: &GenAiApi,
    result: *mut OgaResult,
    context: &str,
) -> Result<(), String> {
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

pub(super) struct OgaOwned<T> {
    pub(super) api: Arc<GenAiApi>,
    pub(super) ptr: *mut T,
    pub(super) destroy: unsafe extern "system" fn(*mut T),
}

impl<T> OgaOwned<T> {
    pub(super) fn new(
        api: Arc<GenAiApi>,
        ptr: *mut T,
        destroy: unsafe extern "system" fn(*mut T),
    ) -> Self {
        Self { api, ptr, destroy }
    }

    pub(super) fn as_ptr(&self) -> *mut T {
        self.ptr
    }

    pub(super) fn into_raw(mut self) -> *mut T {
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

// OgaOwned is only passed between threads via Arc<Mutex<GenAiDirectMlInner>> which
// serializes all access, so Send is safe.
unsafe impl<T> Send for OgaOwned<T> {}
unsafe impl<T> Sync for OgaOwned<T> {}

pub(super) fn read_eos_token_ids(
    api: &GenAiApi,
    tokenizer: *mut OgaTokenizer,
) -> Result<Vec<i32>, String> {
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

#[derive(Clone)]
pub(super) enum CachedGenAiApi {
    Success(Arc<GenAiApi>),
    Failure(String),
}

#[derive(Default)]
pub(super) struct GenAiApiState {
    pub(super) active_fingerprint: Option<String>,
    pub(super) results: HashMap<String, CachedGenAiApi>,
}

pub(super) static GENAI_API: std::sync::OnceLock<Mutex<GenAiApiState>> = std::sync::OnceLock::new();

pub(super) fn genai_api(bundle: &OrtRuntimeBundle) -> Result<Arc<GenAiApi>, String> {
    let state = GENAI_API.get_or_init(|| Mutex::new(GenAiApiState::default()));
    let fingerprint = bundle.fingerprint.value.clone();

    {
        let guard = match state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("Recovering from poisoned GenAI API cache mutex");
                poisoned.into_inner()
            }
        };

        if let Some(cached) = guard.results.get(&fingerprint) {
            return match cached {
                CachedGenAiApi::Success(api) => Ok(Arc::clone(api)),
                CachedGenAiApi::Failure(error) => Err(error.clone()),
            };
        }

        if let Some(active) = guard.active_fingerprint.as_ref() {
            if active != &fingerprint {
                let error = format!(
                    "DirectML GenAI already initialized from bundle fingerprint '{active}'; restart the process to use '{fingerprint}'"
                );
                drop(guard);
                let mut guard = match state.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                guard
                    .results
                    .insert(fingerprint, CachedGenAiApi::Failure(error.clone()));
                return Err(error);
            }
        }
    }

    if let Some(failure) = bundle.directml_validation_failure {
        let error = format!(
            "DirectML runtime bundle is not validated ({}) at {}",
            failure.code(),
            bundle.display_root().display()
        );
        let mut guard = match state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard
            .results
            .insert(fingerprint, CachedGenAiApi::Failure(error.clone()));
        return Err(error);
    }

    let api = GenAiApi::load(bundle)?;
    let mut guard = match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.active_fingerprint = Some(fingerprint.clone());
    guard
        .results
        .insert(fingerprint, CachedGenAiApi::Success(Arc::clone(&api)));
    Ok(api)
}
