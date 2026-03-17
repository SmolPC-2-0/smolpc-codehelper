use super::blender::ensure_blender_addon_prepared;
use super::host_apps::detect_blender_with_policy;
use super::python::prepare_bundled_python;
use super::state::SetupState;
use super::status::collect_setup_status;
use super::types::SETUP_ITEM_HOST_BLENDER;
use std::path::Path;
use std::path::PathBuf;

pub async fn prepare_setup(state: &SetupState) -> SetupResult {
    let cached_blender_path = {
        let cache = state.cache().await;
        cache
            .resolved_host_apps
            .get(SETUP_ITEM_HOST_BLENDER)
            .cloned()
    };
    let result = prepare_setup_inner(state, cached_blender_path.as_ref());
    {
        let mut cache = state.cache().await;
        cache.last_error = result.err();
    }
    collect_setup_status(state).await
}

type SetupResult = smolpc_assistant_types::SetupStatusDto;

fn prepare_setup_inner(
    state: &SetupState,
    cached_blender_path: Option<&PathBuf>,
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
    use crate::setup::types::SETUP_ITEM_HOST_BLENDER;
    use smolpc_assistant_types::SetupItemStateDto;
    use std::path::Path;
    use tempfile::TempDir;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

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
}
