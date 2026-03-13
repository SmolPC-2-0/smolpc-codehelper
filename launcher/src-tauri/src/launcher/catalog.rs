use super::types::{
    LauncherCatalog, LauncherCatalogApp, LauncherInstallState, LauncherRegistry,
    LauncherRegistryApp, ResolvedLauncherApp, LAUNCHER_REGISTRY_SCHEMA_VERSION,
};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::Manager;

const CATALOG_ENV_VAR: &str = "SMOLPC_LAUNCHER_CATALOG";
const CATALOG_RELATIVE_PATH: &str = "launcher/apps.catalog.json";

const REGISTRY_ENV_VAR: &str = "SMOLPC_LAUNCHER_REGISTRY";
const REGISTRY_VENDOR_DIR: &str = "SmolPC";
const REGISTRY_APP_DIR: &str = "launcher";
const REGISTRY_FILE_NAME: &str = "apps.registry.json";

const LOCK_TIMEOUT: Duration = Duration::from_secs(5);
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(50);
const STALE_LOCK_AGE: Duration = Duration::from_secs(30);

struct FileLockGuard {
    path: PathBuf,
}

impl Drop for FileLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn load_catalog(app_handle: &tauri::AppHandle) -> Result<LauncherCatalog, String> {
    let catalog_path = resolve_catalog_path(app_handle)?;
    load_catalog_from_path(&catalog_path)
}

pub fn load_catalog_for_process() -> Result<LauncherCatalog, String> {
    let catalog_path = resolve_catalog_path_for_process()?;
    load_catalog_from_path(&catalog_path)
}

pub fn find_catalog_app(
    catalog: &LauncherCatalog,
    app_id: &str,
) -> Result<LauncherCatalogApp, String> {
    catalog
        .apps
        .iter()
        .find(|app| app.app_id == app_id)
        .cloned()
        .ok_or_else(|| format!("Unknown launcher app_id '{app_id}'"))
}

pub fn resolve_catalog_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let mut candidates = build_catalog_candidates(app_handle.path().resource_dir().ok(), true);
    resolve_first_existing(CATALOG_ENV_VAR, &mut candidates)
}

pub fn resolve_catalog_path_for_process() -> Result<PathBuf, String> {
    let mut candidates = build_catalog_candidates(None, true);
    resolve_first_existing(CATALOG_ENV_VAR, &mut candidates)
}

pub fn resolve_registry_path(_app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    resolve_registry_path_for_process()
}

pub fn resolve_registry_path_for_process() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var(REGISTRY_ENV_VAR) {
        return Ok(PathBuf::from(path));
    }

    let Some(local_data_dir) = dirs::data_local_dir() else {
        return Err("Failed to resolve local data directory for launcher registry".to_string());
    };

    Ok(local_data_dir
        .join(REGISTRY_VENDOR_DIR)
        .join(REGISTRY_APP_DIR)
        .join(REGISTRY_FILE_NAME))
}

pub fn load_registry(app_handle: &tauri::AppHandle) -> Result<LauncherRegistry, String> {
    let registry_path = resolve_registry_path(app_handle)?;
    load_registry_from_path(&registry_path)
}

pub fn load_registry_for_process() -> Result<LauncherRegistry, String> {
    let registry_path = resolve_registry_path_for_process()?;
    load_registry_from_path(&registry_path)
}

pub fn load_registry_from_path(registry_path: &Path) -> Result<LauncherRegistry, String> {
    if !registry_path.exists() {
        return Ok(LauncherRegistry::default());
    }

    let raw = fs::read_to_string(registry_path).map_err(|error| {
        format!(
            "Failed to read launcher registry {}: {error}",
            registry_path.display()
        )
    })?;
    let registry = serde_json::from_str::<LauncherRegistry>(&raw).map_err(|error| {
        format!(
            "Failed to parse launcher registry {}: {error}",
            registry_path.display()
        )
    })?;
    validate_registry(&registry)?;
    Ok(registry)
}

pub fn upsert_registry_entry(
    app_handle: &tauri::AppHandle,
    mut entry: LauncherRegistryApp,
) -> Result<(), String> {
    let registry_path = resolve_registry_path(app_handle)?;
    upsert_registry_entry_at(&registry_path, &mut entry)
}

