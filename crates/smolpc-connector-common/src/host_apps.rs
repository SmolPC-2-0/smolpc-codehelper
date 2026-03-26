use crate::{SETUP_ITEM_HOST_BLENDER, SETUP_ITEM_HOST_GIMP, SETUP_ITEM_HOST_LIBREOFFICE};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostAppDetection {
    pub id: &'static str,
    pub label: &'static str,
    pub path: Option<PathBuf>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug)]
struct HostAppSpec {
    id: &'static str,
    label: &'static str,
    executable_names: &'static [&'static str],
    standard_paths: Vec<PathBuf>,
}

#[allow(dead_code)]
pub fn detect_all(cached: &HashMap<String, PathBuf>) -> Vec<HostAppDetection> {
    detect_all_with_policy(cached, true)
}

pub fn detect_all_with_policy(
    cached: &HashMap<String, PathBuf>,
    allow_system_lookup: bool,
) -> Vec<HostAppDetection> {
    all_specs()
        .into_iter()
        .map(|spec| detect_host_app_with_policy(&spec, cached.get(spec.id), allow_system_lookup))
        .collect()
}

fn detect_host_app(spec: &HostAppSpec, cached: Option<&PathBuf>) -> HostAppDetection {
    detect_host_app_with_policy(spec, cached, true)
}

fn detect_host_app_with_policy(
    spec: &HostAppSpec,
    cached: Option<&PathBuf>,
    allow_system_lookup: bool,
) -> HostAppDetection {
    let resolved = cached.filter(|path| path.exists()).cloned().or_else(|| {
        if allow_system_lookup {
            lookup_windows_app_paths(spec)
                .or_else(|| lookup_standard_paths(spec))
                .or_else(|| lookup_on_path(spec))
        } else {
            None
        }
    });

    let detail = resolved
        .as_ref()
        .map(|path| format!("{} detected at {}", spec.label, path.to_string_lossy()));

    HostAppDetection {
        id: spec.id,
        label: spec.label,
        path: resolved,
        detail,
    }
}

fn all_specs() -> Vec<HostAppSpec> {
    vec![
        HostAppSpec {
            id: SETUP_ITEM_HOST_GIMP,
            label: "GIMP",
            executable_names: if cfg!(windows) {
                &["gimp-3.exe", "gimp-2.10.exe", "gimp.exe"]
            } else {
                &["gimp", "gimp-2.10"]
            },
            standard_paths: standard_paths_for_gimp(),
        },
        HostAppSpec {
            id: SETUP_ITEM_HOST_BLENDER,
            label: "Blender",
            executable_names: if cfg!(windows) {
                &["blender.exe"]
            } else {
                &["blender"]
            },
            standard_paths: standard_paths_for_blender(),
        },
        libreoffice_spec(),
    ]
}

fn libreoffice_spec() -> HostAppSpec {
    HostAppSpec {
        id: SETUP_ITEM_HOST_LIBREOFFICE,
        label: "LibreOffice",
        executable_names: if cfg!(windows) {
            &["soffice.exe"]
        } else {
            &["soffice"]
        },
        standard_paths: standard_paths_for_libreoffice(),
    }
}

fn program_files_candidates() -> Vec<PathBuf> {
    ["ProgramFiles", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(env::var_os)
        .map(PathBuf::from)
        .collect()
}

fn standard_paths_for_gimp() -> Vec<PathBuf> {
    if cfg!(windows) {
        let mut paths = Vec::new();

        // Per-user installs first (GIMP NSIS installer defaults to %LOCALAPPDATA%\Programs)
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            let local = PathBuf::from(local_app_data).join("Programs");
            paths.push(local.join("GIMP 3").join("bin").join("gimp-3.exe"));
        }

        // System-wide installs
        for root in program_files_candidates() {
            paths.push(root.join("GIMP 3").join("bin").join("gimp-3.exe"));
        }

        // GIMP 2.x fallback (lowest priority)
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            let local = PathBuf::from(local_app_data).join("Programs");
            paths.push(local.join("GIMP 2").join("bin").join("gimp-2.10.exe"));
        }
        for root in program_files_candidates() {
            paths.push(root.join("GIMP 2").join("bin").join("gimp-2.10.exe"));
        }

        paths
    } else if cfg!(target_os = "macos") {
        vec![
            PathBuf::from("/Applications/GIMP.app/Contents/MacOS/gimp"),
            PathBuf::from("/opt/homebrew/bin/gimp"),
        ]
    } else {
        vec![
            PathBuf::from("/usr/bin/gimp"),
            PathBuf::from("/usr/local/bin/gimp"),
        ]
    }
}

