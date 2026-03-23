use std::path::{Path, PathBuf};

const KNOWN_BUNDLED_RESOURCE_ROOTS: [&str; 5] =
    ["python", "models", "gimp", "blender", "libreoffice"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BundledResourceDirResolution {
    Direct(PathBuf),
    NestedResources(PathBuf),
    DevFallback(PathBuf),
}

pub(crate) fn default_dev_bundled_resource_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources")
}

pub(crate) fn bundled_resource_dir_path(resolution: &BundledResourceDirResolution) -> &Path {
    match resolution {
        BundledResourceDirResolution::Direct(path)
        | BundledResourceDirResolution::NestedResources(path)
        | BundledResourceDirResolution::DevFallback(path) => path,
    }
}

pub(crate) fn bundled_resource_dir_source(
    resolution: &BundledResourceDirResolution,
) -> &'static str {
    match resolution {
        BundledResourceDirResolution::Direct(_) => "Tauri resource directory",
        BundledResourceDirResolution::NestedResources(_) => "Tauri resource directory/resources",
        BundledResourceDirResolution::DevFallback(_) => "debug manifest-dir resources fallback",
    }
}

pub(crate) fn select_bundled_resource_dir_resolution(
    tauri_result: Result<PathBuf, String>,
    is_debug: bool,
    dev_fallback_root: Option<PathBuf>,
) -> Option<BundledResourceDirResolution> {
    match tauri_result {
        Ok(path) => normalize_tauri_resource_dir(path).or_else(|| {
            if !is_debug {
                None
            } else {
                dev_fallback_root
                    .filter(|path| contains_known_resource_root(path))
                    .map(BundledResourceDirResolution::DevFallback)
            }
        }),
        Err(_) if !is_debug => None,
        Err(_) => dev_fallback_root
            .filter(|path| contains_known_resource_root(path))
            .map(BundledResourceDirResolution::DevFallback),
    }
}

fn normalize_tauri_resource_dir(path: PathBuf) -> Option<BundledResourceDirResolution> {
    if contains_known_resource_root(&path) {
        return Some(BundledResourceDirResolution::Direct(path));
    }

    let nested = path.join("resources");
    if contains_known_resource_root(&nested) {
        Some(BundledResourceDirResolution::NestedResources(nested))
    } else {
        None
    }
}

fn contains_known_resource_root(path: &Path) -> bool {
    KNOWN_BUNDLED_RESOURCE_ROOTS
        .iter()
        .any(|resource| path.join(resource).exists())
}

#[cfg(test)]
mod tests {
    use super::{
        bundled_resource_dir_path, bundled_resource_dir_source, default_dev_bundled_resource_dir,
        select_bundled_resource_dir_resolution, BundledResourceDirResolution,
    };
    use tempfile::TempDir;

    fn write_resource_root(base: &std::path::Path, name: &str) {
        std::fs::create_dir_all(base.join(name)).expect("resource root");
    }

    #[test]
    fn bundled_resource_dir_resolution_preserves_direct_tauri_root() {
        let temp = TempDir::new().expect("temp dir");
        write_resource_root(temp.path(), "python");

        let resolution = select_bundled_resource_dir_resolution(
            Ok(temp.path().to_path_buf()),
            true,
            Some(default_dev_bundled_resource_dir()),
        )
        .expect("resolution");

        assert_eq!(
            resolution,
            BundledResourceDirResolution::Direct(temp.path().to_path_buf())
        );
        assert_eq!(bundled_resource_dir_path(&resolution), temp.path());
        assert_eq!(
            bundled_resource_dir_source(&resolution),
            "Tauri resource directory"
        );
    }

    #[test]
    fn bundled_resource_dir_resolution_normalizes_nested_resources_root() {
        let temp = TempDir::new().expect("temp dir");
        let nested = temp.path().join("resources");
        write_resource_root(&nested, "models");

        let resolution = select_bundled_resource_dir_resolution(
            Ok(temp.path().to_path_buf()),
            true,
            Some(default_dev_bundled_resource_dir()),
        )
        .expect("resolution");

        assert_eq!(
            resolution,
            BundledResourceDirResolution::NestedResources(nested.clone())
        );
        assert_eq!(bundled_resource_dir_path(&resolution), nested);
        assert_eq!(
            bundled_resource_dir_source(&resolution),
            "Tauri resource directory/resources"
        );
    }

    #[test]
    fn bundled_resource_dir_resolution_uses_debug_fallback_when_tauri_is_unusable() {
        let dev = TempDir::new().expect("dev dir");
        write_resource_root(dev.path(), "gimp");

        let resolution = select_bundled_resource_dir_resolution(
            Err("resource dir unavailable".to_string()),
            true,
            Some(dev.path().to_path_buf()),
        )
        .expect("resolution");

        assert_eq!(
            resolution,
            BundledResourceDirResolution::DevFallback(dev.path().to_path_buf())
        );
        assert_eq!(
            bundled_resource_dir_source(&resolution),
            "debug manifest-dir resources fallback"
        );
    }

    #[test]
    fn bundled_resource_dir_resolution_skips_debug_fallback_outside_debug_builds() {
        let dev = TempDir::new().expect("dev dir");
        write_resource_root(dev.path(), "blender");

        let resolution = select_bundled_resource_dir_resolution(
            Err("resource dir unavailable".to_string()),
            false,
            Some(dev.path().to_path_buf()),
        );

        assert_eq!(resolution, None);
    }

    #[test]
    fn bundled_resource_dir_resolution_returns_none_for_unusable_candidates() {
        let temp = TempDir::new().expect("temp dir");
        let dev = TempDir::new().expect("dev dir");

        let resolution = select_bundled_resource_dir_resolution(
            Ok(temp.path().to_path_buf()),
            true,
            Some(dev.path().to_path_buf()),
        );

        assert_eq!(resolution, None);
    }
}
