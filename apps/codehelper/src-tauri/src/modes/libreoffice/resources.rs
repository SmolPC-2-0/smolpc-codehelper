use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceResolutionOptions {
    pub allow_dev_fallback: bool,
}

impl Default for ResourceResolutionOptions {
    fn default() -> Self {
        Self {
            allow_dev_fallback: cfg!(debug_assertions),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibreOfficeResourceLayout {
    pub mcp_server_dir: PathBuf,
    pub readme_path: PathBuf,
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
        if !readme_path.is_file() {
            return Err(format!(
                "LibreOffice scaffold resource directory exists at {} but README.md is missing.",
                candidate.display()
            ));
        }

        return Ok(LibreOfficeResourceLayout {
            mcp_server_dir: candidate,
            readme_path,
        });
    }

    Err("LibreOffice scaffold resources are not staged yet. Expected a tracked README under resources/libreoffice/mcp_server.".to_string())
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

        let layout = resolve_mcp_server_layout(
            Some(tempdir.path()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
            },
        )
        .expect("resolve layout");

        assert_eq!(layout.mcp_server_dir, staged_dir);
        assert!(layout.readme_path.ends_with("README.md"));
    }

    #[test]
    fn missing_readme_is_reported_as_an_error() {
        let tempdir = tempdir().expect("tempdir");
        let staged_dir = tempdir
            .path()
            .join("resources")
            .join("libreoffice")
            .join("mcp_server");
        fs::create_dir_all(&staged_dir).expect("create staged dir");

        let error = resolve_mcp_server_layout(
            Some(tempdir.path()),
            ResourceResolutionOptions {
                allow_dev_fallback: false,
            },
        )
        .expect_err("missing readme should fail");

        assert!(error.contains("README.md is missing"));
    }
}
