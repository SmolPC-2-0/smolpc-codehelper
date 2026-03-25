use super::blender::ensure_blender_addon_prepared;
use super::gimp::ensure_gimp_plugin_runtime_prepared;
use super::host_apps::{detect_blender_with_policy, detect_gimp_with_policy};
use super::python::prepare_bundled_python;
use super::state::SetupState;
use super::status::collect_setup_status;
use smolpc_connector_common::{SETUP_ITEM_HOST_BLENDER, SETUP_ITEM_HOST_GIMP};
use std::path::Path;
use std::path::PathBuf;

pub async fn prepare_setup(state: &SetupState) -> SetupResult {
    state.load_cache_from_disk_if_needed().await;

    let (cached_blender_path, cached_gimp_path) = {
        let cache = state.cache().await;
        (
            cache
                .resolved_host_apps
                .get(SETUP_ITEM_HOST_BLENDER)
                .cloned(),
            cache.resolved_host_apps.get(SETUP_ITEM_HOST_GIMP).cloned(),
        )
    };
    let result = prepare_setup_inner(
        state,
        cached_blender_path.as_ref(),
        cached_gimp_path.as_ref(),
    );
    let should_persist_last_error = {
        let mut cache = state.cache().await;
        let next_error = result.err();
        let changed = cache.last_error != next_error;
        cache.last_error = next_error;
        changed
    };

    // Persist `last_error` when it changes so troubleshooting context survives relaunches.
    if should_persist_last_error {
        state.persist_cache_to_disk().await;
    }

    collect_setup_status(state).await
}

type SetupResult = smolpc_assistant_types::SetupStatusDto;

fn prepare_setup_inner(
    state: &SetupState,
    cached_blender_path: Option<&PathBuf>,
    cached_gimp_path: Option<&PathBuf>,
) -> Result<(), String> {
    let setup_root = state
        .setup_root()
        .ok_or_else(|| "Tauri app-local-data directory is unavailable".to_string())?;
    prepare_setup_directories(&setup_root)?;
    prepare_bundled_python(state.resource_dir(), state.app_local_data_dir())?;
    let blender_detection = detect_blender_with_policy(
        cached_blender_path.map(PathBuf::as_path),
        state.allow_system_host_detection(),
    );
    if let Some(blender_path) = blender_detection.path.as_deref() {
        let _ = ensure_blender_addon_prepared(
            state.resource_dir(),
            state.app_local_data_dir(),
            blender_path,
        )?;
    }
    let gimp_detection = detect_gimp_with_policy(
        cached_gimp_path.map(PathBuf::as_path),
        state.allow_system_host_detection(),
    );
    if let Some(gimp_path) = gimp_detection.path.as_deref() {
        let _outcome = ensure_gimp_plugin_runtime_prepared(
            state.resource_dir(),
            state.app_local_data_dir(),
            gimp_path,
        )?;
    }
    Ok(())
}

