// Disable console window on Windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod addon_sync;
mod logger;
mod ollama;
mod prompts;
mod rag;
mod scene_bridge;
mod shared_engine;
mod state;

use std::path::PathBuf;
use std::sync::Mutex;
use state::GenerationBackend;
use tauri::Manager;

struct BridgeRuntimeState {
    bridge: Mutex<Option<scene_bridge::SceneBridgeHandle>>,
}

struct EngineSpawnTracker {
    spawned_by_us: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[tauri::command]
fn open_logs(app_handle: tauri::AppHandle) -> Result<(), String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;

    logger::open_logs_directory(&app_data_dir)
}

fn main() {
    let _ = env_logger::Builder::from_default_env().try_init();
    let generation_state = commands::generation::GenerationState::default();
    let setup_generation_state = generation_state.clone();

    let result = tauri::Builder::default()
        .manage(generation_state.clone())
        .setup(move |app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            log::info!("App data directory: {:?}", app_data_dir);

            let log_file = logger::setup_log_file(&app_data_dir).expect("Failed to setup log file");
            log::info!("Server logs will be written to: {:?}", log_file);
            let _ = logger::append_log_line(&log_file, "Blender Helper backend starting");

            match addon_sync::sync_blender_addon() {
                Ok(report) => {
                    if let Some(root) = report.config_root {
                        log::info!("[AddonSync] Blender config root: {:?}", root);
                    } else {
                        log::info!("[AddonSync] Blender config root unavailable for this OS");
                    }

                    if report.scanned_versions == 0 {
                        log::info!("[AddonSync] No Blender versions detected; skipped addon sync");
                    } else {
                        log::info!(
                            "[AddonSync] Synced addon across {} Blender version(s): {} updated, {} unchanged, {} failed",
                            report.scanned_versions,
                            report.updated_targets.len(),
                            report.unchanged_targets.len(),
                            report.failed_targets.len()
                        );
                    }

                    for path in report.updated_targets {
                        log::info!("[AddonSync] Updated addon: {:?}", path);
                    }

                    for (path, err) in report.failed_targets {
                        log::warn!("[AddonSync] Failed to sync {:?}: {}", path, err);
                    }
                }
                Err(err) => {
                    log::warn!("[AddonSync] Failed to scan Blender config directories: {}", err);
                }
            }

            let rag_dir = get_rag_directory(app);
            log::info!("RAG data directory: {:?}", rag_dir);

            let rag_index = rag::index::RagIndex::load_from_dir(&rag_dir);
            if rag_index.is_loaded() {
                log::info!(
                    "[RAG] OK: Loaded {} documentation chunks",
                    rag_index.document_count()
                );
            } else {
                log::info!(
                    "[RAG] Warning: Retrieval disabled ({})",
                    rag_index.load_error().unwrap_or("unknown error")
                );
            }

            let backend_state = state::BackendState::new(rag_index);

            // Resolve resource directory for bundled engine binary + DLLs
            // binaries/ and libs/ sit directly under the resource dir (not under resources/)
            let resource_dir = app.path().resource_dir().ok()
                .or_else(|| {
                    // Fallback: try exe directory
                    std::env::current_exe()
                        .ok()
                        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
                })
                .or_else(|| {
                    // Fallback: try cwd/src-tauri (dev mode)
                    std::env::current_dir()
                        .ok()
                        .map(|cwd| cwd.join("src-tauri"))
                        .filter(|p| p.exists())
                });

            log::info!("[SharedEngine] Resource dir: {:?}", resource_dir);
            if let Some(ref resource_dir) = resource_dir {
                log::info!(
                    "[SharedEngine] binaries/ exists: {}, libs/ exists: {}",
                    resource_dir.join("binaries").exists(),
                    resource_dir.join("libs").exists()
                );
            } else {
                log::info!("[SharedEngine] Warning: No resource directory resolved");
            }

            // Clean up stale engine processes from previous crashes
            shared_engine::cleanup_stale_engine();

            let allow_ollama_fallback = state::allow_ollama_fallback();
            if allow_ollama_fallback {
                log::info!(
                    "[Startup] Ollama fallback is enabled via BLENDER_HELPER_ALLOW_OLLAMA_FALLBACK"
                );
            } else {
                log::info!("[Startup] Self-contained mode active (Ollama fallback disabled)");
            }

            // Start with SharedEngine backend — async task will adjust if needed
            backend_state.set_generation_backend(GenerationBackend::SharedEngine);
            app.manage(backend_state.clone());

            // Spawn engine startup as an async task so the UI is not blocked
            let startup_backend = backend_state.clone();
            let startup_resource_dir = resource_dir.clone();
            let engine_tracker = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let tracker_ref = engine_tracker.clone();

            app.manage(EngineSpawnTracker {
                spawned_by_us: engine_tracker,
            });

            tauri::async_runtime::spawn(async move {
                let state = startup_backend;
                let resource_dir = startup_resource_dir;

                match shared_engine::ensure_engine_running(resource_dir.as_deref()).await {
                    Ok(spawned) => {
                        tracker_ref.store(spawned, std::sync::atomic::Ordering::SeqCst);
                        if spawned {
                            log::info!("[SharedEngine] OK: Engine spawned on 127.0.0.1:19432");
                        } else {
                            log::info!("[SharedEngine] OK: Engine already running on 127.0.0.1:19432");
                        }

                        match shared_engine::ensure_model_loaded().await {
                            Ok(model_id) => {
                                log::info!("[SharedEngine] OK: Model ready '{}'", model_id);
                                state.set_loaded_model_id(Some(model_id));
                                state.set_generation_backend(GenerationBackend::SharedEngine);
                            }
                            Err(err) => {
                                log::info!("[SharedEngine] Warning: Autoload failed ({})", err);
                                if allow_ollama_fallback && ollama::is_ollama_available().await {
                                    log::info!("[Startup] Falling back to Ollama");
                                    state.set_generation_backend(GenerationBackend::Ollama);
                                } else {
                                    log::info!("[Startup] Staying on shared_engine (model load failed)");
                                }
                            }
                        }
                    }
                    Err(err) => {
                        log::info!("[SharedEngine] Warning: Could not start engine ({})", err);
                        if allow_ollama_fallback && ollama::is_ollama_available().await {
                            log::info!("[Startup] Falling back to Ollama");
                            state.set_generation_backend(GenerationBackend::Ollama);
                        } else {
                            log::info!("[Startup] Staying on shared_engine (engine unavailable)");
                        }
                    }
                }
            });

            let bridge = tauri::async_runtime::block_on(scene_bridge::start_scene_bridge(
                backend_state.clone(),
                setup_generation_state.clone(),
            ))
            .map_err(|e| format!("Failed to start scene bridge: {}", e))?;

            app.manage(BridgeRuntimeState {
                bridge: Mutex::new(Some(bridge)),
            });

            log::info!("[SceneBridge] OK: Listening on http://127.0.0.1:5179");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_logs,
            commands::generation::set_generation_backend,
            commands::generation::get_generation_backend,
            commands::generation::inference_generate,
            commands::generation::inference_cancel,
            commands::generation::assistant_stream_ask,
            commands::generation::is_generating,
            commands::assistant::assistant_ask,
            commands::assistant::assistant_analyze_scene,
            commands::assistant::retrieve_rag_context,
            commands::assistant::assistant_status,
            commands::scene::scene_current,
            commands::scene::scene_update,
        ])
        .build(tauri::generate_context!());

    match result {
        Ok(app) => {
            app.run(|app_handle, event| {
                if let tauri::RunEvent::ExitRequested { .. } = event {
                    if let Some(state) = app_handle.try_state::<BridgeRuntimeState>() {
                        if let Ok(mut bridge_guard) = state.bridge.lock() {
                            if let Some(bridge) = bridge_guard.as_mut() {
                                bridge.stop();
                            }
                        }
                    }

                    // Shut down engine if we spawned it
                    if let Some(tracker) = app_handle.try_state::<EngineSpawnTracker>() {
                        if tracker.spawned_by_us.load(std::sync::atomic::Ordering::SeqCst) {
                            log::info!("[SharedEngine] Shutting down engine we spawned...");
                            let _ = tauri::async_runtime::block_on(
                                shared_engine::shutdown_engine(),
                            );
                        }
                    }
                }
            });
        }
        Err(e) => {
            log::error!("[Tauri] Error: Failed to build Tauri app - {}", e);
            std::process::exit(1);
        }
    }
}

