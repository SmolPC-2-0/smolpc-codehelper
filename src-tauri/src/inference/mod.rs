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
pub mod kv_cache;
pub mod session;
pub mod tokenizer;
pub mod types;

// Re-export commonly used types
pub use generator::Generator;
pub use kv_cache::KVCache;
pub use session::InferenceSession;
pub use tokenizer::TokenizerWrapper;

use std::path::PathBuf;
use std::sync::Once;

static ORT_INIT: Once = Once::new();

/// Initialize ONNX Runtime with the bundled library.
/// Must be called before any `ort` operations.
pub fn init_onnx_runtime() -> Result<(), String> {
    let mut init_result: Result<(), String> = Ok(());

    ORT_INIT.call_once(|| {
        let dylib_path = find_onnxruntime_dylib();
        log::info!("Initializing ONNX Runtime from: {}", dylib_path.display());

        if let Err(e) = ort::init_from(dylib_path.to_string_lossy().to_string()).commit() {
            init_result = Err(format!("Failed to initialize ONNX Runtime: {}", e));
        } else {
            log::info!("ONNX Runtime initialized successfully");
        }
    });

    init_result
}

/// Find the ONNX Runtime library.
/// Production: next to executable. Development: in ort-extracted folder.
fn find_onnxruntime_dylib() -> PathBuf {
    #[cfg(target_os = "windows")]
    let dylib_name = "onnxruntime.dll";
    #[cfg(target_os = "macos")]
    let dylib_name = "libonnxruntime.dylib";
    #[cfg(target_os = "linux")]
    let dylib_name = "libonnxruntime.so";

    // Production: next to executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let path = exe_dir.join(dylib_name);
            if path.exists() {
                return path;
            }
        }
    }

    // Development: in extracted folder (relative to src-tauri)
    let dev_path =
        PathBuf::from("../ort-extracted/onnxruntime-win-x64-1.22.1/lib").join(dylib_name);
    if dev_path.exists() {
        return dev_path;
    }

    // Last resort: assume it's in current directory or PATH
    PathBuf::from(dylib_name)
}
