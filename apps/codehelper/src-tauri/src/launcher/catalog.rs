use super::types::{LauncherAppSummary, LauncherManifest, LauncherManifestApp};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::Manager;

const MANIFEST_ENV_VAR: &str = "SMOLPC_LAUNCHER_MANIFEST";
const MANIFEST_RELATIVE_PATH: &str = "launcher/apps.manifest.json";

pub fn load_manifest(app_handle: &tauri::AppHandle) -> Result<LauncherManifest, String> {
    let manifest_path = resolve_manifest_path(app_handle)?;
    let raw_manifest = std::fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "Failed to read launcher manifest {}: {error}",
            manifest_path.display()
        )
    })?;
    let manifest = serde_json::from_str::<LauncherManifest>(&raw_manifest).map_err(|error| {
        format!(
            "Failed to parse launcher manifest {}: {error}",
            manifest_path.display()
        )
    })?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn list_apps(app_handle: &tauri::AppHandle) -> Result<Vec<LauncherAppSummary>, String> {
    let manifest = load_manifest(app_handle)?;
    Ok(manifest.apps.iter().map(LauncherAppSummary::from).collect())
}

pub fn find_app(manifest: &LauncherManifest, app_id: &str) -> Result<LauncherManifestApp, String> {
    manifest
        .apps
        .iter()
        .find(|entry| entry.app_id == app_id)
        .cloned()
        .ok_or_else(|| format!("Unknown launcher app_id '{app_id}'"))
}

fn resolve_manifest_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var(MANIFEST_ENV_VAR) {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Ok(candidate);
        }
        return Err(format!(
            "{MANIFEST_ENV_VAR} points to missing manifest: {}",
            candidate.display()
        ));
    }

    let mut candidates = Vec::new();
    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        candidates.push(resource_dir.join(MANIFEST_RELATIVE_PATH));
    }

    let dev_candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("launcher")
        .join("apps.manifest.json");
    candidates.push(dev_candidate);

    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    Err(format!(
        "Launcher manifest not found. Checked: {}",
        candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn validate_manifest(manifest: &LauncherManifest) -> Result<(), String> {
    if manifest.apps.is_empty() {
        return Err("Launcher manifest must include at least one app entry".to_string());
    }

    let mut seen_ids = HashSet::new();
    for app in &manifest.apps {
        let app_id = app.app_id.trim();
        if app_id.is_empty() {
            return Err("Launcher manifest contains an app with empty app_id".to_string());
        }
        if !seen_ids.insert(app_id.to_string()) {
            return Err(format!(
                "Launcher manifest contains duplicate app_id '{app_id}'"
            ));
        }
        if app.display_name.trim().is_empty() {
            return Err(format!(
                "Launcher app '{app_id}' must include a non-empty display_name"
            ));
        }

        validate_absolute_executable(app_id, &app.exe_path)?;
        validate_focus_command(app_id, app.focus_command.as_ref())?;
        validate_min_engine_api_major(app_id, app.min_engine_api_major)?;
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

fn validate_focus_command(app_id: &str, focus_command: Option<&Vec<String>>) -> Result<(), String> {
    let Some(focus_command) = focus_command else {
        return Ok(());
    };

    if focus_command.is_empty() {
        return Err(format!(
            "Launcher app '{app_id}' focus_command must contain at least one token"
        ));
    }
    if focus_command[0].trim().is_empty() {
        return Err(format!(
            "Launcher app '{app_id}' focus_command executable token cannot be empty"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_manifest_rejects_relative_executable_path() {
        let manifest = LauncherManifest {
            apps: vec![LauncherManifestApp {
                app_id: "codehelper".to_string(),
                display_name: "Code Helper".to_string(),
                exe_path: "relative/path.exe".to_string(),
                args: vec![],
                focus_command: None,
                min_engine_api_major: Some(1),
            }],
        };

        let error = validate_manifest(&manifest).expect_err("relative path must fail");
        assert!(error.contains("must be absolute"));
    }

    #[test]
    fn validate_manifest_rejects_duplicate_app_ids() {
        let entry = LauncherManifestApp {
            app_id: "dup".to_string(),
            display_name: "App".to_string(),
            exe_path: "C:\\Program Files\\App\\app.exe".to_string(),
            args: vec![],
            focus_command: None,
            min_engine_api_major: Some(1),
        };
        let manifest = LauncherManifest {
            apps: vec![entry.clone(), entry],
        };

        let error = validate_manifest(&manifest).expect_err("duplicate ids must fail");
        assert!(error.contains("duplicate app_id"));
    }
}