/// Gets the RAG system directory
/// In development: uses project root rag_system/
/// In production: uses bundled resources
fn get_rag_directory(app: &tauri::App) -> PathBuf {
    // Check bundled resource paths (tauri.conf.json specifies "resources/rag_system/...")
    if let Ok(resource_dir) = app.path().resource_dir() {
        // Tauri 2 bundles "resources/rag_system/..." relative to the resource dir
        let bundled = resource_dir.join("resources").join("rag_system");
        if bundled.join("simple_db").join("metadata.json").exists() {
            return bundled;
        }
    }

    if let Ok(resource_path) = app
        .path()
        .resolve("resources/rag_system", tauri::path::BaseDirectory::Resource)
    {
        if resource_path.exists() {
            return resource_path;
        }
    }

    if let Ok(resource_path) = app
        .path()
        .resolve("rag_system", tauri::path::BaseDirectory::Resource)
    {
        if resource_path.exists() {
            return resource_path;
        }
    }

    if let Ok(exe_dir) = std::env::current_exe() {
        if let Some(parent) = exe_dir.parent() {
            // Check resources/ subfolder next to executable
            let exe_resources_rag = parent.join("resources").join("rag_system");
            if exe_resources_rag.exists() {
                return exe_resources_rag;
            }

            let exe_rag = parent.join("rag_system");
            if exe_rag.exists() {
                return exe_rag;
            }

            let up_rag = parent.join("_up_").join("rag_system");
            if up_rag.exists() {
                return up_rag;
            }
        }
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let dev_path = cwd.join("rag_system");
    if dev_path.exists() {
        return dev_path;
    }

    let parent_path = cwd.parent().map(|p| p.join("rag_system"));
    if let Some(ref parent_path) = parent_path {
        if parent_path.exists() {
            return parent_path.clone();
        }
    }

    dev_path
}