pub fn upsert_registry_entry_at(
    registry_path: &Path,
    entry: &mut LauncherRegistryApp,
) -> Result<(), String> {
    validate_registry_entry(entry)?;
    let lock_path = lock_path_for_registry(registry_path);
    let _guard = acquire_registry_lock(&lock_path)?;

    let mut registry = load_registry_from_path(registry_path)?;
    registry.schema_version = LAUNCHER_REGISTRY_SCHEMA_VERSION;

    if let Some(existing) = registry
        .apps
        .iter_mut()
        .find(|app| app.app_id == entry.app_id)
    {
        *existing = entry.clone();
    } else {
        registry.apps.push(entry.clone());
    }

    registry
        .apps
        .sort_by(|left, right| left.app_id.cmp(&right.app_id));
    validate_registry(&registry)?;
    write_registry_atomic(registry_path, &registry)
}

pub fn remove_registry_entry(app_handle: &tauri::AppHandle, app_id: &str) -> Result<bool, String> {
    let registry_path = resolve_registry_path(app_handle)?;
    remove_registry_entry_at(&registry_path, app_id)
}

pub fn remove_registry_entry_at(registry_path: &Path, app_id: &str) -> Result<bool, String> {
    let lock_path = lock_path_for_registry(registry_path);
    let _guard = acquire_registry_lock(&lock_path)?;

    let mut registry = load_registry_from_path(registry_path)?;
    let before = registry.apps.len();
    registry.apps.retain(|app| app.app_id != app_id);
    let removed = before != registry.apps.len();

    if removed {
        validate_registry(&registry)?;
        write_registry_atomic(registry_path, &registry)?;
    }

    Ok(removed)
}

pub fn merge_catalog_and_registry(
    catalog: &LauncherCatalog,
    registry: &LauncherRegistry,
) -> Vec<ResolvedLauncherApp> {
    catalog
        .apps
        .iter()
        .map(|catalog_app| {
            let registration = registry
                .apps
                .iter()
                .find(|entry| entry.app_id == catalog_app.app_id)
                .cloned();
            let install_state = match registration.as_ref() {
                None => LauncherInstallState::NotInstalled,
                Some(entry) => {
                    if entry.executable_path().exists() {
                        LauncherInstallState::Installed
                    } else {
                        LauncherInstallState::Broken
                    }
                }
            };

            ResolvedLauncherApp {
                catalog: catalog_app.clone(),
                registration,
                install_state,
            }
        })
        .collect()
}

pub fn resolve_app(
    app_handle: &tauri::AppHandle,
    app_id: &str,
) -> Result<ResolvedLauncherApp, String> {
    let catalog = load_catalog(app_handle)?;
    let registry = load_registry(app_handle)?;
    resolve_app_from_sources(&catalog, &registry, app_id)
}

pub fn resolve_app_from_sources(
    catalog: &LauncherCatalog,
    registry: &LauncherRegistry,
    app_id: &str,
) -> Result<ResolvedLauncherApp, String> {
    merge_catalog_and_registry(catalog, registry)
        .into_iter()
        .find(|entry| entry.catalog.app_id == app_id)
        .ok_or_else(|| format!("Unknown launcher app_id '{app_id}'"))
}

pub fn now_utc_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    seconds.to_string()
}