fn standard_paths_for_blender() -> Vec<PathBuf> {
    if cfg!(windows) {
        let mut paths = Vec::new();
        for root in program_files_candidates() {
            let foundation = root.join("Blender Foundation");
            if let Ok(entries) = std::fs::read_dir(&foundation) {
                // Enumerate all "Blender *" directories, prefer newest version
                let mut candidates: Vec<(PathBuf, String)> = entries
                    .filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        if name.starts_with("Blender ") {
                            Some((e.path().join("blender.exe"), name))
                        } else {
                            None
                        }
                    })
                    .collect();
                // Sort by version number (numeric, not lexicographic)
                candidates.sort_by(|(_, a), (_, b)| {
                    let parse_ver = |s: &str| -> (u32, u32) {
                        let v = s.trim_start_matches("Blender ");
                        let mut parts = v.split('.');
                        let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
                        let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
                        (major, minor)
                    };
                    parse_ver(b).cmp(&parse_ver(a)) // descending
                });
                paths.extend(candidates.into_iter().map(|(path, _)| path));
            }
        }
        paths
    } else if cfg!(target_os = "macos") {
        vec![
            PathBuf::from("/Applications/Blender.app/Contents/MacOS/Blender"),
            PathBuf::from("/opt/homebrew/bin/blender"),
        ]
    } else {
        vec![
            PathBuf::from("/usr/bin/blender"),
            PathBuf::from("/usr/local/bin/blender"),
        ]
    }
}

fn standard_paths_for_libreoffice() -> Vec<PathBuf> {
    if cfg!(windows) {
        program_files_candidates()
            .into_iter()
            .flat_map(|root| {
                [
                    root.join("LibreOffice").join("program").join("soffice.exe"),
                    root.join("Collabora Office")
                        .join("program")
                        .join("soffice.exe"),
                ]
            })
            .collect()
    } else if cfg!(target_os = "macos") {
        vec![
            PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS/soffice"),
            PathBuf::from("/Applications/Collabora Office.app/Contents/MacOS/soffice"),
        ]
    } else {
        vec![
            PathBuf::from("/usr/bin/soffice"),
            PathBuf::from("/usr/local/bin/soffice"),
            PathBuf::from("/snap/bin/libreoffice"),
        ]
    }
}

fn lookup_standard_paths(spec: &HostAppSpec) -> Option<PathBuf> {
    spec.standard_paths
        .iter()
        .find(|path| path.exists())
        .cloned()
}

fn lookup_on_path(spec: &HostAppSpec) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    env::split_paths(&path_var)
        .flat_map(|dir| {
            spec.executable_names
                .iter()
                .map(move |name| dir.join(name))
                .collect::<Vec<_>>()
        })
        .find(|candidate| candidate.exists())
}

