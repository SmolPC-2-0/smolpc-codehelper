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
pub mod session;
pub mod tokenizer;
pub mod types;

#[cfg(test)]
pub mod benchmark;

// Re-export commonly used types
pub use backend::InferenceBackend;
#[cfg(target_os = "windows")]
pub use genai::GenAiDirectMlGenerator;
pub use generator::Generator;
pub use runtime_adapter::InferenceRuntimeAdapter;
pub use session::InferenceSession;
pub use tokenizer::TokenizerWrapper;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ORT_INIT: OnceLock<Result<(), String>> = OnceLock::new();

/// Initialize ONNX Runtime with the bundled library.
/// Must be called before any `ort` operations.
///
/// `resource_dir` is the Tauri resource directory (from `app.path().resource_dir()`).
/// Pass `None` when running outside of Tauri (e.g., tests, CLI tools).
///
/// The result is cached — subsequent calls return the stored outcome without re-initializing.
pub fn init_onnx_runtime(resource_dir: Option<&Path>) -> Result<(), String> {
    ORT_INIT
        .get_or_init(|| {
            let dylib_path = find_onnxruntime_dylib(resource_dir);
            log::info!("Initializing ONNX Runtime from: {}", dylib_path.display());

            #[cfg(target_os = "windows")]
            {
                // Keep GenAI lookup anchored to the exact ORT directory to avoid mixed DLL sets.
                if let Some(parent) = dylib_path.parent() {
                    std::env::set_var("SMOLPC_ORT_DYLIB_DIR", parent);
                }
            }

            #[cfg(target_os = "windows")]
            preload_directml_dll(resource_dir, &dylib_path);

            match ort::init_from(dylib_path.to_string_lossy().to_string()) {
                Ok(builder) => {
                    if builder.commit() {
                        log::info!("ONNX Runtime initialized successfully");
                    } else {
                        log::info!("ONNX Runtime already initialized");
                    }
                    Ok(())
                }
                Err(e) => Err(format!("Failed to initialize ONNX Runtime: {}", e)),
            }
        })
        .clone()
}

/// Find the ONNX Runtime library by searching several locations in priority order:
/// 1. Tauri resource dir (production bundles)
/// 2. Next to executable (Windows production, manual placement)
/// 3. Local libs/ directory (development — relative to src-tauri/)
/// 4. Legacy extracted path (backward compat with ort-extracted/)
/// 5. Bare filename (system PATH / DYLD_LIBRARY_PATH fallback)
fn find_onnxruntime_dylib(resource_dir: Option<&Path>) -> PathBuf {
    #[cfg(target_os = "windows")]
    let dylib_name = "onnxruntime.dll";
    #[cfg(target_os = "macos")]
    let dylib_name = "libonnxruntime.dylib";
    #[cfg(target_os = "linux")]
    let dylib_name = "libonnxruntime.so";

    // 1. Tauri resource directory (production)
    if let Some(res_dir) = resource_dir {
        let path = res_dir.join("libs").join(dylib_name);
        if path.exists() {
            return path;
        }
    }

    // 2. Next to executable (Windows production, or manual placement)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let path = exe_dir.join(dylib_name);
            if path.exists() {
                return path;
            }
        }
    }

    // 3. Local libs/ directory (development — relative to src-tauri/)
    let dev_path = PathBuf::from("libs").join(dylib_name);
    if dev_path.exists() {
        return dev_path;
    }

    // 4. Legacy extracted path (backward compat)
    let legacy_path =
        PathBuf::from("../ort-extracted/onnxruntime-win-x64-1.22.1/lib").join(dylib_name);
    if legacy_path.exists() {
        return legacy_path;
    }

    // 5. Bare name — rely on system PATH / DYLD_LIBRARY_PATH
    PathBuf::from(dylib_name)
}

#[cfg(target_os = "windows")]
fn preload_directml_dll(resource_dir: Option<&Path>, ort_dylib_path: &Path) {
    let mut candidates = Vec::new();

    if let Some(res_dir) = resource_dir {
        candidates.push(res_dir.join("libs").join("DirectML.dll"));
    }

    if let Some(parent) = ort_dylib_path.parent() {
        candidates.push(parent.join("DirectML.dll"));
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("DirectML.dll"));
        }
    }

    candidates.push(PathBuf::from("libs").join("DirectML.dll"));
    let attempted_paths: Vec<String> = candidates
        .iter()
        .map(|candidate| candidate.display().to_string())
        .collect();

    let mut seen = std::collections::HashSet::new();
    for candidate in candidates {
        let key = candidate.to_string_lossy().to_string();
        if !seen.insert(key) {
            continue;
        }
        if !candidate.exists() {
            continue;
        }

        // Keep the library loaded for process lifetime; ORT/DirectML expects it to stay resident.
        match unsafe { libloading::Library::new(&candidate) } {
            Ok(lib) => {
                // SAFETY: DirectML/ORT expect this module to remain loaded for the process
                // lifetime. We intentionally leak this handle so the DLL stays resident.
                std::mem::forget(lib);
                log::info!("Preloaded DirectML.dll from {}", candidate.display());
                return;
            }
            Err(e) => {
                log::debug!(
                    "Failed to preload DirectML.dll from {}: {}",
                    candidate.display(),
                    e
                );
            }
        }
    }

    log::warn!(
        "DirectML.dll preload failed from deterministic paths; DirectML backend may be unavailable. attempted_paths={:?}",
        attempted_paths
    );
}
