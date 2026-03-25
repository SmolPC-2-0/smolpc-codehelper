use super::blender::blender_addon_item;
use super::gimp::gimp_plugin_runtime_item;
use smolpc_connector_common::host_apps::{detect_all_with_policy, HostAppDetection};
use smolpc_connector_common::launch::setup_launch_detail;
use super::models::bundled_model_item;
use smolpc_connector_common::python::bundled_python_item;
use super::state::SetupState;
use super::types::SETUP_ITEM_ENGINE_RUNTIME;
use smolpc_connector_common::{
    SETUP_ITEM_HOST_BLENDER, SETUP_ITEM_HOST_GIMP, SETUP_ITEM_HOST_LIBREOFFICE,
};
use smolpc_assistant_types::{
    SetupItemDto, SetupItemStateDto, SetupOverallStateDto, SetupStatusDto,
};

pub async fn collect_setup_status(state: &SetupState) -> SetupStatusDto {
    state.load_cache_from_disk_if_needed().await;

    let (detections, cache_changed) = {
        let mut cache = state.cache().await;
        let detections = detect_all_with_policy(
            &cache.resolved_host_apps,
            state.allow_system_host_detection(),
        );
        let next_resolved = detections
            .iter()
            .filter_map(|detection| {
                detection
                    .path
                    .as_ref()
                    .map(|path| (detection.id.to_string(), path.clone()))
            })
            .collect();

        let cache_changed = cache.resolved_host_apps != next_resolved;
        if cache_changed {
            cache.resolved_host_apps = next_resolved;
        }

        (detections, cache_changed)
    };

    if cache_changed {
        state.persist_cache_to_disk().await;
    }

    let mut items = Vec::new();
    items.push(engine_runtime_item(state));
    items.push(bundled_model_item(state.resource_dir()));
    items.push(bundled_python_item(
        state.resource_dir(),
        state.app_local_data_dir(),
    ));
    let blender_detection = detections
        .iter()
        .find(|detection| detection.id == SETUP_ITEM_HOST_BLENDER)
        .cloned();
    let gimp_detection = detections
        .iter()
        .find(|detection| detection.id == SETUP_ITEM_HOST_GIMP)
        .cloned();
    for detection in detections {
        let is_blender = detection.id == SETUP_ITEM_HOST_BLENDER;
        let is_gimp = detection.id == SETUP_ITEM_HOST_GIMP;
        items.push(host_detection_item(detection));
        if is_blender {
            items.push(blender_addon_item(
                state.resource_dir(),
                state.app_local_data_dir(),
                blender_detection
                    .as_ref()
                    .and_then(|value| value.path.as_deref()),
            ));
        }
        if is_gimp {
            items.push(gimp_plugin_runtime_item(
                state.resource_dir(),
                state.app_local_data_dir(),
                gimp_detection
                    .as_ref()
                    .and_then(|value| value.path.as_deref()),
            ));
        }
    }

    let last_error = {
        let cache = state.cache().await;
        cache.last_error.clone()
    };

    SetupStatusDto {
        overall_state: overall_state_for_items(&items, last_error.is_some()),
        items,
        last_error,
    }
}

fn engine_runtime_item(state: &SetupState) -> SetupItemDto {
    match state.setup_root() {
        Some(setup_root) => SetupItemDto {
            id: SETUP_ITEM_ENGINE_RUNTIME.to_string(),
            label: "Engine runtime".to_string(),
            state: SetupItemStateDto::Ready,
            detail: Some(format!(
                "Shared engine runtime contract resolves through {}. {}",
                setup_root.display(),
                setup_launch_detail()
            )),
            required: true,
            can_prepare: false,
        },
        None => SetupItemDto {
            id: SETUP_ITEM_ENGINE_RUNTIME.to_string(),
            label: "Engine runtime".to_string(),
            state: SetupItemStateDto::Error,
            detail: Some(
                "Tauri app-local-data directory is unavailable, so setup roots cannot be created"
                    .to_string(),
            ),
            required: true,
            can_prepare: false,
        },
    }
}

