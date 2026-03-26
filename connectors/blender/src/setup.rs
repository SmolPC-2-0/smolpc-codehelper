use smolpc_connector_common::manifests::{load_manifest, missing_expected_paths, resource_root};
use smolpc_connector_common::SETUP_ITEM_BLENDER_ADDON;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use smolpc_assistant_types::{SetupItemDto, SetupItemStateDto};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::time::{SystemTime, UNIX_EPOCH};

pub const BLENDER_ADDON_MODULE_ID: &str = "blender_helper_http";
pub const BLENDER_ADDON_VERSION: &str = "7.0.0";

const BLENDER_RESOURCE_ROOT: &str = "blender";
const BLENDER_ADDON_RELATIVE_PATH: &str = "addon/blender_helper_http.py";
const BLENDER_ADDON_MARKER_FILE: &str = "blender-addon.json";
const BLENDER_ADDON_DIR_PROBE_KIND: &str = "smolpc_blender_addon_dir_probe";
const BLENDER_ADDON_ENABLE_KIND: &str = "smolpc_blender_addon_enable";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlenderAddonPrepareOutcome {
    AlreadyReady(BlenderAddonMarker),
    Prepared(BlenderAddonMarker),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlenderAddonMarker {
    pub addon_version: String,
    pub addon_module_id: String,
    pub provision_target_path: String,
    pub source_resource_path: String,
    pub timestamp: u64,
}

pub fn blender_addon_item(
    resource_dir: Option<&Path>,
    app_local_data_dir: Option<&Path>,
    blender_path: Option<&Path>,
) -> SetupItemDto {
    let label = "Blender addon".to_string();

    let Some(blender_path) = blender_path else {
        return SetupItemDto {
            id: SETUP_ITEM_BLENDER_ADDON.to_string(),
            label,
            state: SetupItemStateDto::Missing,
            detail: Some(
                "Blender is not installed or could not be detected yet, so the bundled addon cannot be provisioned."
                    .to_string(),
            ),
            required: true,
            can_prepare: false,
        };
    };

    let addon_source = match resolve_addon_source(resource_dir) {
        Ok(path) => path,
        Err(error) => {
            return SetupItemDto {
                id: SETUP_ITEM_BLENDER_ADDON.to_string(),
                label,
                state: SetupItemStateDto::Missing,
                detail: Some(error),
                required: true,
                can_prepare: false,
            };
        }
    };

    let Some(app_local_data_dir) = app_local_data_dir else {
        return SetupItemDto {
            id: SETUP_ITEM_BLENDER_ADDON.to_string(),
            label,
            state: SetupItemStateDto::Error,
            detail: Some(
                "Tauri app-local-data directory is unavailable, so the Blender addon provision marker cannot be stored."
                    .to_string(),
            ),
            required: true,
            can_prepare: false,
        };
    };

    match read_marker(Some(app_local_data_dir)) {
        Ok(Some(marker)) if marker_matches_current_source(&marker, &addon_source) => {
            let target_path = PathBuf::from(&marker.provision_target_path);
            if target_path.exists() {
                SetupItemDto {
                    id: SETUP_ITEM_BLENDER_ADDON.to_string(),
                    label,
                    state: SetupItemStateDto::Ready,
                    detail: Some(format!(
                        "Bundled Blender addon {BLENDER_ADDON_VERSION} is provisioned at {} for Blender at {}.",
                        target_path.display(),
                        blender_path.display()
                    )),
                    required: true,
                    can_prepare: false,
                }
            } else {
                SetupItemDto {
                    id: SETUP_ITEM_BLENDER_ADDON.to_string(),
                    label,
                    state: SetupItemStateDto::NotPrepared,
                    detail: Some(format!(
                        "Blender addon marker exists, but the provisioned addon file is missing at {}. Run Prepare to repair it.",
                        target_path.display()
                    )),
                    required: true,
                    can_prepare: true,
                }
            }
        }
        Ok(Some(_)) => SetupItemDto {
            id: SETUP_ITEM_BLENDER_ADDON.to_string(),
            label,
            state: SetupItemStateDto::NotPrepared,
            detail: Some(
                "Blender addon provisioning is out of date and needs to be repaired from the bundled resource snapshot."
                    .to_string(),
            ),
            required: true,
            can_prepare: true,
        },
        Ok(None) => SetupItemDto {
            id: SETUP_ITEM_BLENDER_ADDON.to_string(),
            label,
            state: SetupItemStateDto::NotPrepared,
            detail: Some(format!(
                "Blender detected at {}. The bundled addon snapshot is staged and can be provisioned automatically.",
                blender_path.display()
            )),
            required: true,
            can_prepare: true,
        },
        Err(error) => SetupItemDto {
            id: SETUP_ITEM_BLENDER_ADDON.to_string(),
            label,
            state: SetupItemStateDto::NotPrepared,
            detail: Some(format!(
                "Blender addon provision state is invalid and needs repair. {error}"
            )),
            required: true,
            can_prepare: true,
        },
    }
}

pub fn ensure_blender_addon_prepared(
    resource_dir: Option<&Path>,
    app_local_data_dir: Option<&Path>,
    blender_path: &Path,
) -> Result<BlenderAddonPrepareOutcome, String> {
    let addon_source = resolve_addon_source(resource_dir)?;
    if let Some(marker) = read_marker(app_local_data_dir)? {
        let target_path = PathBuf::from(&marker.provision_target_path);
        if marker_matches_current_source(&marker, &addon_source) && target_path.exists() {
            return Ok(BlenderAddonPrepareOutcome::AlreadyReady(marker));
        }
    }

    let addon_dir = probe_blender_addon_dir(blender_path)?;
    std::fs::create_dir_all(&addon_dir).map_err(|error| {
        format!(
            "Failed to create the Blender addon directory {}: {error}",
            addon_dir.display()
        )
    })?;

    let target_path = addon_dir.join(format!("{BLENDER_ADDON_MODULE_ID}.py"));
    std::fs::copy(&addon_source, &target_path).map_err(|error| {
        format!(
            "Failed to copy the bundled Blender addon from {} to {}: {error}",
            addon_source.display(),
            target_path.display()
        )
    })?;

    enable_blender_addon(blender_path)?;

    let marker = BlenderAddonMarker {
        addon_version: BLENDER_ADDON_VERSION.to_string(),
        addon_module_id: BLENDER_ADDON_MODULE_ID.to_string(),
        provision_target_path: target_path.to_string_lossy().to_string(),
        source_resource_path: addon_source.to_string_lossy().to_string(),
        timestamp: now_unix_seconds(),
    };
    write_marker(app_local_data_dir, &marker)?;

    Ok(BlenderAddonPrepareOutcome::Prepared(marker))
}

pub fn provision_marker_path(app_local_data_dir: Option<&Path>) -> Option<PathBuf> {
    app_local_data_dir.map(|base| {
        base.join("setup")
            .join("state")
            .join(BLENDER_ADDON_MARKER_FILE)
    })
}

fn resolve_addon_source(resource_dir: Option<&Path>) -> Result<PathBuf, String> {
    let root = resource_root(resource_dir, BLENDER_RESOURCE_ROOT)?;
    let manifest = load_manifest(&root)?;
    let missing = missing_expected_paths(&root, &manifest);
    if !missing.is_empty() {
        return Err(format!(
            "Bundled Blender resources are incomplete. Missing {}",
            missing.join(", ")
        ));
    }

    let addon_source = root.join(BLENDER_ADDON_RELATIVE_PATH);
    if addon_source.is_file() {
        Ok(addon_source)
    } else {
        Err(format!(
            "Bundled Blender addon snapshot is missing: {}",
            addon_source.display()
        ))
    }
}

fn read_marker(app_local_data_dir: Option<&Path>) -> Result<Option<BlenderAddonMarker>, String> {
    let Some(path) = provision_marker_path(app_local_data_dir) else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "Failed to read Blender addon marker {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_str(&raw).map(Some).map_err(|error| {
        format!(
            "Failed to parse Blender addon marker {}: {error}",
            path.display()
        )
    })
}