#[cfg(windows)]
fn lookup_windows_app_paths(spec: &HostAppSpec) -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    for executable in spec.executable_names {
        let key = format!(r"HKLM\Software\Microsoft\Windows\CurrentVersion\App Paths\{executable}");
        let output = std::process::Command::new("reg")
            .args(["query", &key, "/ve"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if !output.status.success() {
            continue;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(path) = parse_reg_query_path(&stdout) {
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(windows)]
fn parse_reg_query_path(stdout: &str) -> Option<PathBuf> {
    stdout
        .lines()
        .find_map(|line| line.split("REG_SZ").nth(1))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(not(windows))]
fn lookup_windows_app_paths(_spec: &HostAppSpec) -> Option<PathBuf> {
    None
}

pub fn detect_libreoffice(cached: Option<&Path>) -> HostAppDetection {
    let cached = cached.map(Path::to_path_buf);
    detect_host_app(&libreoffice_spec(), cached.as_ref())
}

pub fn detect_blender(cached: Option<&Path>) -> HostAppDetection {
    detect_blender_with_policy(cached, true)
}

pub fn detect_gimp(cached: Option<&Path>) -> HostAppDetection {
    detect_gimp_with_policy(cached, true)
}

pub fn detect_gimp_with_policy(
    cached: Option<&Path>,
    allow_system_lookup: bool,
) -> HostAppDetection {
    let cached = cached.map(Path::to_path_buf);
    let Some(spec) = all_specs()
        .into_iter()
        .find(|candidate| candidate.id == SETUP_ITEM_HOST_GIMP)
    else {
        return HostAppDetection {
            id: SETUP_ITEM_HOST_GIMP,
            label: "GIMP",
            path: None,
            detail: Some("GIMP host detection is unavailable in this build.".to_string()),
        };
    };
    detect_host_app_with_policy(&spec, cached.as_ref(), allow_system_lookup)
}

pub fn detect_blender_with_policy(
    cached: Option<&Path>,
    allow_system_lookup: bool,
) -> HostAppDetection {
    let cached = cached.map(Path::to_path_buf);
    let spec = all_specs()
        .into_iter()
        .find(|candidate| candidate.id == SETUP_ITEM_HOST_BLENDER)
        .expect("blender spec");
    detect_host_app_with_policy(&spec, cached.as_ref(), allow_system_lookup)
}

#[allow(dead_code)]
fn _path_exists(path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::{detect_host_app, HostAppSpec};
    use std::env;
    use std::path::PathBuf;
    use std::sync::{Mutex as StdMutex, OnceLock};
    use tempfile::TempDir;

    static PATH_LOCK: OnceLock<StdMutex<()>> = OnceLock::new();

    fn path_lock() -> &'static StdMutex<()> {
        PATH_LOCK.get_or_init(|| StdMutex::new(()))
    }

    fn with_path(path: &std::path::Path, callback: impl FnOnce()) {
        let _guard = path_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let original = env::var_os("PATH");
        env::set_var("PATH", path.as_os_str());
        callback();
        match original {
            Some(value) => env::set_var("PATH", value),
            None => env::remove_var("PATH"),
        }
    }

    #[test]
    fn cached_path_wins_when_it_still_exists() {
        let temp = TempDir::new().expect("temp dir");
        let executable = temp.path().join("blender.exe");
        std::fs::write(&executable, "bin").expect("write executable");

        let spec = HostAppSpec {
            id: "host_blender",
            label: "Blender",
            executable_names: &["blender.exe"],
            standard_paths: vec![],
        };

        let detection = detect_host_app(&spec, Some(&executable));
        assert_eq!(detection.path, Some(executable));
    }

    #[test]
    fn standard_path_is_used_when_cached_path_is_missing() {
        let temp = TempDir::new().expect("temp dir");
        let executable = temp.path().join("gimp.exe");
        std::fs::write(&executable, "bin").expect("write executable");

        let spec = HostAppSpec {
            id: "host_gimp",
            label: "GIMP",
            executable_names: &["gimp.exe"],
            standard_paths: vec![executable.clone()],
        };

        let detection = detect_host_app(&spec, Some(&PathBuf::from("/missing/gimp.exe")));
        assert_eq!(detection.path, Some(executable));
    }

    #[test]
    fn path_lookup_accepts_gimp_3_executable_name() {
        let temp = TempDir::new().expect("temp dir");
        let executable = temp.path().join("gimp-3.exe");
        std::fs::write(&executable, "bin").expect("write executable");

        let spec = HostAppSpec {
            id: "host_gimp",
            label: "GIMP",
            executable_names: &["gimp-3.exe", "gimp.exe"],
            standard_paths: vec![],
        };

        with_path(temp.path(), || {
            let detection = detect_host_app(&spec, Some(&PathBuf::from("/missing/gimp.exe")));
            assert_eq!(detection.path, Some(executable.clone()));
        });
    }
}
