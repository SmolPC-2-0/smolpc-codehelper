use super::types::{DEFAULT_BUNDLED_MODEL_ID, SETUP_ITEM_BUNDLED_MODEL};
use smolpc_assistant_types::{SetupItemDto, SetupItemStateDto};
use smolpc_connector_common::manifests::{load_manifest, missing_expected_paths, resource_root};
use std::path::Path;

/// Check the shared models directory (%LOCALAPPDATA%\SmolPC\models\) for the default model.
fn shared_model_exists() -> bool {
    if let Some(base) = dirs::data_local_dir() {
        let shared = base
            .join("SmolPC")
            .join("models")
            .join(DEFAULT_BUNDLED_MODEL_ID);
        if shared.exists() {
            return true;
        }
    }
    false
}

pub fn bundled_model_item(resource_dir: Option<&Path>) -> SetupItemDto {
    let detail_prefix = "Default model";

    // First check the shared models directory (where the engine actually looks)
    if shared_model_exists() {
        let shared_path = dirs::data_local_dir()
            .unwrap()
            .join("SmolPC")
            .join("models")
            .join(DEFAULT_BUNDLED_MODEL_ID);
        return SetupItemDto {
            id: SETUP_ITEM_BUNDLED_MODEL.to_string(),
            label: "Bundled model".to_string(),
            state: SetupItemStateDto::Ready,
            detail: Some(format!(
                "{detail_prefix} found at {}",
                shared_path.display()
            )),
            required: true,
            can_prepare: false,
        };
    }

    // Fall back to checking the bundled resources path
    match resource_root(resource_dir, "models") {
        Ok(root) => match load_manifest(&root) {
            Ok(manifest) => {
                let missing = missing_expected_paths(&root, &manifest);
                let default_model_path = root.join(DEFAULT_BUNDLED_MODEL_ID);
                if !default_model_path.exists() {
                    return SetupItemDto {
                        id: SETUP_ITEM_BUNDLED_MODEL.to_string(),
                        label: "Bundled model".to_string(),
                        state: SetupItemStateDto::NotPrepared,
                        detail: Some(format!(
                            "{detail_prefix} is not staged yet. Missing {}",
                            default_model_path.display()
                        )),
                        required: true,
                        can_prepare: false,
                    };
                }

                if !missing.is_empty() {
                    return SetupItemDto {
                        id: SETUP_ITEM_BUNDLED_MODEL.to_string(),
                        label: "Bundled model".to_string(),
                        state: SetupItemStateDto::NotPrepared,
                        detail: Some(format!(
                            "{detail_prefix} manifest is present, but staged paths are missing: {}",
                            missing.join(", ")
                        )),
                        required: true,
                        can_prepare: false,
                    };
                }

                SetupItemDto {
                    id: SETUP_ITEM_BUNDLED_MODEL.to_string(),
                    label: "Bundled model".to_string(),
                    state: SetupItemStateDto::Ready,
                    detail: Some(format!(
                        "{detail_prefix} is staged at {}",
                        default_model_path.display()
                    )),
                    required: true,
                    can_prepare: false,
                }
            }
            Err(error) => SetupItemDto {
                id: SETUP_ITEM_BUNDLED_MODEL.to_string(),
                label: "Bundled model".to_string(),
                state: SetupItemStateDto::Missing,
                detail: Some(error),
                required: true,
                can_prepare: false,
            },
        },
        Err(error) => SetupItemDto {
            id: SETUP_ITEM_BUNDLED_MODEL.to_string(),
            label: "Bundled model".to_string(),
            state: SetupItemStateDto::Missing,
            detail: Some(error),
            required: true,
            can_prepare: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::bundled_model_item;
    use smolpc_assistant_types::SetupItemStateDto;
    use tempfile::TempDir;

    #[test]
    fn bundled_model_item_reports_missing_manifest() {
        let temp = TempDir::new().expect("temp dir");
        let item = bundled_model_item(Some(temp.path()));
        assert_eq!(item.state, SetupItemStateDto::Missing);
    }

    #[test]
    fn bundled_model_item_reports_ready_when_manifest_and_model_exist() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path().join("models");
        std::fs::create_dir_all(root.join("qwen3-4b-instruct-2507")).expect("create model dir");
        std::fs::write(root.join("README.md"), "placeholder").expect("write readme");
        std::fs::write(
            root.join("manifest.json"),
            r#"{
              "version": "phase2",
              "source": "tests",
              "expectedPaths": ["README.md", "qwen3-4b-instruct-2507"],
              "status": "staged"
            }"#,
        )
        .expect("write manifest");

        let item = bundled_model_item(Some(temp.path()));
        assert_eq!(item.state, SetupItemStateDto::Ready);
    }
}