fn write_marker(
    app_local_data_dir: Option<&Path>,
    marker: &BlenderAddonMarker,
) -> Result<(), String> {
    let path = provision_marker_path(app_local_data_dir)
        .ok_or_else(|| "Tauri app-local-data directory is unavailable".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create Blender addon marker directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let raw = serde_json::to_string_pretty(marker)
        .map_err(|error| format!("Failed to serialize Blender addon marker: {error}"))?;
    std::fs::write(&path, raw).map_err(|error| {
        format!(
            "Failed to write Blender addon marker {}: {error}",
            path.display()
        )
    })
}

fn marker_matches_current_source(marker: &BlenderAddonMarker, addon_source: &Path) -> bool {
    marker.addon_version == BLENDER_ADDON_VERSION
        && marker.addon_module_id == BLENDER_ADDON_MODULE_ID
        && marker.source_resource_path == addon_source.to_string_lossy()
}

fn probe_blender_addon_dir(blender_path: &Path) -> Result<PathBuf, String> {
    let value = run_blender_background_json(
        blender_path,
        &build_addon_dir_probe_expr(),
        "probe the Blender addon directory",
    )?;
    let object = value
        .as_object()
        .ok_or_else(|| "Blender addon directory probe did not return a JSON object.".to_string())?;
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("error");
    if status != "ok" {
        let error = object
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Blender did not report an addon directory.");
        return Err(format!("Blender addon directory probe failed. {error}"));
    }
    let addon_dir = object
        .get("addonDir")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Blender did not report a usable addon directory path.".to_string())?;
    Ok(PathBuf::from(addon_dir))
}

fn enable_blender_addon(blender_path: &Path) -> Result<(), String> {
    let value = run_blender_background_json(
        blender_path,
        &build_enable_addon_expr(),
        "enable the Blender addon",
    )?;
    let object = value
        .as_object()
        .ok_or_else(|| "Blender addon enable command did not return a JSON object.".to_string())?;
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("error");
    if status != "ok" {
        let error = object
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Blender did not confirm addon enablement.");
        return Err(format!("Blender addon enable command failed. {error}"));
    }
    if object.get("loaded").and_then(Value::as_bool) != Some(true) {
        return Err(
            "Blender reported the addon enable command, but the addon is not loaded yet."
                .to_string(),
        );
    }
    Ok(())
}

fn run_blender_background_json(
    blender_path: &Path,
    python_expr: &str,
    action: &str,
) -> Result<Value, String> {
    let mut cmd = Command::new(blender_path);
    cmd.arg("--background")
        .arg("--python-expr")
        .arg(python_expr)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    let output = cmd
        .output()
        .map_err(|error| format!("Failed to start Blender to {action}: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let parsed = parse_last_json_line(&stdout).or_else(|| parse_last_json_line(&stderr));

    if !output.status.success() {
        let detail = parsed
            .as_ref()
            .and_then(json_error_detail)
            .or_else(|| non_empty_output_detail(&stderr))
            .or_else(|| non_empty_output_detail(&stdout))
            .unwrap_or_else(|| "Blender did not return a detailed error.".to_string());
        return Err(format!("Failed to {action}. {detail}"));
    }

    parsed.ok_or_else(|| {
        format!("Blender completed the {action} step but did not return the expected JSON payload.")
    })
}

fn parse_last_json_line(output: &str) -> Option<Value> {
    output.lines().rev().find_map(|line| {
        let line = line.trim();
        if line.starts_with('{') && line.ends_with('}') {
            serde_json::from_str::<Value>(line).ok()
        } else {
            None
        }
    })
}

fn json_error_detail(value: &Value) -> Option<String> {
    value
        .get("error")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn non_empty_output_detail(output: &str) -> Option<String> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn build_addon_dir_probe_expr() -> String {
    format!(
        r#"import bpy, json
try:
    addon_dir = bpy.utils.user_resource("SCRIPTS", path="addons", create=True)
    print(json.dumps({{"status": "ok", "kind": "{BLENDER_ADDON_DIR_PROBE_KIND}", "addonDir": addon_dir}}))
except Exception as exc:
    print(json.dumps({{"status": "error", "kind": "{BLENDER_ADDON_DIR_PROBE_KIND}", "error": str(exc)}}))
    raise"#
    )
}

fn build_enable_addon_expr() -> String {
    format!(
        r#"import addon_utils, bpy, json
module = "{BLENDER_ADDON_MODULE_ID}"
try:
    addon_utils.enable(module, default_set=True)
    default, loaded = addon_utils.check(module)
    if hasattr(bpy.ops.wm, "save_userpref"):
        bpy.ops.wm.save_userpref()
    if not loaded:
        raise RuntimeError(f"{{module}} is not loaded after enable")
    print(json.dumps({{"status": "ok", "kind": "{BLENDER_ADDON_ENABLE_KIND}", "module": module, "loaded": loaded, "default": default}}))
except Exception as exc:
    print(json.dumps({{"status": "error", "kind": "{BLENDER_ADDON_ENABLE_KIND}", "error": str(exc)}}))
    raise"#
    )
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{
        blender_addon_item, ensure_blender_addon_prepared, provision_marker_path,
        BlenderAddonPrepareOutcome, BLENDER_ADDON_VERSION,
    };
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
            "# blender addon snapshot\n",
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
        probe_should_fail: bool,
        enable_should_fail: bool,
    ) -> std::path::PathBuf {
        let blender_path = root.join("fake-blender");
        let script = format!(
            r#"#!/bin/sh
ARGS="$*"
if printf '%s' "$ARGS" | grep -q "smolpc_blender_addon_dir_probe"; then
  if [ "{probe_should_fail}" = "true" ]; then
    echo "probe failed" >&2
    exit 2
  fi
  echo '{{"status":"ok","addonDir":"{addon_dir}"}}'
  exit 0
fi
if printf '%s' "$ARGS" | grep -q "smolpc_blender_addon_enable"; then
  if [ "{enable_should_fail}" = "true" ]; then
    echo '{{"status":"error","error":"enable failed"}}'
    exit 2
  fi
  printf 'enabled\n' >> "{enable_log}"
  echo '{{"status":"ok","loaded":true,"module":"blender_helper_http"}}'
  exit 0
fi
printf 'launched\n' >> "{enable_log}.launch"
exit 0
"#,
            addon_dir = addon_dir.display(),
            enable_log = enable_log.display(),
        );
        std::fs::write(&blender_path, script).expect("write blender script");
        let mut permissions = std::fs::metadata(&blender_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&blender_path, permissions).expect("chmod");
        blender_path
    }

    #[test]
    fn blender_addon_item_reports_not_prepared_when_blender_is_installed() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        write_blender_resources(resource_temp.path());

        let item = blender_addon_item(
            Some(resource_temp.path()),
            Some(app_temp.path()),
            Some(Path::new("/fake/blender")),
        );

        assert_eq!(item.state, SetupItemStateDto::NotPrepared);
        assert!(item.can_prepare);
    }

    #[cfg(unix)]
    #[test]
    fn ensure_blender_addon_prepared_copies_addon_and_writes_marker() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let host_temp = TempDir::new().expect("host temp");
        write_blender_resources(resource_temp.path());

        let addon_dir = host_temp.path().join("addons");
        let enable_log = host_temp.path().join("enable.log");
        let blender_path =
            write_fake_blender_script(host_temp.path(), &addon_dir, &enable_log, false, false);

        let outcome = ensure_blender_addon_prepared(
            Some(resource_temp.path()),
            Some(app_temp.path()),
            &blender_path,
        )
        .expect("prepare blender addon");

        assert!(matches!(outcome, BlenderAddonPrepareOutcome::Prepared(_)));
        assert!(addon_dir.join("blender_helper_http.py").exists());
        assert!(enable_log.exists());
        assert!(provision_marker_path(Some(app_temp.path()))
            .expect("marker path")
            .exists());
    }

    #[cfg(unix)]
    #[test]
    fn probe_failure_returns_honest_error() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let host_temp = TempDir::new().expect("host temp");
        write_blender_resources(resource_temp.path());

        let addon_dir = host_temp.path().join("addons");
        let enable_log = host_temp.path().join("enable.log");
        let blender_path =
            write_fake_blender_script(host_temp.path(), &addon_dir, &enable_log, true, false);

        let error = ensure_blender_addon_prepared(
            Some(resource_temp.path()),
            Some(app_temp.path()),
            &blender_path,
        )
        .expect_err("probe should fail");

        assert!(error.contains("probe the Blender addon directory"));
    }

    #[cfg(unix)]
    #[test]
    fn enable_failure_returns_honest_error() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let host_temp = TempDir::new().expect("host temp");
        write_blender_resources(resource_temp.path());

        let addon_dir = host_temp.path().join("addons");
        let enable_log = host_temp.path().join("enable.log");
        let blender_path =
            write_fake_blender_script(host_temp.path(), &addon_dir, &enable_log, false, true);

        let error = ensure_blender_addon_prepared(
            Some(resource_temp.path()),
            Some(app_temp.path()),
            &blender_path,
        )
        .expect_err("enable should fail");

        assert!(error.contains("enable the Blender addon"));
    }

    #[cfg(unix)]
    #[test]
    fn existing_marker_short_circuits_reprovisioning() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        let host_temp = TempDir::new().expect("host temp");
        write_blender_resources(resource_temp.path());

        let addon_dir = host_temp.path().join("addons");
        std::fs::create_dir_all(&addon_dir).expect("addon dir");
        let target = addon_dir.join("blender_helper_http.py");
        std::fs::write(&target, "# addon").expect("target");
        let marker_path = provision_marker_path(Some(app_temp.path())).expect("marker path");
        if let Some(parent) = marker_path.parent() {
            std::fs::create_dir_all(parent).expect("marker parent");
        }
        std::fs::write(
            &marker_path,
            format!(
                r#"{{
                  "addonVersion": "{BLENDER_ADDON_VERSION}",
                  "addonModuleId": "blender_helper_http",
                  "provisionTargetPath": "{}",
                  "sourceResourcePath": "{}",
                  "timestamp": 1
                }}"#,
                target.display(),
                resource_temp
                    .path()
                    .join("blender")
                    .join("addon")
                    .join("blender_helper_http.py")
                    .display()
            ),
        )
        .expect("marker");

        let addon_dir_for_probe = host_temp.path().join("unused");
        let enable_log = host_temp.path().join("enable.log");
        let blender_path = write_fake_blender_script(
            host_temp.path(),
            &addon_dir_for_probe,
            &enable_log,
            false,
            false,
        );

        let outcome = ensure_blender_addon_prepared(
            Some(resource_temp.path()),
            Some(app_temp.path()),
            &blender_path,
        )
        .expect("prepare");

        assert!(matches!(
            outcome,
            BlenderAddonPrepareOutcome::AlreadyReady(_)
        ));
        assert!(!enable_log.exists());
    }
}
