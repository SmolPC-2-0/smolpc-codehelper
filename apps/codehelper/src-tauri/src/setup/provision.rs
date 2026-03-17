use super::python::prepare_bundled_python;
use super::state::SetupState;
use super::status::collect_setup_status;
use std::path::Path;

pub async fn prepare_setup(state: &SetupState) -> SetupResult {
    let result = prepare_setup_inner(state);
    {
        let mut cache = state.cache().await;
        cache.last_error = result.err();
    }
    collect_setup_status(state).await
}

type SetupResult = smolpc_assistant_types::SetupStatusDto;

fn prepare_setup_inner(state: &SetupState) -> Result<(), String> {
    let setup_root = state
        .setup_root()
        .ok_or_else(|| "Tauri app-local-data directory is unavailable".to_string())?;
    prepare_setup_directories(&setup_root)?;
    prepare_bundled_python(state.resource_dir(), state.app_local_data_dir())?;
    Ok(())
}

fn prepare_setup_directories(setup_root: &Path) -> Result<(), String> {
    for relative in ["python", "state", "logs"] {
        let path = setup_root.join(relative);
        std::fs::create_dir_all(&path)
            .map_err(|error| format!("Failed to create setup directory {}: {error}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::prepare_setup;
    use crate::setup::state::SetupState;
    use smolpc_assistant_types::SetupItemStateDto;
    use tempfile::TempDir;

    #[tokio::test]
    async fn prepare_setup_creates_app_local_directories_and_python_runtime() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let python_root = resource_temp.path().join("python");
        let models_root = resource_temp.path().join("models");
        std::fs::create_dir_all(python_root.join("payload").join("bin")).expect("python payload");
        std::fs::create_dir_all(&models_root).expect("models root");
        std::fs::write(python_root.join("README.md"), "placeholder").expect("readme");
        std::fs::write(python_root.join("payload").join("bin").join("python"), "python")
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

        let state = SetupState::new(
            Some(resource_temp.path().to_path_buf()),
            Some(app_temp.path().to_path_buf()),
        );

        let status = prepare_setup(&state).await;
        assert!(app_temp.path().join("setup").join("python").exists());
        assert!(app_temp.path().join("setup").join("state").exists());
        assert!(app_temp.path().join("setup").join("logs").exists());
        assert!(
            app_temp
                .path()
                .join("setup")
                .join("python")
                .join("payload")
                .join("bin")
                .join("python")
                .exists()
        );
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

        let state = SetupState::new(
            Some(resource_temp.path().to_path_buf()),
            Some(app_temp.path().to_path_buf()),
        );
        let _ = prepare_setup(&state).await;

        assert!(!gimp_profile.exists());
        assert!(!blender_profile.exists());
    }
}
