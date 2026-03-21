use super::manifests::{load_manifest, missing_expected_paths, resource_root};
use super::types::SETUP_ITEM_GIMP_PLUGIN_RUNTIME;
use serde::{Deserialize, Serialize};
use smolpc_assistant_types::{SetupItemDto, SetupItemStateDto};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const GIMP_PLUGIN_SOCKET_HOST: &str = "127.0.0.1";
pub const GIMP_PLUGIN_SOCKET_PORT: u16 = 9877;

const GIMP_RESOURCE_ROOT: &str = "gimp";
const GIMP_PLUGIN_DIR_RELATIVE_PATH: &str = "plugin/gimp-mcp-plugin";
const GIMP_PLUGIN_ENTRY_RELATIVE_PATH: &str = "plugin/gimp-mcp-plugin/gimp-mcp-plugin.py";
const GIMP_BRIDGE_ENTRY_RELATIVE_PATH: &str = "bridge/smolpc_gimp_mcp_tcp_bridge.py";
const GIMP_MARKER_FILE: &str = "gimp-plugin-runtime.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GimpPluginRuntimePrepareOutcome {
    AlreadyReady(GimpPluginRuntimeMarker),
    Prepared(GimpPluginRuntimeMarker),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GimpResourceLayout {
    pub root: PathBuf,
    pub manifest_version: String,
    pub plugin_dir: PathBuf,
    pub plugin_entry: PathBuf,
    pub bridge_entry: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GimpPluginRuntimeMarker {
    pub manifest_version: String,
    pub provision_target_dir: String,
    pub provision_target_entry: String,
    pub source_plugin_dir: String,
    pub bridge_entrypoint: String,
    pub timestamp: u64,
}

pub fn gimp_plugin_runtime_item(
    resource_dir: Option<&Path>,
    app_local_data_dir: Option<&Path>,
    gimp_path: Option<&Path>,
) -> SetupItemDto {
    let label = "GIMP plugin/runtime".to_string();

    let Some(gimp_path) = gimp_path else {
        return SetupItemDto {
            id: SETUP_ITEM_GIMP_PLUGIN_RUNTIME.to_string(),
            label,
            state: SetupItemStateDto::Missing,
            detail: Some(
                "GIMP is not installed or could not be detected yet, so the bundled plugin/runtime cannot be provisioned."
                    .to_string(),
            ),
            required: true,
            can_prepare: false,
        };
    };

    if let Err(detail) = validate_supported_gimp(gimp_path) {
        return SetupItemDto {
            id: SETUP_ITEM_GIMP_PLUGIN_RUNTIME.to_string(),
            label,
            state: SetupItemStateDto::Missing,
            detail: Some(detail),
            required: true,
            can_prepare: false,
        };
    }

    let layout = match resolve_gimp_resource_layout(resource_dir) {
        Ok(layout) => layout,
        Err(error) => {
            return SetupItemDto {
                id: SETUP_ITEM_GIMP_PLUGIN_RUNTIME.to_string(),
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
            id: SETUP_ITEM_GIMP_PLUGIN_RUNTIME.to_string(),
            label,
            state: SetupItemStateDto::Error,
            detail: Some(
                "Tauri app-local-data directory is unavailable, so the GIMP provision marker cannot be stored."
                    .to_string(),
            ),
            required: true,
            can_prepare: false,
        };
    };

    match read_marker(Some(app_local_data_dir)) {
        Ok(Some(marker)) if marker_matches_current_layout(&marker, &layout) => {
            let target_entry = PathBuf::from(&marker.provision_target_entry);
            if target_entry.is_file() {
                SetupItemDto {
                    id: SETUP_ITEM_GIMP_PLUGIN_RUNTIME.to_string(),
                    label,
                    state: SetupItemStateDto::Ready,
                    detail: Some(format!(
                        "Bundled GIMP plugin/runtime is provisioned at {} for GIMP at {}. Runtime bridge stays on 127.0.0.1:10008.",
                        target_entry.display(),
                        gimp_path.display()
                    )),
                    required: true,
                    can_prepare: false,
                }
            } else {
                SetupItemDto {
                    id: SETUP_ITEM_GIMP_PLUGIN_RUNTIME.to_string(),
                    label,
                    state: SetupItemStateDto::NotPrepared,
                    detail: Some(format!(
                        "GIMP provision marker exists, but the provisioned plugin entry is missing at {}. Run Prepare to repair it.",
                        target_entry.display()
                    )),
                    required: true,
                    can_prepare: true,
                }
            }
        }
        Ok(Some(_)) => SetupItemDto {
            id: SETUP_ITEM_GIMP_PLUGIN_RUNTIME.to_string(),
            label,
            state: SetupItemStateDto::NotPrepared,
            detail: Some(
                "Bundled GIMP plugin/runtime provisioning is out of date and needs repair from the bundled resource snapshot."
                    .to_string(),
            ),
            required: true,
            can_prepare: true,
        },
        Ok(None) => SetupItemDto {
            id: SETUP_ITEM_GIMP_PLUGIN_RUNTIME.to_string(),
            label,
            state: SetupItemStateDto::NotPrepared,
            detail: Some(format!(
                "GIMP detected at {}. The bundled plugin/runtime snapshot is staged and can be provisioned automatically.",
                gimp_path.display()
            )),
            required: true,
            can_prepare: true,
        },
        Err(error) => SetupItemDto {
            id: SETUP_ITEM_GIMP_PLUGIN_RUNTIME.to_string(),
            label,
            state: SetupItemStateDto::NotPrepared,
            detail: Some(format!(
                "GIMP plugin/runtime provision state is invalid and needs repair. {error}"
            )),
            required: true,
            can_prepare: true,
        },
    }
}

pub fn ensure_gimp_plugin_runtime_prepared(
    resource_dir: Option<&Path>,
    app_local_data_dir: Option<&Path>,
    gimp_path: &Path,
) -> Result<GimpPluginRuntimePrepareOutcome, String> {
    validate_supported_gimp(gimp_path)?;
    let layout = resolve_gimp_resource_layout(resource_dir)?;
    let target_dir = resolve_gimp_plugin_target_dir(gimp_path)?;
    let target_entry = target_dir.join("gimp-mcp-plugin.py");

    if let Some(marker) = read_marker(app_local_data_dir)? {
        if marker_matches_current_layout(&marker, &layout) && target_entry.is_file() {
            return Ok(GimpPluginRuntimePrepareOutcome::AlreadyReady(marker));
        }
    }

    if target_dir.exists() {
        std::fs::remove_dir_all(&target_dir).map_err(|error| {
            format!(
                "Failed to clear the existing GIMP plugin directory {}: {error}",
                target_dir.display()
            )
        })?;
    }
    std::fs::create_dir_all(&target_dir).map_err(|error| {
        format!(
            "Failed to create the GIMP plugin directory {}: {error}",
            target_dir.display()
        )
    })?;
    copy_path_recursively(&layout.plugin_dir, &target_dir)?;
    ensure_unix_executable(&target_entry)?;

    let marker = GimpPluginRuntimeMarker {
        manifest_version: layout.manifest_version.clone(),
        provision_target_dir: target_dir.to_string_lossy().to_string(),
        provision_target_entry: target_entry.to_string_lossy().to_string(),
        source_plugin_dir: layout.plugin_dir.to_string_lossy().to_string(),
        bridge_entrypoint: layout.bridge_entry.to_string_lossy().to_string(),
        timestamp: now_unix_seconds(),
    };
    write_marker(app_local_data_dir, &marker)?;

    Ok(GimpPluginRuntimePrepareOutcome::Prepared(marker))
}

pub fn resolve_gimp_resource_layout(
    resource_dir: Option<&Path>,
) -> Result<GimpResourceLayout, String> {
    let root = resource_root(resource_dir, GIMP_RESOURCE_ROOT)?;
    let manifest = load_manifest(&root)?;
    let missing = missing_expected_paths(&root, &manifest);
    if !missing.is_empty() {
        return Err(format!(
            "Bundled GIMP resources are incomplete. Missing {}",
            missing.join(", ")
        ));
    }

    let plugin_dir = root.join(GIMP_PLUGIN_DIR_RELATIVE_PATH);
    let plugin_entry = root.join(GIMP_PLUGIN_ENTRY_RELATIVE_PATH);
    let bridge_entry = root.join(GIMP_BRIDGE_ENTRY_RELATIVE_PATH);

    if !plugin_dir.is_dir() {
        return Err(format!(
            "Bundled GIMP plugin directory is missing: {}",
            plugin_dir.display()
        ));
    }
    if !plugin_entry.is_file() {
        return Err(format!(
            "Bundled GIMP plugin entrypoint is missing: {}",
            plugin_entry.display()
        ));
    }
    if !bridge_entry.is_file() {
        return Err(format!(
            "Bundled GIMP TCP bridge is missing: {}",
            bridge_entry.display()
        ));
    }

    Ok(GimpResourceLayout {
        root,
        manifest_version: manifest.version,
        plugin_dir,
        plugin_entry,
        bridge_entry,
    })
}

pub fn provision_marker_path(app_local_data_dir: Option<&Path>) -> Option<PathBuf> {
    app_local_data_dir.map(|base| base.join("setup").join("state").join(GIMP_MARKER_FILE))
}

pub fn validate_supported_gimp(gimp_path: &Path) -> Result<(), String> {
    let lower = gimp_path.to_string_lossy().to_lowercase();
    if lower.contains("gimp-2.10") || lower.contains("gimp 2") {
        return Err(format!(
            "Detected GIMP at {} appears to be GIMP 2.x. Phase 5 targets GIMP 3.x only.",
            gimp_path.display()
        ));
    }
    Ok(())
}

fn marker_matches_current_layout(
    marker: &GimpPluginRuntimeMarker,
    layout: &GimpResourceLayout,
) -> bool {
    marker.manifest_version == layout.manifest_version
        && marker.source_plugin_dir == layout.plugin_dir.to_string_lossy()
        && marker.bridge_entrypoint == layout.bridge_entry.to_string_lossy()
}

fn read_marker(
    app_local_data_dir: Option<&Path>,
) -> Result<Option<GimpPluginRuntimeMarker>, String> {
    let Some(path) = provision_marker_path(app_local_data_dir) else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "Failed to read GIMP plugin/runtime marker {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_str(&raw).map(Some).map_err(|error| {
        format!(
            "Failed to parse GIMP plugin/runtime marker {}: {error}",
            path.display()
        )
    })
}

