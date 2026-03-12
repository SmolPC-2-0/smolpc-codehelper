mod commands;
pub mod launcher;

use commands::engine::engine_status;
use commands::launcher::{
    launcher_install_app, launcher_launch_or_focus, launcher_list_apps, launcher_pick_manual_exe,
    launcher_register_manual_path,
};
use launcher::orchestrator::{
    any_app_running, any_app_running_cached, resolve_engine_client, shutdown_engine, LauncherState,
};
use smolpc_engine_client::{StartupMode, StartupPolicy};
use std::path::PathBuf;
use tauri::Manager;

/// Resolve the `libs/` directory that contains bundled runtime DLLs.
/// In dev: `launcher/src-tauri/libs/`
/// In production: next to the packaged executable under `resources/libs/`
fn resolve_libs_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            candidates.push(exe_dir.join("resources").join("libs"));
            candidates.push(exe_dir.join("Resources").join("libs"));
            candidates.push(exe_dir.join("libs"));
            if let Some(parent) = exe_dir.parent() {
                candidates.push(parent.join("resources").join("libs"));
                candidates.push(parent.join("Resources").join("libs"));
            }
        }
    }

    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("libs"));

    candidates
        .into_iter()
        .find(|dir| dir.join("onnxruntime.dll").exists())
}

/// Configure environment variables so the engine host (spawned as a child
/// process) finds the correct DLLs bundled with the launcher, instead of
/// whatever versions might be in System32 or elsewhere on PATH.
///
/// Sets:
/// - `ORT_DYLIB_PATH`      → onnxruntime.dll (used by the `ort` crate)
/// - `SMOLPC_GENAI_DYLIB`  → onnxruntime-genai.dll (used by GenAI DirectML backend)
pub fn configure_runtime_dlls_early() {
    let Some(libs_dir) = resolve_libs_dir() else {
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

            // Eager engine start — non-blocking, UI loads immediately.
            // Restored so launcher boot still warms the engine even if frontend only polls status.
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
                    let apps_running = any_app_running_cached(&monitor_handle, state.inner()).await;

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
            launcher_install_app,
            launcher_register_manual_path,
            launcher_pick_manual_exe,
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
            if label == "main" {
                api.prevent_close();

                if any_app_running(app_handle) {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.hide();
                    }
                    return;
                }

                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<LauncherState>();
                    let _ = shutdown_engine(state.inner()).await;
                    app_handle.exit(0);
                });
            }
        }
    });
}
