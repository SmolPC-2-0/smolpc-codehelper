use super::manifests::{load_manifest, missing_expected_paths, resource_root};
use super::types::SETUP_ITEM_BUNDLED_PYTHON;
use smolpc_assistant_types::{SetupItemDto, SetupItemStateDto};
use std::path::{Path, PathBuf};

const PREPARED_VERSION_FILE: &str = ".prepared-version";
const README_FILE: &str = "README.md";
const MANIFEST_FILE: &str = "manifest.json";

pub fn bundled_python_item(
    resource_dir: Option<&Path>,
    app_local_data_dir: Option<&Path>,
) -> SetupItemDto {
    match resource_root(resource_dir, "python") {
        Ok(root) => match load_manifest(&root) {
            Ok(manifest) => {
                let prepared_root = prepared_python_root(app_local_data_dir);
                let missing = missing_expected_paths(&root, &manifest);

                if prepared_version_matches(prepared_root.as_deref(), &manifest.version) {
                    return SetupItemDto {
                        id: SETUP_ITEM_BUNDLED_PYTHON.to_string(),
                        label: "Bundled Python".to_string(),
                        state: SetupItemStateDto::Ready,
                        detail: prepared_root
                            .map(|path| format!("Bundled Python is prepared at {}", path.display())),
                        required: true,
                        can_prepare: false,
                    };
                }

                if !missing.is_empty() {
                    return SetupItemDto {
                        id: SETUP_ITEM_BUNDLED_PYTHON.to_string(),
                        label: "Bundled Python".to_string(),
                        state: SetupItemStateDto::NotPrepared,
                        detail: Some(format!(
                            "Bundled Python payload is not staged yet. Missing {}",
                            missing.join(", ")
                        )),
                        required: true,
                        can_prepare: false,
                    };
                }

                SetupItemDto {
                    id: SETUP_ITEM_BUNDLED_PYTHON.to_string(),
                    label: "Bundled Python".to_string(),
                    state: SetupItemStateDto::NotPrepared,
                    detail: Some(
                        "Bundled Python payload is staged and can be prepared into app-local setup state"
                            .to_string(),
                    ),
                    required: true,
                    can_prepare: app_local_data_dir.is_some(),
                }
            }
            Err(error) => SetupItemDto {
                id: SETUP_ITEM_BUNDLED_PYTHON.to_string(),
                label: "Bundled Python".to_string(),
                state: SetupItemStateDto::Missing,
                detail: Some(error),
                required: true,
                can_prepare: false,
            },
        },
        Err(error) => SetupItemDto {
            id: SETUP_ITEM_BUNDLED_PYTHON.to_string(),
            label: "Bundled Python".to_string(),
            state: SetupItemStateDto::Missing,
            detail: Some(error),
            required: true,
            can_prepare: false,
        },
    }
}

pub fn prepare_bundled_python(
    resource_dir: Option<&Path>,
    app_local_data_dir: Option<&Path>,
) -> Result<(), String> {
    let root = resource_root(resource_dir, "python")?;
    let manifest = load_manifest(&root)?;
    let missing = missing_expected_paths(&root, &manifest);
    if !missing.is_empty() {
        return Err(format!(
            "Bundled Python payload is not staged yet. Missing {}",
            missing.join(", ")
        ));
    }

    let prepared_root = prepared_python_root(app_local_data_dir)
        .ok_or_else(|| "Tauri app-local-data directory is unavailable".to_string())?;
    std::fs::create_dir_all(&prepared_root).map_err(|error| {
        format!(
            "Failed to create bundled Python setup directory {}: {error}",
            prepared_root.display()
        )
    })?;

    copy_payload_entries(&root, &prepared_root)?;
    std::fs::write(prepared_root.join(PREPARED_VERSION_FILE), manifest.version).map_err(|error| {
        format!(
            "Failed to write prepared Python version marker in {}: {error}",
            prepared_root.display()
        )
    })?;

    Ok(())
}

pub fn prepared_python_root(app_local_data_dir: Option<&Path>) -> Option<PathBuf> {
    app_local_data_dir.map(|path| path.join("setup").join("python"))
}

fn prepared_version_matches(root: Option<&Path>, version: &str) -> bool {
    let Some(root) = root else {
        return false;
    };
    let marker = root.join(PREPARED_VERSION_FILE);
    std::fs::read_to_string(marker)
        .map(|value| value.trim() == version)
        .unwrap_or(false)
}

fn copy_payload_entries(source_root: &Path, target_root: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(source_root)
        .map_err(|error| format!("Failed to read Python payload root {}: {error}", source_root.display()))?
    {
        let entry = entry.map_err(|error| format!("Failed to inspect Python payload entry: {error}"))?;
        let path = entry.path();
        let name = entry.file_name();
        if name.to_str() == Some(MANIFEST_FILE) || name.to_str() == Some(README_FILE) {
            continue;
        }
        let target = target_root.join(name);
        copy_path_recursively(&path, &target)?;
    }
    Ok(())
}

fn copy_path_recursively(source: &Path, target: &Path) -> Result<(), String> {
    let metadata = std::fs::metadata(source)
        .map_err(|error| format!("Failed to stat {}: {error}", source.display()))?;
    if metadata.is_dir() {
        std::fs::create_dir_all(target)
            .map_err(|error| format!("Failed to create directory {}: {error}", target.display()))?;
        for entry in std::fs::read_dir(source)
            .map_err(|error| format!("Failed to read directory {}: {error}", source.display()))?
        {
            let entry =
                entry.map_err(|error| format!("Failed to inspect directory entry: {error}"))?;
            copy_path_recursively(&entry.path(), &target.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("Failed to create parent directory {}: {error}", parent.display())
            })?;
        }
        std::fs::copy(source, target).map_err(|error| {
            format!(
                "Failed to copy {} to {}: {error}",
                source.display(),
                target.display()
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{bundled_python_item, prepare_bundled_python};
    use smolpc_assistant_types::SetupItemStateDto;
    use tempfile::TempDir;

    #[test]
    fn bundled_python_item_reports_not_prepared_when_payload_missing() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path().join("python");
        std::fs::create_dir_all(&root).expect("python root");
        std::fs::write(root.join("README.md"), "placeholder").expect("readme");
        std::fs::write(
            root.join("manifest.json"),
            r#"{
              "version": "phase2",
              "source": "tests",
              "expectedPaths": ["README.md", "payload"],
              "status": "placeholder"
            }"#,
        )
        .expect("manifest");

        let item = bundled_python_item(Some(temp.path()), Some(temp.path()));
        assert_eq!(item.state, SetupItemStateDto::NotPrepared);
    }

    #[test]
    fn prepare_bundled_python_copies_staged_payload() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let root = resource_temp.path().join("python");
        std::fs::create_dir_all(root.join("payload").join("bin")).expect("payload");
        std::fs::write(root.join("README.md"), "placeholder").expect("readme");
        std::fs::write(root.join("payload").join("bin").join("python"), "python").expect("binary");
        std::fs::write(
            root.join("manifest.json"),
            r#"{
              "version": "phase2",
              "source": "tests",
              "expectedPaths": ["README.md", "payload"],
              "status": "staged"
            }"#,
        )
        .expect("manifest");

        prepare_bundled_python(Some(resource_temp.path()), Some(app_temp.path())).expect("prepare");

        let prepared_root = app_temp.path().join("setup").join("python");
        assert!(prepared_root.join("payload").join("bin").join("python").exists());
        assert!(prepared_root.join(".prepared-version").exists());
    }
}
