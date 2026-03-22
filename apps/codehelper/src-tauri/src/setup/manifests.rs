use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceManifest {
    pub version: String,
    pub source: String,
    pub expected_paths: Vec<String>,
    pub status: String,
}

pub fn resource_root(resource_dir: Option<&Path>, resource_name: &str) -> Result<PathBuf, String> {
    let base = resource_dir.ok_or_else(|| "Tauri resource directory is unavailable".to_string())?;
    let root = base.join(resource_name);
    if root.exists() {
        Ok(root)
    } else {
        Err(format!(
            "Bundled resource root is missing: {}",
            root.display()
        ))
    }
}

pub fn manifest_path(root: &Path) -> PathBuf {
    root.join("manifest.json")
}

pub fn load_manifest(root: &Path) -> Result<ResourceManifest, String> {
    let path = manifest_path(root);
    let raw = std::fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read manifest {}: {error}", path.display()))?;
    let manifest: ResourceManifest = serde_json::from_str(&raw)
        .map_err(|error| format!("Failed to parse manifest {}: {error}", path.display()))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_manifest(manifest: &ResourceManifest) -> Result<(), String> {
    if manifest.version.trim().is_empty() {
        return Err("Manifest version must not be empty".to_string());
    }
    if manifest.source.trim().is_empty() {
        return Err("Manifest source must not be empty".to_string());
    }
    if manifest.status.trim().is_empty() {
        return Err("Manifest status must not be empty".to_string());
    }
    if manifest.expected_paths.is_empty() {
        return Err("Manifest expectedPaths must not be empty".to_string());
    }
    if manifest
        .expected_paths
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err("Manifest expectedPaths entries must not be empty".to_string());
    }
    Ok(())
}

pub fn missing_expected_paths(root: &Path, manifest: &ResourceManifest) -> Vec<String> {
    manifest
        .expected_paths
        .iter()
        .filter(|relative| !root.join(relative.as_str()).exists())
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{load_manifest, missing_expected_paths, validate_manifest, ResourceManifest};
    use tempfile::TempDir;

    #[test]
    fn validate_manifest_rejects_empty_values() {
        let manifest = ResourceManifest {
            version: "".to_string(),
            source: "phase2".to_string(),
            expected_paths: vec!["payload".to_string()],
            status: "placeholder".to_string(),
        };

        let error = validate_manifest(&manifest).expect_err("validation should fail");
        assert!(error.contains("version"));
    }

    #[test]
    fn missing_expected_paths_reports_relative_entries() {
        let temp = TempDir::new().expect("temp dir");
        std::fs::create_dir_all(temp.path().join("payload")).expect("payload dir");

        let manifest = ResourceManifest {
            version: "1".to_string(),
            source: "tests".to_string(),
            expected_paths: vec!["payload".to_string(), "README.md".to_string()],
            status: "placeholder".to_string(),
        };

        let missing = missing_expected_paths(temp.path(), &manifest);
        assert_eq!(missing, vec!["README.md".to_string()]);
    }

    #[test]
    fn load_manifest_parses_camel_case_file() {
        let temp = TempDir::new().expect("temp dir");
        let path = temp.path().join("manifest.json");
        std::fs::write(
            &path,
            r#"{
              "version": "phase2",
              "source": "tests",
              "expectedPaths": ["payload"],
              "status": "placeholder"
            }"#,
        )
        .expect("write manifest");

        let manifest = load_manifest(temp.path()).expect("load manifest");
        assert_eq!(manifest.version, "phase2");
        assert_eq!(manifest.expected_paths, vec!["payload"]);
    }
}
