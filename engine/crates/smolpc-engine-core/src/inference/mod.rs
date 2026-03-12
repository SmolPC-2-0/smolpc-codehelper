pub mod backend;
pub mod backend_store;
pub mod genai;
/// ONNX Runtime inference engine module
///
/// This module provides the core inference functionality for running LLMs via ONNX Runtime.
/// It replaces the Ollama-based inference system with direct ONNX model execution.
///
/// # Architecture
///
/// - `session`: Wrapper around `ort::Session` for model loading
/// - `tokenizer`: Text <-> token ID conversion using Hugging Face tokenizers
/// - `generator`: Autoregressive generation loop with KV cache management
/// - `sampler`: Token sampling strategies (greedy, temperature, top-k, top-p)
/// - `types`: Shared type definitions
pub mod generator;
pub mod input_builder;
pub mod kv_cache;
pub mod runtime_adapter;
pub mod runtime_loading;
pub mod session;
pub mod tokenizer;
pub mod types;

#[cfg(test)]
pub mod benchmark;

pub use backend::InferenceBackend;
#[cfg(target_os = "windows")]
pub use genai::GenAiDirectMlGenerator;
#[cfg(target_os = "windows")]
pub use genai::{OpenVinoGenAiGenerator, OpenVinoNpuPipelineConfig};
pub use generator::Generator;
pub use runtime_adapter::InferenceRuntimeAdapter;
pub use runtime_loading::{
    BundleValidationFailureClass, OpenVinoDeviceProbe, OpenVinoRuntimeBundle,
    OpenVinoRuntimeLoader, OrtRuntimeBundle, OrtRuntimeLoader, RequiredRuntimeFile,
    RuntimeBundleFingerprint, RuntimeFamily, RuntimeVersionMetadata,
};
pub use session::InferenceSession;
pub use tokenizer::TokenizerWrapper;

use std::path::{Path, PathBuf};

pub fn init_onnx_runtime(resource_dir: Option<&Path>) -> Result<(), String> {
    let bundle = resolve_compat_ort_bundle(resource_dir);
    OrtRuntimeLoader::ensure_initialized(&bundle).map(|_| ())
}

fn resolve_compat_ort_bundle(resource_dir: Option<&Path>) -> OrtRuntimeBundle {
    let mut candidates = Vec::new();
    if let Some(resource_dir) = resource_dir {
        candidates.push(resource_dir.join("libs"));
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("libs"));
        }
    }
    if cfg!(debug_assertions) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if let Some(workspace_root) = manifest_dir.parent().and_then(|parent| parent.parent()) {
            candidates.push(
                workspace_root
                    .join("apps")
                    .join("codehelper")
                    .join("src-tauri")
                    .join("libs"),
            );
            candidates.push(workspace_root.join("src-tauri").join("libs"));
        }
    }

    let mut bundles = candidates
        .into_iter()
        .map(build_compat_ort_bundle)
        .collect::<Vec<_>>();
    if bundles.is_empty() {
        return build_compat_ort_bundle(missing_runtime_root("libs"));
    }
    bundles.sort_by_key(|bundle| {
        (
            if bundle.directml_validated() {
                3
            } else if bundle.genai_validated() {
                2
            } else if bundle.ort_validated() {
                1
            } else {
                0
            },
            bundle.canonical_root.is_some(),
        )
    });
    bundles.pop().expect("non-empty bundles")
}

fn build_compat_ort_bundle(bundle_root: PathBuf) -> OrtRuntimeBundle {
    let canonical_root = bundle_root.canonicalize().ok();
    let onnxruntime_dll = bundle_root.join("onnxruntime.dll");
    let providers_shared = bundle_root.join("onnxruntime_providers_shared.dll");
    let genai_dll = bundle_root.join("onnxruntime-genai.dll");
    let directml_dll = bundle_root.join("DirectML.dll");
    let required_files = vec![
        RequiredRuntimeFile::new("onnxruntime.dll", onnxruntime_dll.clone()),
        RequiredRuntimeFile::new("onnxruntime_providers_shared.dll", providers_shared.clone()),
        RequiredRuntimeFile::new("onnxruntime-genai.dll", genai_dll.clone()),
        RequiredRuntimeFile::new("DirectML.dll", directml_dll.clone()),
    ];
    let version_metadata = vec![
        RuntimeVersionMetadata::new("ort-crate", backend::ORT_CRATE_VERSION),
        RuntimeVersionMetadata::new("ort-genai", "bundled"),
    ];
    let ort_validation_failure = if !bundle_root.exists() {
        Some(BundleValidationFailureClass::MissingRoot)
    } else if canonical_root.is_none() {
        Some(BundleValidationFailureClass::CanonicalizationFailed)
    } else if !onnxruntime_dll.exists() {
        Some(BundleValidationFailureClass::OrtCoreMissing)
    } else if !providers_shared.exists() {
        Some(BundleValidationFailureClass::OrtProvidersSharedMissing)
    } else {
        None
    };
    let genai_validation_failure = ort_validation_failure
        .or_else(|| (!genai_dll.exists()).then_some(BundleValidationFailureClass::OrtGenAiMissing));
    let directml_validation_failure = genai_validation_failure.or_else(|| {
        (!directml_dll.exists()).then_some(BundleValidationFailureClass::DirectMlMissing)
    });
    let fingerprint = RuntimeBundleFingerprint::new(
        RuntimeFamily::Ort,
        canonical_root.clone(),
        &bundle_root,
        &required_files,
        &version_metadata,
    );

    OrtRuntimeBundle {
        bundle_root,
        canonical_root,
        onnxruntime_dll,
        onnxruntime_providers_shared_dll: providers_shared,
        onnxruntime_genai_dll: genai_dll,
        directml_dll,
        required_files,
        version_metadata,
        ort_validation_failure,
        genai_validation_failure,
        directml_validation_failure,
        fingerprint,
    }
}

fn missing_runtime_root(name: &str) -> PathBuf {
    std::env::temp_dir()
        .join("smolpc-runtime-missing")
        .join(name)
}