fn write_marker(
    app_local_data_dir: Option<&Path>,
    marker: &GimpPluginRuntimeMarker,
) -> Result<(), String> {
    let path = provision_marker_path(app_local_data_dir)
        .ok_or_else(|| "Tauri app-local-data directory is unavailable".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create the GIMP marker directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let raw = serde_json::to_string_pretty(marker)
        .map_err(|error| format!("Failed to serialize GIMP marker: {error}"))?;
    std::fs::write(&path, raw).map_err(|error| {
        format!(
            "Failed to write the GIMP plugin/runtime marker {}: {error}",
            path.display()
        )
    })
}

fn resolve_gimp_plugin_target_dir(_gimp_path: &Path) -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let appdata = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| {
                "APPDATA is unavailable, so the GIMP profile root cannot be resolved.".to_string()
            })?;
        return Ok(appdata
            .join("GIMP")
            .join("3.0")
            .join("plug-ins")
            .join("gimp-mcp-plugin"));
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
            "HOME is unavailable, so the GIMP profile root cannot be resolved.".to_string()
        })?;
        return Ok(home
            .join("Library")
            .join("Application Support")
            .join("GIMP")
            .join("3.0")
            .join("plug-ins")
            .join("gimp-mcp-plugin"));
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
            "HOME is unavailable, so the GIMP profile root cannot be resolved.".to_string()
        })?;
        let lower = _gimp_path.to_string_lossy().to_lowercase();
        if lower.contains("/snap/") {
            return Ok(home
                .join("snap")
                .join("gimp")
                .join("current")
                .join(".config")
                .join("GIMP")
                .join("3.0")
                .join("plug-ins")
                .join("gimp-mcp-plugin"));
        }

        Ok(home
            .join(".config")
            .join("GIMP")
            .join("3.0")
            .join("plug-ins")
            .join("gimp-mcp-plugin"))
    }
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
                format!(
                    "Failed to create parent directory {}: {error}",
                    parent.display()
                )
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

