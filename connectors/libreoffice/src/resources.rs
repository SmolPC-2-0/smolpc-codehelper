use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceResolutionOptions {
    pub allow_dev_fallback: bool,
    pub allow_system_python_fallback: bool,
}

impl Default for ResourceResolutionOptions {
    fn default() -> Self {
        Self {
            allow_dev_fallback: cfg!(debug_assertions),
            allow_system_python_fallback: cfg!(debug_assertions),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibreOfficeResourceLayout {
    pub mcp_server_dir: PathBuf,
    pub readme_path: PathBuf,
    pub mcp_server_py_path: PathBuf,
    pub test_functions_py_path: PathBuf,
}

fn resource_candidates(
    resource_dir: Option<&Path>,
    options: ResourceResolutionOptions,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(resource_dir) = resource_dir {
        candidates.push(resource_dir.join("libreoffice").join("mcp_server"));
        candidates.push(
            resource_dir
                .join("resources")
                .join("libreoffice")
                .join("mcp_server"),
        );
    }

    if options.allow_dev_fallback {
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("mcp_server"),
        );
    }

    candidates
}

pub fn resolve_mcp_server_layout(
    resource_dir: Option<&Path>,
    options: ResourceResolutionOptions,
) -> Result<LibreOfficeResourceLayout, String> {
    for candidate in resource_candidates(resource_dir, options) {
        if !candidate.exists() {
            continue;
        }

        if !candidate.is_dir() {
            return Err(format!(
                "LibreOffice scaffold resource path exists but is not a directory: {}",
                candidate.display()
            ));
        }

        let readme_path = candidate.join("README.md");
        let mcp_server_py_path = candidate.join("mcp_server.py");
        let test_functions_py_path = candidate.join("test_functions.py");

        for (required_path, label) in [
            (&readme_path, "README.md"),
            (&mcp_server_py_path, "mcp_server.py"),
            (&test_functions_py_path, "test_functions.py"),
        ] {
            if !required_path.is_file() {
                return Err(format!(
                    "LibreOffice MCP resource candidate {} is missing required file {}.",
                    candidate.display(),
                    label
                ));
            }
        }

        return Ok(LibreOfficeResourceLayout {
            mcp_server_dir: candidate,
            readme_path,
            mcp_server_py_path,
            test_functions_py_path,
        });
    }

    Err("LibreOffice MCP runtime resources are not bundled yet. Expected README.md, mcp_server.py, and test_functions.py under resources/libreoffice/mcp_server.".to_string())
}

#[cfg(test)]
mod tests {
    use super::{resolve_mcp_server_layout, ResourceResolutionOptions};
    use std::fs;
    use tempfile::tempdir;

    fn write_runtime_assets(root: &std::path::Path) {
        fs::create_dir_all(root).expect("create staged dir");
        fs::write(root.join("README.md"), "placeholder").expect("write readme");
        fs::write(root.join("mcp_server.py"), "print('mcp_server')").expect("write mcp_server");
        fs::write(root.join("test_functions.py"), "print('test_functions')")
            .expect("write test_functions");
    }

    #[test]
    fn falls_back_to_nested_when_direct_missing() {
        let tempdir = tempdir().expect("tempdir");
        let nested_dir = tempdir
            .path()
            .join("resources")
            .join("libreoffice")
            .join("mcp_server");
        write_runtime_assets(&nested_dir);

        let layout = resolve_mcp_server_layout(
            Some(tempdir.path()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
                allow_system_python_fallback: true,
            },
        )
        .expect("resolve layout");

        assert_eq!(layout.mcp_server_dir, nested_dir);
        assert!(layout.readme_path.ends_with("README.md"));
        assert!(layout.mcp_server_py_path.ends_with("mcp_server.py"));
    }

    #[test]
    fn prefers_direct_candidate_when_direct_and_nested_both_exist() {
        let tempdir = tempdir().expect("tempdir");
        let direct_dir = tempdir.path().join("libreoffice").join("mcp_server");
        let nested_dir = tempdir
            .path()
            .join("resources")
            .join("libreoffice")
            .join("mcp_server");
        write_runtime_assets(&direct_dir);
        write_runtime_assets(&nested_dir);

        let layout = resolve_mcp_server_layout(
            Some(tempdir.path()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
                allow_system_python_fallback: true,
            },
        )
        .expect("resolve layout");

        assert_eq!(layout.mcp_server_dir, direct_dir);
    }

    #[test]
    fn still_reports_missing_required_asset_with_candidate_path_context() {
        let tempdir = tempdir().expect("tempdir");
        let direct_dir = tempdir.path().join("libreoffice").join("mcp_server");
        fs::create_dir_all(&direct_dir).expect("create staged dir");
        fs::write(direct_dir.join("README.md"), "placeholder").expect("write readme");

        let error = resolve_mcp_server_layout(
            Some(tempdir.path()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
                allow_system_python_fallback: true,
            },
        )
        .expect_err("missing mcp_server.py should fail");

        assert!(error.contains("mcp_server.py"), "error should mention mcp_server.py: {error}");
        assert!(error.contains(direct_dir.to_string_lossy().as_ref()));
    }

    #[test]
    fn repo_bundled_runtime_assets_resolve_in_dev_mode() {
        let layout = resolve_mcp_server_layout(None, ResourceResolutionOptions::default())
            .expect("resolve repo layout");

        assert!(layout.mcp_server_py_path.ends_with("mcp_server.py"));
    }
}