fn resolve_first_existing(env_var: &str, candidates: &mut Vec<PathBuf>) -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var(env_var) {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Ok(candidate);
        }
        return Err(format!(
            "{env_var} points to missing file: {}",
            candidate.display()
        ));
    }

    for candidate in candidates.iter() {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    Err(format!(
        "Launcher catalog not found. Checked: {}",
        candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn build_catalog_candidates(
    resource_dir: Option<PathBuf>,
    include_process_candidates: bool,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(resource_dir) = resource_dir {
        candidates.push(resource_dir.join(CATALOG_RELATIVE_PATH));
    }

    if include_process_candidates {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                candidates.push(
                    exe_dir
                        .join("resources")
                        .join("launcher")
                        .join("apps.catalog.json"),
                );
                candidates.push(exe_dir.join("launcher").join("apps.catalog.json"));
            }
        }
    }

    let dev_candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("launcher")
        .join("apps.catalog.json");
    candidates.push(dev_candidate);

    candidates
}

fn validate_catalog(catalog: &LauncherCatalog) -> Result<(), String> {
    if catalog.apps.is_empty() {
        return Err("Launcher catalog must include at least one app entry".to_string());
    }

    let mut seen_ids = HashSet::new();
    for app in &catalog.apps {
        let app_id = app.app_id.trim();
        if app_id.is_empty() {
            return Err("Launcher catalog contains an app with empty app_id".to_string());
        }
        if !seen_ids.insert(app_id.to_string()) {
            return Err(format!(
                "Launcher catalog contains duplicate app_id '{app_id}'"
            ));
        }
        if app.display_name.trim().is_empty() {
            return Err(format!(
                "Launcher app '{app_id}' must include a non-empty display_name"
            ));
        }

        if let Some(installer) = &app.installer {
            let installer_url = installer.url.trim();
            if installer_url.is_empty() {
                return Err(format!(
                    "Launcher app '{app_id}' installer.url must be non-empty when installer is configured"
                ));
            }

            if is_http_url(installer_url) {
                return Err(format!(
                    "Launcher app '{app_id}' installer.url must not use insecure http://"
                ));
            }

            let digest = installer.sha256.as_deref().map(str::trim);
            let digest = digest.filter(|digest| !digest.is_empty());
            if is_https_url(installer_url) {
                let Some(digest) = digest else {
                    return Err(format!(
                        "Launcher app '{app_id}' remote installer.url requires installer.sha256"
                    ));
                };
                if !is_valid_sha256_hex(digest) {
                    return Err(format!(
                        "Launcher app '{app_id}' installer.sha256 must be a 64-char hex digest for remote installers"
                    ));
                }
            } else if let Some(digest) = digest {
                if !is_valid_sha256_hex(digest) {
                    return Err(format!(
                        "Launcher app '{app_id}' installer.sha256 must be a 64-char hex digest when provided"
                    ));
                }
            }
        }

        validate_min_engine_api_major(app_id, app.min_engine_api_major)?;
    }

    Ok(())
}

fn load_catalog_from_path(catalog_path: &Path) -> Result<LauncherCatalog, String> {
    let raw_catalog = fs::read_to_string(catalog_path).map_err(|error| {
        format!(
            "Failed to read launcher catalog {}: {error}",
            catalog_path.display()
        )
    })?;
    let catalog = serde_json::from_str::<LauncherCatalog>(&raw_catalog).map_err(|error| {
        format!(
            "Failed to parse launcher catalog {}: {error}",
            catalog_path.display()
        )
    })?;
    validate_catalog(&catalog)?;
    Ok(catalog)
}

fn validate_registry(registry: &LauncherRegistry) -> Result<(), String> {
    if registry.schema_version != LAUNCHER_REGISTRY_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported launcher registry schema_version {} (expected {})",
            registry.schema_version, LAUNCHER_REGISTRY_SCHEMA_VERSION
        ));
    }

    let mut seen_ids = HashSet::new();
    for entry in &registry.apps {
        let app_id = entry.app_id.trim();
        if app_id.is_empty() {
            return Err("Launcher registry contains an entry with empty app_id".to_string());
        }
        if !seen_ids.insert(app_id.to_string()) {
            return Err(format!(
                "Launcher registry contains duplicate app_id '{app_id}'"
            ));
        }

        validate_registry_entry(entry)?;
    }

    Ok(())
}

fn validate_registry_entry(entry: &LauncherRegistryApp) -> Result<(), String> {
    let app_id = entry.app_id.trim();
    if app_id.is_empty() {
        return Err("Launcher registry entry app_id cannot be empty".to_string());
    }
    validate_absolute_executable(app_id, &entry.exe_path)?;
    validate_command_tokens(app_id, "launch_command", entry.launch_command.as_ref())?;
    validate_command_tokens(app_id, "focus_command", entry.focus_command.as_ref())?;
    if entry.installed_at.trim().is_empty() {
        return Err(format!(
            "Launcher registry app '{app_id}' installed_at must be non-empty"
        ));
    }
    if entry.source.trim().is_empty() {
        return Err(format!(
            "Launcher registry app '{app_id}' source must be non-empty"
        ));
    }

    Ok(())
}

