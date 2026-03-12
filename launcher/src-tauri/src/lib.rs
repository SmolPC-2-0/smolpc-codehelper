mod commands;
mod launcher;

use commands::engine::engine_status;
use commands::launcher::{launcher_launch_or_focus, launcher_list_apps};
use launcher::orchestrator::{
    any_app_running, resolve_engine_client, shutdown_engine, LauncherState,
};
use smolpc_engine_client::{StartupMode, StartupPolicy};
use std::path::PathBuf;
use tauri::Manager;

/// Resolve the `libs/` directory that contains bundled runtime DLLs.
/// In dev: `launcher/src-tauri/libs/`
/// In production: Tauri resource dir `libs/`
fn resolve_libs_dir(app: &tauri::App) -> Option<PathBuf> {
    let candidates = [
        // Production: resource_dir/libs/
        app.path()
            .resource_dir()
            .ok()
            .map(|d| d.join("libs")),
        // Dev: CARGO_MANIFEST_DIR/libs/
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("libs")),
    ];

    candidates
        .into_iter()
        .flatten()
        .find(|dir| dir.join("onnxruntime.dll").exists())
}

/// Configure environment variables so the engine host (spawned as a child
/// process) finds the correct DLLs bundled with the launcher, instead of
/// whatever versions might be in System32 or elsewhere on PATH.
///
/// Sets:
/// - `ORT_DYLIB_PATH`      → onnxruntime.dll (used by the `ort` crate)
/// - `SMOLPC_GENAI_DYLIB`  → onnxruntime-genai.dll (used by GenAI DirectML backend)
fn configure_runtime_dlls(app: &tauri::App) {
    let Some(libs_dir) = resolve_libs_dir(app) else {
        log::warn!("Could not find bundled libs/ directory; engine host may fail to start");
        return;
    };

    log::info!("Resolved runtime libs directory: {}", libs_dir.display());

    // ORT_DYLIB_PATH — the ort crate uses this to find onnxruntime.dll
    if std::env::var("ORT_DYLIB_PATH").is_err() {
        let ort_dll = libs_dir.join("onnxruntime.dll");
        if ort_dll.exists() {
            log::info!("Setting ORT_DYLIB_PATH → {}", ort_dll.display());
            std::env::set_var("ORT_DYLIB_PATH", &ort_dll);
        }
    } else {
        log::info!("ORT_DYLIB_PATH already set, skipping");
    }

    // SMOLPC_GENAI_DYLIB — engine-core uses this for onnxruntime-genai.dll
    if std::env::var("SMOLPC_GENAI_DYLIB").is_err() {
        let genai_dll = libs_dir.join("onnxruntime-genai.dll");
        if genai_dll.exists() {
            log::info!("Setting SMOLPC_GENAI_DYLIB → {}", genai_dll.display());
            std::env::set_var("SMOLPC_GENAI_DYLIB", &genai_dll);
        }
    } else {
        log::info!("SMOLPC_GENAI_DYLIB already set, skipping");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Debug)
                        .build(),
                )?;
            }

            configure_runtime_dlls(app);

            // Eager engine start — non-blocking, UI loads immediately
            let eager_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = eager_handle.state::<LauncherState>();
                match resolve_engine_client(&eager_handle, state.inner()).await {
                    Ok(client) => {
                        log::info!("Eager engine start: connected");
                        if let Err(e) = client
                            .ensure_started(StartupMode::Auto, StartupPolicy::default())
                            .await
                        {
                            log::warn!("Eager engine start: ensure_started failed: {e}");
                        }
                    }
                    Err(e) => log::warn!("Eager engine start: connect_or_spawn failed: {e}"),
                }
            });

            // Background monitor — shuts down engine when launcher is
            // hidden/closed AND all helper apps are closed.
            let monitor_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = monitor_handle.state::<LauncherState>();

                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                    // Skip if no engine client yet
                    if state.peek_client().await.is_none() {
                        continue;
                    }

                    let window_visible = monitor_handle
                        .get_webview_window("main")
                        .and_then(|w| w.is_visible().ok())
                        .unwrap_or(false);
                    let apps_running = any_app_running(&monitor_handle);

                    // Keep engine alive while launcher is visible or any app is running
                    if window_visible || apps_running {
                        continue;
                    }

                    // Launcher hidden + no apps running → shut down engine and exit
                    log::info!("Launcher hidden and all apps closed. Shutting down engine.");
                    let _ = shutdown_engine(state.inner()).await;
                    log::info!("Engine shut down. Exiting launcher process.");
                    monitor_handle.exit(0);
                    return;
                }
            });

            Ok(())
        })
        .manage(LauncherState::default())
        .invoke_handler(tauri::generate_handler![
            launcher_list_apps,
            launcher_launch_or_focus,
            engine_status,
        ])
        .build(tauri::generate_context!())
        .expect("error building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } = &event
        {
            if label == "main" && any_app_running(app_handle) {
                api.prevent_close();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
        }
    });
}
