use super::types::{SETUP_ITEM_HOST_BLENDER, SETUP_ITEM_HOST_GIMP, SETUP_ITEM_HOST_LIBREOFFICE};
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
                &["gimp-2.10.exe", "gimp.exe"]
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
        program_files_candidates()
            .into_iter()
            .flat_map(|root| {
                [
                    root.join("GIMP 2").join("bin").join("gimp-2.10.exe"),
                    root.join("GIMP 3").join("bin").join("gimp-3.exe"),
                ]
            })
            .collect()
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
        program_files_candidates()
            .into_iter()
            .flat_map(|root| {
                [
                    root.join("Blender Foundation")
                        .join("Blender 4.2")
                        .join("blender.exe"),
                    root.join("Blender Foundation")
                        .join("Blender 4.1")
                        .join("blender.exe"),
                    root.join("Blender Foundation")
                        .join("Blender 4.0")
                        .join("blender.exe"),
                ]
            })
            .collect()
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
    for executable in spec.executable_names {
        let key = format!(r"HKLM\Software\Microsoft\Windows\CurrentVersion\App Paths\{executable}");
        let output = std::process::Command::new("reg")
            .args(["query", &key, "/ve"])
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
    use std::path::PathBuf;
    use tempfile::TempDir;

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
}