fn validate_absolute_executable(app_id: &str, executable: &str) -> Result<(), String> {
    let path = Path::new(executable);
    if !path.is_absolute() {
        return Err(format!(
            "Launcher app '{app_id}' exe_path must be absolute: '{executable}'"
        ));
    }
    Ok(())
}

fn validate_command_tokens(
    app_id: &str,
    field_name: &str,
    command: Option<&Vec<String>>,
) -> Result<(), String> {
    let Some(command) = command else {
        return Ok(());
    };

    if command.is_empty() {
        return Err(format!(
            "Launcher app '{app_id}' {field_name} must contain at least one token"
        ));
    }
    if command[0].trim().is_empty() {
        return Err(format!(
            "Launcher app '{app_id}' {field_name} executable token cannot be empty"
        ));
    }
    Ok(())
}

fn validate_min_engine_api_major(app_id: &str, major: Option<u64>) -> Result<(), String> {
    if let Some(major) = major {
        if major == 0 {
            return Err(format!(
                "Launcher app '{app_id}' min_engine_api_major must be >= 1 when provided"
            ));
        }
    }
    Ok(())
}

fn is_valid_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn is_http_url(value: &str) -> bool {
    has_url_prefix(value, "http://")
}

fn is_https_url(value: &str) -> bool {
    has_url_prefix(value, "https://")
}

