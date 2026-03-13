use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const ADDON_FILE_NAME: &str = "blender_helper_http.py";
const ADDON_FILE_BYTES: &[u8] = include_bytes!("../../blender_addon/blender_helper_http.py");

pub struct AddonSyncReport {
    pub config_root: Option<PathBuf>,
    pub scanned_versions: usize,
    pub updated_targets: Vec<PathBuf>,
    pub unchanged_targets: Vec<PathBuf>,
    pub failed_targets: Vec<(PathBuf, String)>,
}

pub fn sync_blender_addon() -> Result<AddonSyncReport, String> {
    let config_root = blender_config_root();
    let mut report = AddonSyncReport {
        config_root: config_root.clone(),
        scanned_versions: 0,
        updated_targets: Vec::new(),
        unchanged_targets: Vec::new(),
        failed_targets: Vec::new(),
    };

    let Some(root) = config_root else {
        return Ok(report);
    };

    if !root.exists() {
        return Ok(report);
    }

    let mut version_dirs = discover_version_dirs(&root)
        .map_err(|err| format!("Failed to scan Blender config root '{}': {}", root.display(), err))?;
    version_dirs.sort_by(|a, b| {
        let av = parse_version_tuple(a.file_name().and_then(|n| n.to_str()).unwrap_or_default());
        let bv = parse_version_tuple(b.file_name().and_then(|n| n.to_str()).unwrap_or_default());
        bv.cmp(&av)
    });

    report.scanned_versions = version_dirs.len();

    for version_dir in version_dirs {
        let addon_target = version_dir
            .join("scripts")
            .join("addons")
            .join(ADDON_FILE_NAME);

        match sync_target_file(&addon_target) {
            Ok(was_updated) => {
                if was_updated {
                    report.updated_targets.push(addon_target);
                } else {
                    report.unchanged_targets.push(addon_target);
                }
            }
            Err(err) => {
                report
                    .failed_targets
                    .push((addon_target, err.to_string()));
            }
        }
    }

    Ok(report)
}

fn discover_version_dirs(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if is_blender_version_folder(name) {
            dirs.push(path);
        }
    }
    Ok(dirs)
}

fn is_blender_version_folder(name: &str) -> bool {
    let parts: Vec<&str> = name.split('.').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return false;
    }

    parts
        .iter()
        .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

fn parse_version_tuple(name: &str) -> (u32, u32, u32) {
    let mut parts = name
        .split('.')
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect::<Vec<u32>>();
    while parts.len() < 3 {
        parts.push(0);
    }
    (parts[0], parts[1], parts[2])
}

fn sync_target_file(target: &Path) -> io::Result<bool> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    if let Ok(existing) = fs::read(target) {
        if existing == ADDON_FILE_BYTES {
            return Ok(false);
        }
    }

    fs::write(target, ADDON_FILE_BYTES)?;
    Ok(true)
}

fn blender_config_root() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("Blender Foundation").join("Blender"));
    }

    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join("Library").join("Application Support").join("Blender"));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
            return Some(PathBuf::from(path).join("blender"));
        }

        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join(".config").join("blender"));
    }

    #[allow(unreachable_code)]
    None
}
