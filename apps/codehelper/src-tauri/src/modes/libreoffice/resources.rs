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
    pub main_py_path: PathBuf,
    pub libre_py_path: PathBuf,
    pub helper_py_path: PathBuf,
    pub helper_utils_py_path: PathBuf,
    pub helper_test_functions_py_path: PathBuf,
}

fn resource_candidates(
    resource_dir: Option<&Path>,
    options: ResourceResolutionOptions,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(resource_dir) = resource_dir {
        candidates.push(
            resource_dir
                .join("resources")
                .join("libreoffice")
                .join("mcp_server"),
        );
        candidates.push(resource_dir.join("libreoffice").join("mcp_server"));
    }

    if options.allow_dev_fallback {
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("libreoffice")
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
        let main_py_path = candidate.join("main.py");
        let libre_py_path = candidate.join("libre.py");
        let helper_py_path = candidate.join("helper.py");
        let helper_utils_py_path = candidate.join("helper_utils.py");
        let helper_test_functions_py_path = candidate.join("helper_test_functions.py");

        for (required_path, label) in [
            (&readme_path, "README.md"),
            (&main_py_path, "main.py"),
            (&libre_py_path, "libre.py"),
            (&helper_py_path, "helper.py"),
            (&helper_utils_py_path, "helper_utils.py"),
            (&helper_test_functions_py_path, "helper_test_functions.py"),
        ] {
            if !required_path.is_file() {
                return Err(format!(
                    "LibreOffice MCP resource directory exists at {} but {} is missing.",
                    candidate.display(),
                    label
                ));
            }
        }

        return Ok(LibreOfficeResourceLayout {
            mcp_server_dir: candidate,
            readme_path,
            main_py_path,
            libre_py_path,
            helper_py_path,
            helper_utils_py_path,
            helper_test_functions_py_path,
        });
    }

    Err("LibreOffice MCP runtime resources are not bundled yet. Expected README.md, main.py, libre.py, helper.py, helper_utils.py, and helper_test_functions.py under resources/libreoffice/mcp_server.".to_string())
}

#[cfg(test)]
mod tests {
    use super::{resolve_mcp_server_layout, ResourceResolutionOptions};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn resolves_staged_resource_layout() {
        let tempdir = tempdir().expect("tempdir");
        let staged_dir = tempdir
            .path()
            .join("resources")
            .join("libreoffice")
            .join("mcp_server");
        fs::create_dir_all(&staged_dir).expect("create staged dir");
        fs::write(staged_dir.join("README.md"), "placeholder").expect("write readme");
        fs::write(staged_dir.join("main.py"), "print('main')").expect("write main");
        fs::write(staged_dir.join("libre.py"), "print('libre')").expect("write libre");
        fs::write(staged_dir.join("helper.py"), "print('helper')").expect("write helper");
        fs::write(staged_dir.join("helper_utils.py"), "print('utils')").expect("write utils");
        fs::write(
            staged_dir.join("helper_test_functions.py"),
            "print('tests')",
        )
        .expect("write helper tests");

        let layout = resolve_mcp_server_layout(
            Some(tempdir.path()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
                allow_system_python_fallback: true,
            },
        )
        .expect("resolve layout");

        assert_eq!(layout.mcp_server_dir, staged_dir);
        assert!(layout.readme_path.ends_with("README.md"));
        assert!(layout.main_py_path.ends_with("main.py"));
        assert!(layout.libre_py_path.ends_with("libre.py"));
    }

    #[test]
    fn missing_runtime_asset_is_reported_as_an_error() {
        let tempdir = tempdir().expect("tempdir");
        let staged_dir = tempdir
            .path()
            .join("resources")
            .join("libreoffice")
            .join("mcp_server");
        fs::create_dir_all(&staged_dir).expect("create staged dir");
        fs::write(staged_dir.join("README.md"), "placeholder").expect("write readme");
        fs::write(staged_dir.join("main.py"), "print('main')").expect("write main");

        let error = resolve_mcp_server_layout(
            Some(tempdir.path()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
                allow_system_python_fallback: true,
            },
        )
        .expect_err("missing readme should fail");

        assert!(error.contains("libre.py is missing"));
    }

    #[test]
    fn repo_bundled_runtime_assets_resolve_in_dev_mode() {
        let layout = resolve_mcp_server_layout(None, ResourceResolutionOptions::default())
            .expect("resolve repo layout");

        assert!(layout.main_py_path.ends_with("main.py"));
        assert!(layout.helper_py_path.ends_with("helper.py"));
    }
}
