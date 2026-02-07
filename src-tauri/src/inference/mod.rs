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
pub mod session;
pub mod tokenizer;
pub mod types;

#[cfg(test)]
pub mod benchmark;

// Re-export commonly used types
pub use generator::Generator;
pub use kv_cache::KVCache;
pub use session::InferenceSession;
pub use tokenizer::TokenizerWrapper;

use std::path::{Path, PathBuf};
use std::sync::Once;

static ORT_INIT: Once = Once::new();

/// Initialize ONNX Runtime with the bundled library.
/// Must be called before any `ort` operations.
///
/// `resource_dir` is the Tauri resource directory (from `app.path().resource_dir()`).
/// Pass `None` when running outside of Tauri (e.g., tests, CLI tools).
pub fn init_onnx_runtime(resource_dir: Option<&Path>) -> Result<(), String> {
    let mut init_result: Result<(), String> = Ok(());

    ORT_INIT.call_once(|| {
        let dylib_path = find_onnxruntime_dylib(resource_dir);
        log::info!("Initializing ONNX Runtime from: {}", dylib_path.display());

        if let Err(e) = ort::init_from(dylib_path.to_string_lossy().to_string()).commit() {
            init_result = Err(format!("Failed to initialize ONNX Runtime: {}", e));
        } else {
            log::info!("ONNX Runtime initialized successfully");
        }
    });

    init_result
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