fn prepare_setup_directories(setup_root: &Path) -> Result<(), String> {
    for relative in ["python", "state", "logs"] {
        let path = setup_root.join(relative);
        std::fs::create_dir_all(&path).map_err(|error| {
            format!(
                "Failed to create setup directory {}: {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::prepare_setup;
    use crate::setup::state::SetupState;
    use smolpc_connector_common::{
        SETUP_ITEM_GIMP_PLUGIN_RUNTIME, SETUP_ITEM_HOST_BLENDER, SETUP_ITEM_HOST_GIMP,
    };
    use smolpc_assistant_types::SetupItemStateDto;
    use std::path::Path;
    use std::sync::{Mutex as StdMutex, OnceLock};
    use tempfile::TempDir;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    static ENV_LOCK: OnceLock<StdMutex<()>> = OnceLock::new();

    fn env_lock() -> &'static StdMutex<()> {
        ENV_LOCK.get_or_init(|| StdMutex::new(()))
    }

    fn with_profile_env<T>(base: &Path, callback: impl FnOnce() -> T) -> T {
        let _guard = env_lock().lock().expect("env lock");

        #[cfg(windows)]
        let key = "APPDATA";
        #[cfg(not(windows))]
        let key = "HOME";

        let original = std::env::var_os(key);
        std::env::set_var(key, base);
        let result = callback();
        match original {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
        result
    }

    fn write_blender_resources(root: &Path) {
        let blender_root = root.join("blender");
        std::fs::create_dir_all(blender_root.join("addon")).expect("addon dir");
        std::fs::create_dir_all(blender_root.join("rag_system")).expect("rag dir");
        std::fs::write(blender_root.join("README.md"), "phase4 blender resources").expect("readme");
        std::fs::write(
            blender_root.join("addon").join("blender_helper_http.py"),
            "# addon snapshot\n",
        )
        .expect("addon");
        std::fs::write(
            blender_root.join("manifest.json"),
            r#"{
              "version": "phase4-blender",
              "source": "tests",
              "expectedPaths": ["README.md", "rag_system", "addon/blender_helper_http.py"],
              "status": "tracked"
            }"#,
        )
        .expect("manifest");
    }

    #[cfg(unix)]
    fn write_fake_blender_script(
        root: &Path,
        addon_dir: &Path,
        enable_log: &Path,
    ) -> std::path::PathBuf {
        let blender_path = root.join("fake-blender");
        let script = format!(
            r#"#!/bin/sh
ARGS="$*"
if printf '%s' "$ARGS" | grep -q "smolpc_blender_addon_dir_probe"; then
  echo '{{"status":"ok","addonDir":"{addon_dir}"}}'
  exit 0
fi
if printf '%s' "$ARGS" | grep -q "smolpc_blender_addon_enable"; then
  printf 'enabled\n' >> "{enable_log}"
  echo '{{"status":"ok","loaded":true,"module":"blender_helper_http"}}'
  exit 0
fi
exit 0
"#,
            addon_dir = addon_dir.display(),
            enable_log = enable_log.display(),
        );
        std::fs::write(&blender_path, script).expect("script");
        let mut permissions = std::fs::metadata(&blender_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&blender_path, permissions).expect("chmod");
        blender_path
    }

    #[tokio::test]
    async fn prepare_setup_creates_app_local_directories_and_python_runtime() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let python_root = resource_temp.path().join("python");
        let models_root = resource_temp.path().join("models");
        std::fs::create_dir_all(python_root.join("payload").join("bin")).expect("python payload");
        std::fs::create_dir_all(&models_root).expect("models root");
        std::fs::write(python_root.join("README.md"), "placeholder").expect("readme");
        std::fs::write(
            python_root.join("payload").join("bin").join("python"),
            "python",
        )
        .expect("runtime file");
        std::fs::write(
            python_root.join("manifest.json"),
            r#"{
              "version": "phase2",
              "source": "tests",
              "expectedPaths": ["README.md", "payload"],
              "status": "staged"
            }"#,
        )
        .expect("manifest");
        std::fs::write(
            models_root.join("manifest.json"),
            r#"{
              "version": "phase2",
              "source": "tests",
              "expectedPaths": ["qwen3-4b-instruct-2507"],
              "status": "placeholder"
            }"#,
        )
        .expect("manifest");

        let state = SetupState::with_host_detection(
            Some(resource_temp.path().to_path_buf()),
            Some(app_temp.path().to_path_buf()),
            false,
        );

        let status = prepare_setup(&state).await;
        assert!(app_temp.path().join("setup").join("python").exists());
        assert!(app_temp.path().join("setup").join("state").exists());
        assert!(app_temp.path().join("setup").join("logs").exists());
        assert!(app_temp
            .path()
            .join("setup")
            .join("python")
            .join("payload")
            .join("bin")
            .join("python")
            .exists());
        let python = status
            .items
            .iter()
            .find(|item| item.id == "bundled_python")
            .expect("python item");
        assert_eq!(python.state, SetupItemStateDto::Ready);
    }

    #[tokio::test]
    async fn prepare_setup_does_not_touch_external_user_profile_roots() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let external_temp = TempDir::new().expect("external temp");
        let gimp_profile = external_temp.path().join("gimp-profile");
        let blender_profile = external_temp.path().join("blender-profile");

        let python_root = resource_temp.path().join("python");
        let models_root = resource_temp.path().join("models");
        std::fs::create_dir_all(python_root.join("payload")).expect("python payload");
        std::fs::create_dir_all(&models_root).expect("models root");
        std::fs::write(python_root.join("README.md"), "placeholder").expect("readme");
        std::fs::write(python_root.join("payload").join("python"), "python").expect("runtime");
        std::fs::write(
            python_root.join("manifest.json"),
            r#"{
              "version": "phase2",
              "source": "tests",
              "expectedPaths": ["README.md", "payload"],
              "status": "staged"
            }"#,
        )
        .expect("manifest");
        std::fs::write(
            models_root.join("manifest.json"),
            r#"{
              "version": "phase2",
              "source": "tests",
              "expectedPaths": ["qwen3-4b-instruct-2507"],
              "status": "placeholder"
            }"#,
        )
        .expect("manifest");

        let state = SetupState::with_host_detection(
            Some(resource_temp.path().to_path_buf()),
            Some(app_temp.path().to_path_buf()),
            false,
        );
        let _ = prepare_setup(&state).await;

        assert!(!gimp_profile.exists());
        assert!(!blender_profile.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn prepare_setup_provisions_blender_addon_when_blender_is_detected() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let host_temp = TempDir::new().expect("host temp");

        let python_root = resource_temp.path().join("python");
        let models_root = resource_temp.path().join("models");
        std::fs::create_dir_all(python_root.join("payload").join("bin")).expect("python payload");
        std::fs::create_dir_all(&models_root).expect("models root");
        std::fs::write(python_root.join("README.md"), "placeholder").expect("readme");
        std::fs::write(
            python_root.join("payload").join("bin").join("python"),
            "python",
        )
        .expect("runtime file");
        std::fs::write(
            python_root.join("manifest.json"),
            r#"{
              "version": "phase2",
              "source": "tests",
              "expectedPaths": ["README.md", "payload"],
              "status": "staged"
            }"#,
        )
        .expect("manifest");
        std::fs::write(
            models_root.join("manifest.json"),
            r#"{
              "version": "phase2",
              "source": "tests",
              "expectedPaths": ["qwen3-4b-instruct-2507"],
              "status": "placeholder"
            }"#,
        )
        .expect("manifest");
        write_blender_resources(resource_temp.path());

        let addon_dir = host_temp.path().join("addons");
        let enable_log = host_temp.path().join("enable.log");
        let blender_path = write_fake_blender_script(host_temp.path(), &addon_dir, &enable_log);

        let state = SetupState::with_host_detection(
            Some(resource_temp.path().to_path_buf()),
            Some(app_temp.path().to_path_buf()),
            false,
        );
        {
            let mut cache = state.cache().await;
            cache
                .resolved_host_apps
                .insert(SETUP_ITEM_HOST_BLENDER.to_string(), blender_path);
        }

        let status = prepare_setup(&state).await;
        let blender_addon = status
            .items
            .iter()
            .find(|item| item.id == "blender_addon")
            .expect("blender addon item");

        assert_eq!(blender_addon.state, SetupItemStateDto::Ready);
        assert!(addon_dir.join("blender_helper_http.py").exists());
        assert!(enable_log.exists());
        assert!(app_temp
            .path()
            .join("setup")
            .join("state")
            .join("blender-addon.json")
            .exists());
    }

    #[test]
    fn prepare_setup_provisions_gimp_plugin_runtime_when_gimp_is_detected() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let host_temp = TempDir::new().expect("host temp");

        let python_root = resource_temp.path().join("python");
        let models_root = resource_temp.path().join("models");
        let gimp_root = resource_temp.path().join("gimp");
        std::fs::create_dir_all(python_root.join("payload").join("bin")).expect("python payload");
        std::fs::create_dir_all(&models_root).expect("models root");
        std::fs::create_dir_all(gimp_root.join("upstream")).expect("gimp upstream");
        std::fs::create_dir_all(gimp_root.join("bridge")).expect("gimp bridge");
        std::fs::create_dir_all(gimp_root.join("plugin").join("gimp-mcp-plugin"))
            .expect("gimp plugin");
        std::fs::write(python_root.join("README.md"), "placeholder").expect("readme");
        std::fs::write(
            python_root.join("payload").join("bin").join("python"),
            "python",
        )
        .expect("runtime file");
        std::fs::write(
            python_root.join("manifest.json"),
            r#"{
              "version": "phase2",
              "source": "tests",
              "expectedPaths": ["README.md", "payload"],
              "status": "staged"
            }"#,
        )
        .expect("manifest");
        std::fs::write(
            models_root.join("manifest.json"),
            r#"{
              "version": "phase2",
              "source": "tests",
              "expectedPaths": ["qwen3-4b-instruct-2507"],
              "status": "placeholder"
            }"#,
        )
        .expect("manifest");
        std::fs::write(gimp_root.join("README.md"), "gimp").expect("gimp readme");
        std::fs::write(gimp_root.join("LICENSE"), "GPL").expect("gimp license");
        std::fs::write(gimp_root.join("upstream").join("README.md"), "upstream")
            .expect("upstream readme");
        std::fs::write(
            gimp_root.join("upstream").join("GIMP_MCP_PROTOCOL.md"),
            "protocol",
        )
        .expect("protocol");
        std::fs::write(
            gimp_root.join("upstream").join("gimp_mcp_server.py"),
            "print('server')\n",
        )
        .expect("server");
        std::fs::write(
            gimp_root.join("upstream").join("pyproject.toml"),
            "[project]\nname='gimp-mcp'\n",
        )
        .expect("pyproject");
        std::fs::write(gimp_root.join("upstream").join("uv.lock"), "lock").expect("uv lock");
        std::fs::write(
            gimp_root
                .join("plugin")
                .join("gimp-mcp-plugin")
                .join("gimp-mcp-plugin.py"),
            "#!/usr/bin/env python3\nprint('plugin')\n",
        )
        .expect("plugin");
        std::fs::write(
            gimp_root
                .join("bridge")
                .join("smolpc_gimp_mcp_tcp_bridge.py"),
            "#!/usr/bin/env python3\nprint('bridge')\n",
        )
        .expect("bridge");
        std::fs::write(
            gimp_root.join("manifest.json"),
            r#"{
              "version": "phase5-test",
              "source": "tests",
              "expectedPaths": [
                "README.md",
                "LICENSE",
                "upstream/README.md",
                "upstream/GIMP_MCP_PROTOCOL.md",
                "upstream/gimp_mcp_server.py",
                "upstream/pyproject.toml",
                "upstream/uv.lock",
                "plugin/gimp-mcp-plugin/gimp-mcp-plugin.py",
                "bridge/smolpc_gimp_mcp_tcp_bridge.py"
              ],
              "status": "tracked"
            }"#,
        )
        .expect("manifest");

        let profile_root = app_temp.path().join("profile");
        std::fs::create_dir_all(&profile_root).expect("profile root");

        with_profile_env(&profile_root, || {
            let runtime = tokio::runtime::Runtime::new().expect("runtime");
            runtime.block_on(async {
                let state = SetupState::with_host_detection(
                    Some(resource_temp.path().to_path_buf()),
                    Some(app_temp.path().to_path_buf()),
                    false,
                );
                let gimp_path = host_temp.path().join("gimp-3.exe");
                std::fs::write(&gimp_path, "gimp").expect("fake gimp");
                {
                    let mut cache = state.cache().await;
                    cache
                        .resolved_host_apps
                        .insert(SETUP_ITEM_HOST_GIMP.to_string(), gimp_path);
                }

                let status = prepare_setup(&state).await;
                let gimp_item = status
                    .items
                    .iter()
                    .find(|item| item.id == SETUP_ITEM_GIMP_PLUGIN_RUNTIME)
                    .expect("gimp item");
                assert_eq!(gimp_item.state, SetupItemStateDto::Ready);
            });
        });
    }
}