fn has_url_prefix(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn lock_path_for_registry(registry_path: &Path) -> PathBuf {
    let mut lock_name = registry_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(REGISTRY_FILE_NAME)
        .to_string();
    lock_name.push_str(".lock");
    registry_path.with_file_name(lock_name)
}

fn acquire_registry_lock(lock_path: &Path) -> Result<FileLockGuard, String> {
    acquire_registry_lock_with_timeout(lock_path, LOCK_TIMEOUT)
}

fn acquire_registry_lock_with_timeout(
    lock_path: &Path,
    timeout: Duration,
) -> Result<FileLockGuard, String> {
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create launcher registry directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let started = Instant::now();
    loop {
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(lock_path)
        {
            Ok(mut file) => {
                let stamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let _ = writeln!(file, "pid={} ts={stamp}", std::process::id());
                return Ok(FileLockGuard {
                    path: lock_path.to_path_buf(),
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if is_stale_lock(lock_path)? {
                    let _ = fs::remove_file(lock_path);
                    continue;
                }
                if started.elapsed() >= timeout {
                    return Err(format!(
                        "Timed out waiting for launcher registry lock: {}",
                        lock_path.display()
                    ));
                }
                std::thread::sleep(LOCK_RETRY_DELAY);
            }
            Err(error) => {
                return Err(format!(
                    "Failed to acquire launcher registry lock {}: {error}",
                    lock_path.display()
                ));
            }
        }
    }
}

fn is_stale_lock(lock_path: &Path) -> Result<bool, String> {
    let metadata = fs::metadata(lock_path).map_err(|error| {
        format!(
            "Failed to inspect launcher registry lock {}: {error}",
            lock_path.display()
        )
    })?;
    let Ok(modified) = metadata.modified() else {
        return Ok(false);
    };
    let Ok(age) = modified.elapsed() else {
        return Ok(false);
    };
    Ok(age > STALE_LOCK_AGE)
}

fn write_registry_atomic(registry_path: &Path, registry: &LauncherRegistry) -> Result<(), String> {
    if let Some(parent) = registry_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create launcher registry directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let payload = serde_json::to_vec_pretty(registry)
        .map_err(|error| format!("Failed to serialize launcher registry: {error}"))?;
    let temp_name = format!(
        "{}.tmp.{}.{}",
        registry_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(REGISTRY_FILE_NAME),
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let temp_path = registry_path.with_file_name(temp_name);

    fs::write(&temp_path, payload).map_err(|error| {
        format!(
            "Failed to write launcher registry temp file {}: {error}",
            temp_path.display()
        )
    })?;

    replace_registry_file(&temp_path, registry_path)?;

    Ok(())
}

#[cfg(not(windows))]
fn replace_registry_file(temp_path: &Path, registry_path: &Path) -> Result<(), String> {
    fs::rename(temp_path, registry_path).map_err(|error| {
        format!(
            "Failed to finalize launcher registry {}: {error}",
            registry_path.display()
        )
    })
}

#[cfg(windows)]
fn replace_registry_file(temp_path: &Path, registry_path: &Path) -> Result<(), String> {
    let backup_path = backup_path_for_registry(registry_path);
    if backup_path.exists() {
        fs::remove_file(&backup_path).map_err(|error| {
            format!(
                "Failed to clear stale launcher registry backup {}: {error}",
                backup_path.display()
            )
        })?;
    }

    let had_existing_registry = registry_path.exists();
    if had_existing_registry {
        fs::rename(registry_path, &backup_path).map_err(|error| {
            format!(
                "Failed to create launcher registry backup {}: {error}",
                backup_path.display()
            )
        })?;
    }

    if let Err(error) = fs::rename(temp_path, registry_path) {
        if had_existing_registry {
            let restore_result = fs::rename(&backup_path, registry_path).map_err(|restore_error| {
                format!(
                    "Failed to restore launcher registry from backup {} -> {}: {restore_error}",
                    backup_path.display(),
                    registry_path.display()
                )
            });
            if let Err(restore_error) = restore_result {
                return Err(format!(
                    "Failed to finalize launcher registry {}: {error}; {restore_error}",
                    registry_path.display()
                ));
            }
        }
        return Err(format!(
            "Failed to finalize launcher registry {}: {error}",
            registry_path.display()
        ));
    }

    if had_existing_registry {
        let _ = fs::remove_file(&backup_path);
    }

    Ok(())
}

fn backup_path_for_registry(registry_path: &Path) -> PathBuf {
    let mut backup_name = registry_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(REGISTRY_FILE_NAME)
        .to_string();
    backup_name.push_str(".bak");
    registry_path.with_file_name(backup_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::types::{InstallerKind, LauncherInstallerSpec};

    fn temp_test_path(name: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .join("smolpc-launcher-tests")
            .join(format!("{}-{}", std::process::id(), now_utc_timestamp()));
        fs::create_dir_all(&root).expect("temp test root should be creatable");
        root.join(name)
    }

    #[test]
    fn validate_registry_rejects_relative_executable_path() {
        let registry = LauncherRegistry {
            schema_version: LAUNCHER_REGISTRY_SCHEMA_VERSION,
            apps: vec![LauncherRegistryApp {
                app_id: "codehelper".to_string(),
                exe_path: "relative/path.exe".to_string(),
                args: vec![],
                launch_command: None,
                focus_command: None,
                installed_at: now_utc_timestamp(),
                source: "installer".to_string(),
            }],
        };

        let error = validate_registry(&registry).expect_err("relative path must fail");
        assert!(error.contains("must be absolute"));
    }

    #[test]
    fn merge_catalog_and_registry_calculates_install_states() {
        let catalog = LauncherCatalog {
            apps: vec![
                LauncherCatalogApp {
                    app_id: "installed".to_string(),
                    display_name: "Installed".to_string(),
                    icon: None,
                    min_engine_api_major: Some(1),
                    installer: Some(LauncherInstallerSpec {
                        url: "installers/a.exe".to_string(),
                        sha256: None,
                        kind: InstallerKind::Exe,
                    }),
                },
                LauncherCatalogApp {
                    app_id: "missing".to_string(),
                    display_name: "Missing".to_string(),
                    icon: None,
                    min_engine_api_major: Some(1),
                    installer: None,
                },
            ],
        };
        let installed_path = temp_test_path("installed.exe");
        fs::write(&installed_path, b"test").expect("test exe should be created");
        let broken_path = temp_test_path("broken.exe");

        let registry = LauncherRegistry {
            schema_version: LAUNCHER_REGISTRY_SCHEMA_VERSION,
            apps: vec![
                LauncherRegistryApp {
                    app_id: "installed".to_string(),
                    exe_path: installed_path.display().to_string(),
                    args: vec![],
                    launch_command: None,
                    focus_command: None,
                    installed_at: now_utc_timestamp(),
                    source: "installer".to_string(),
                },
                LauncherRegistryApp {
                    app_id: "missing".to_string(),
                    exe_path: broken_path.display().to_string(),
                    args: vec![],
                    launch_command: None,
                    focus_command: None,
                    installed_at: now_utc_timestamp(),
                    source: "installer".to_string(),
                },
            ],
        };

        let merged = merge_catalog_and_registry(&catalog, &registry);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].install_state, LauncherInstallState::Installed);
        assert_eq!(merged[1].install_state, LauncherInstallState::Broken);
    }

    #[test]
    fn validate_catalog_rejects_http_installer_urls() {
        let catalog = LauncherCatalog {
            apps: vec![LauncherCatalogApp {
                app_id: "codehelper".to_string(),
                display_name: "Code Helper".to_string(),
                icon: None,
                min_engine_api_major: Some(1),
                installer: Some(LauncherInstallerSpec {
                    url: "http://example.com/codehelper.exe".to_string(),
                    sha256: Some("a".repeat(64)),
                    kind: InstallerKind::Exe,
                }),
            }],
        };

        let error = validate_catalog(&catalog).expect_err("http URL should fail validation");
        assert!(error.contains("must not use insecure http://"));
    }

    #[test]
    fn validate_catalog_requires_sha256_for_https_installer_urls() {
        let catalog = LauncherCatalog {
            apps: vec![LauncherCatalogApp {
                app_id: "codehelper".to_string(),
                display_name: "Code Helper".to_string(),
                icon: None,
                min_engine_api_major: Some(1),
                installer: Some(LauncherInstallerSpec {
                    url: "https://example.com/codehelper.exe".to_string(),
                    sha256: None,
                    kind: InstallerKind::Exe,
                }),
            }],
        };

        let error = validate_catalog(&catalog).expect_err("missing sha256 should fail");
        assert!(error.contains("requires installer.sha256"));
    }

    #[test]
    fn lock_guard_blocks_parallel_writer_until_timeout() {
        let registry_path = temp_test_path("apps.registry.json");
        let lock_path = lock_path_for_registry(&registry_path);
        let _first = acquire_registry_lock_with_timeout(&lock_path, Duration::from_secs(1))
            .expect("first lock should succeed");

        let second = acquire_registry_lock_with_timeout(&lock_path, Duration::from_millis(120));
        assert!(second.is_err());
    }

    #[test]
    fn backup_path_for_registry_appends_bak_suffix() {
        let registry_path = temp_test_path("apps.registry.json");
        let backup_path = backup_path_for_registry(&registry_path);
        assert_eq!(
            backup_path.file_name().and_then(|name| name.to_str()),
            Some("apps.registry.json.bak")
        );
    }

    #[test]
    fn write_registry_atomic_replaces_existing_registry_file() {
        let registry_path = temp_test_path("apps.registry.json");
        let initial_registry = LauncherRegistry {
            schema_version: LAUNCHER_REGISTRY_SCHEMA_VERSION,
            apps: vec![LauncherRegistryApp {
                app_id: "old-app".to_string(),
                exe_path: temp_test_path("old-app.exe").display().to_string(),
                args: vec![],
                launch_command: None,
                focus_command: None,
                installed_at: now_utc_timestamp(),
                source: "installer".to_string(),
            }],
        };
        let replacement_registry = LauncherRegistry {
            schema_version: LAUNCHER_REGISTRY_SCHEMA_VERSION,
            apps: vec![LauncherRegistryApp {
                app_id: "new-app".to_string(),
                exe_path: temp_test_path("new-app.exe").display().to_string(),
                args: vec![],
                launch_command: None,
                focus_command: None,
                installed_at: now_utc_timestamp(),
                source: "installer".to_string(),
            }],
        };

        write_registry_atomic(&registry_path, &initial_registry)
            .expect("initial registry write should succeed");
        write_registry_atomic(&registry_path, &replacement_registry)
            .expect("replacement registry write should succeed");

        let loaded = load_registry_from_path(&registry_path).expect("registry should be readable");
        assert_eq!(loaded.apps.len(), 1);
        assert_eq!(loaded.apps[0].app_id, "new-app");
    }
}