fn ensure_unix_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = std::fs::metadata(path)
            .map_err(|error| format!("Failed to stat {}: {error}", path.display()))?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).map_err(|error| {
            format!(
                "Failed to mark the GIMP plugin entrypoint executable at {}: {error}",
                path.display()
            )
        })?;
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_gimp_plugin_runtime_prepared, gimp_plugin_runtime_item, provision_marker_path,
        validate_supported_gimp, GimpPluginRuntimePrepareOutcome,
    };
    use smolpc_assistant_types::SetupItemStateDto;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex as StdMutex, OnceLock};
    use tempfile::TempDir;

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

    fn write_gimp_resources(root: &Path) {
        let gimp_root = root.join("gimp");
        std::fs::create_dir_all(gimp_root.join("upstream")).expect("upstream dir");
        std::fs::create_dir_all(gimp_root.join("bridge")).expect("bridge dir");
        std::fs::create_dir_all(gimp_root.join("plugin").join("gimp-mcp-plugin"))
            .expect("plugin dir");
        std::fs::write(gimp_root.join("README.md"), "gimp resources").expect("readme");
        std::fs::write(gimp_root.join("LICENSE"), "GPL-3.0").expect("license");
        std::fs::write(
            gimp_root.join("upstream").join("README.md"),
            "upstream readme",
        )
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
    }

    #[test]
    fn validate_supported_gimp_rejects_gimp_2_paths() {
        let error = validate_supported_gimp(Path::new("/Applications/GIMP 2/bin/gimp-2.10"))
            .expect_err("gimp 2 should be rejected");
        assert!(error.contains("GIMP 3.x"));
    }

    #[test]
    fn gimp_plugin_runtime_item_reports_not_prepared_before_copy() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        write_gimp_resources(resource_temp.path());

        let item = with_profile_env(app_temp.path(), || {
            gimp_plugin_runtime_item(
                Some(resource_temp.path()),
                Some(app_temp.path()),
                Some(Path::new("/Program Files/GIMP 3/bin/gimp-3.exe")),
            )
        });

        assert_eq!(item.state, SetupItemStateDto::NotPrepared);
        assert!(item.can_prepare);
    }

    #[test]
    fn ensure_gimp_plugin_runtime_prepared_copies_plugin_and_writes_marker() {
        let resource_temp = TempDir::new().expect("resource temp");
        let app_temp = TempDir::new().expect("app temp");
        write_gimp_resources(resource_temp.path());

        with_profile_env(app_temp.path(), || {
            let outcome = ensure_gimp_plugin_runtime_prepared(
                Some(resource_temp.path()),
                Some(app_temp.path()),
                Path::new("/Program Files/GIMP 3/bin/gimp-3.exe"),
            )
            .expect("prepare gimp resources");

            match outcome {
                GimpPluginRuntimePrepareOutcome::Prepared(marker)
                | GimpPluginRuntimePrepareOutcome::AlreadyReady(marker) => {
                    let target_entry = PathBuf::from(marker.provision_target_entry);
                    assert!(target_entry.is_file());
                }
            }

            let marker_path =
                provision_marker_path(Some(app_temp.path())).expect("marker path available");
            assert!(marker_path.is_file());
        });
    }
}
