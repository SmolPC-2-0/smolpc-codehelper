use std::path::{Path, PathBuf};
use std::process::Command;
use sysinfo::{ProcessesToUpdate, System};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlenderLaunchOutcome {
    AlreadyRunning,
    Launched,
}

pub fn setup_launch_detail() -> &'static str {
    "Host-app launch remains mode-driven. Blender may auto-launch on first use once its addon is provisioned."
}

pub fn is_matching_blender_process_running(blender_path: &Path) -> bool {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system
        .processes()
        .values()
        .any(|process| executable_matches(process.exe(), blender_path))
}

pub fn launch_blender_if_needed(blender_path: &Path) -> Result<BlenderLaunchOutcome, String> {
    maybe_launch_blender_with(
        blender_path,
        is_matching_blender_process_running(blender_path),
        |path| {
            Command::new(path)
                .spawn()
                .map(|_| ())
                .map_err(|error| format!("Failed to launch Blender at {}: {error}", path.display()))
        },
    )
}

fn maybe_launch_blender_with<F>(
    blender_path: &Path,
    already_running: bool,
    launch: F,
) -> Result<BlenderLaunchOutcome, String>
where
    F: FnOnce(&Path) -> Result<(), String>,
{
    if already_running {
        return Ok(BlenderLaunchOutcome::AlreadyRunning);
    }

    launch(blender_path)?;
    Ok(BlenderLaunchOutcome::Launched)
}

fn executable_matches(candidate: Option<&Path>, target: &Path) -> bool {
    let Some(candidate) = candidate else {
        return false;
    };

    if path_identity(candidate) == path_identity(target) {
        return true;
    }

    candidate.file_name() == target.file_name()
}

fn path_identity(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::{executable_matches, maybe_launch_blender_with, BlenderLaunchOutcome};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn maybe_launch_blender_with_skips_spawn_when_process_is_running() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);

        let outcome = maybe_launch_blender_with(Path::new("/fake/blender"), true, move |_| {
            called_clone.store(true, Ordering::SeqCst);
            Ok(())
        })
        .expect("launch outcome");

        assert_eq!(outcome, BlenderLaunchOutcome::AlreadyRunning);
        assert!(!called.load(Ordering::SeqCst));
    }

    #[test]
    fn maybe_launch_blender_with_spawns_when_process_is_absent() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);

        let outcome = maybe_launch_blender_with(Path::new("/fake/blender"), false, move |_| {
            called_clone.store(true, Ordering::SeqCst);
            Ok(())
        })
        .expect("launch outcome");

        assert_eq!(outcome, BlenderLaunchOutcome::Launched);
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn executable_matches_accepts_exact_target_path() {
        let target = PathBuf::from("/tmp/fake-blender");
        assert!(executable_matches(Some(target.as_path()), target.as_path()));
    }
}