fn host_detection_item(detection: HostAppDetection) -> SetupItemDto {
    let is_detected = detection.path.is_some();
    let detail = match detection.path.as_ref() {
        Some(path) => Some(format!("{} detected at {}", detection.label, path.display())),
        None => Some(format!(
            "{} is not installed or could not be detected yet. Setup reports host-app presence and provider-owned repair state; interactive launch is user-controlled via Open App.",
            detection.label
        )),
    };

    let required = matches!(
        detection.id,
        SETUP_ITEM_HOST_GIMP | SETUP_ITEM_HOST_BLENDER | SETUP_ITEM_HOST_LIBREOFFICE
    );

    SetupItemDto {
        id: detection.id.to_string(),
        label: detection.label.to_string(),
        state: if is_detected {
            SetupItemStateDto::Ready
        } else {
            SetupItemStateDto::Missing
        },
        detail,
        required,
        can_prepare: false,
    }
}

fn overall_state_for_items(items: &[SetupItemDto], has_last_error: bool) -> SetupOverallStateDto {
    if has_last_error
        || items
            .iter()
            .any(|item| item.required && item.state == SetupItemStateDto::Error)
    {
        return SetupOverallStateDto::Error;
    }

    if items
        .iter()
        .any(|item| item.required && item.state != SetupItemStateDto::Ready)
    {
        SetupOverallStateDto::NeedsAttention
    } else {
        SetupOverallStateDto::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::collect_setup_status;
    use crate::setup::state::SetupState;
    use smolpc_connector_common::{SETUP_ITEM_BLENDER_ADDON, SETUP_ITEM_GIMP_PLUGIN_RUNTIME};
    use smolpc_assistant_types::{SetupItemStateDto, SetupOverallStateDto};
    use tempfile::TempDir;

    #[tokio::test]
    async fn collect_setup_status_returns_all_required_items() {
        let resource_temp = TempDir::new().expect("resource temp");
        std::fs::create_dir_all(resource_temp.path().join("python")).expect("python root");
        std::fs::create_dir_all(resource_temp.path().join("models")).expect("models root");

        let app_temp = TempDir::new().expect("app temp");
        let state = SetupState::with_host_detection(
            Some(resource_temp.path().to_path_buf()),
            Some(app_temp.path().to_path_buf()),
            false,
        );

        let status = collect_setup_status(&state).await;
        assert_eq!(status.items.len(), 8);
        assert!(status.items.iter().any(|item| item.id == "engine_runtime"));
        assert!(status.items.iter().any(|item| item.id == "bundled_model"));
        assert!(status.items.iter().any(|item| item.id == "bundled_python"));
        assert!(status
            .items
            .iter()
            .any(|item| item.id == SETUP_ITEM_BLENDER_ADDON));
        assert!(status
            .items
            .iter()
            .any(|item| item.id == SETUP_ITEM_GIMP_PLUGIN_RUNTIME));
    }

    #[tokio::test]
    async fn missing_python_manifest_is_reported_honestly() {
        let resource_temp = TempDir::new().expect("resource temp");
        std::fs::create_dir_all(resource_temp.path().join("python")).expect("python root");
        std::fs::create_dir_all(resource_temp.path().join("models")).expect("models root");
        std::fs::write(
            resource_temp.path().join("models").join("manifest.json"),
            r#"{
              "version": "phase2",
              "source": "tests",
              "expectedPaths": ["qwen3-4b-instruct-2507"],
              "status": "placeholder"
            }"#,
        )
        .expect("manifest");

        let app_temp = TempDir::new().expect("app temp");
        let state = SetupState::with_host_detection(
            Some(resource_temp.path().to_path_buf()),
            Some(app_temp.path().to_path_buf()),
            false,
        );

        let status = collect_setup_status(&state).await;
        let python = status
            .items
            .iter()
            .find(|item| item.id == "bundled_python")
            .expect("python item");
        assert_eq!(python.state, SetupItemStateDto::Missing);
        assert_eq!(status.overall_state, SetupOverallStateDto::NeedsAttention);
    }
}
