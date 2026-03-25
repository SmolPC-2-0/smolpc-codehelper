use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};

pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send>;

/// Extract a ZIP archive to `target_dir`.
///
/// Extraction is performed into a sibling temp directory first
/// (`target_dir` with the extension replaced by `"extracting"`), then
/// atomically renamed to `target_dir` on success.
///
/// * `archive_path`  — path to the `.zip` file
/// * `target_dir`    — final extraction destination (must not already exist)
/// * `cancel`        — checked before each entry; if set the temp dir is
///                     removed and an error is returned
/// * `on_progress`   — `(bytes_written, total_bytes)` where `total_bytes`
///                     is the size of the archive file on disk
///
/// Returns the resolved `target_dir` on success.
pub fn extract_zip(
    archive_path: &Path,
    target_dir: &Path,
    cancel: Arc<AtomicBool>,
    on_progress: ProgressCallback,
) -> Result<PathBuf, String> {
    // Total bytes = archive file size (used as the progress denominator so the
    // caller can correlate with download progress).
    let total_bytes = archive_path
        .metadata()
        .map_err(|e| format!("cannot stat archive: {e}"))?
        .len();

    let temp_dir = target_dir.with_extension("extracting");

    // Clean up any leftover temp dir from a previous aborted attempt.
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)
            .map_err(|e| format!("cannot remove stale temp dir: {e}"))?;
    }

    fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("cannot create temp dir: {e}"))?;

    let file = fs::File::open(archive_path)
        .map_err(|e| format!("cannot open archive: {e}"))?;

    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("invalid ZIP archive: {e}"))?;

    let mut bytes_written: u64 = 0;

    for i in 0..archive.len() {
        // Check cancellation before each entry.
        if cancel.load(Ordering::Relaxed) {
            let _ = fs::remove_dir_all(&temp_dir);
            return Err("extraction cancelled".to_string());
        }

        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("cannot read ZIP entry {i}: {e}"))?;

        // enclosed_name() rejects path-traversal attempts (zip-slip).
        let rel_path = match entry.enclosed_name() {
            Some(p) => p,
            None => continue,
        };

        let out_path = temp_dir.join(&rel_path);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|e| format!("cannot create directory {}: {e}", out_path.display()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!("cannot create parent dir {}: {e}", parent.display())
                })?;
            }

            let mut out_file = fs::File::create(&out_path)
                .map_err(|e| format!("cannot create file {}: {e}", out_path.display()))?;

            io::copy(&mut entry, &mut out_file)
                .map_err(|e| format!("IO error extracting {}: {e}", rel_path.display()))?;

            bytes_written += out_path
                .metadata()
                .map(|m| m.len())
                .unwrap_or(0);

            on_progress(bytes_written.min(total_bytes), total_bytes);
        }
    }

    // Atomic rename: temp → target.
    fs::rename(&temp_dir, target_dir)
        .map_err(|e| format!("cannot rename temp dir to target: {e}"))?;

    Ok(target_dir.to_owned())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    fn create_test_zip(dir: &Path) -> PathBuf {
        let zip_path = dir.join("test.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("hello.txt", options).unwrap();
        writer.write_all(b"hello world").unwrap();
        writer.start_file("subdir/nested.txt", options).unwrap();
        writer.write_all(b"nested content").unwrap();
        writer.finish().unwrap();
        zip_path
    }

    #[test]
    fn test_extract_zip_creates_files() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = create_test_zip(tmp.path());
        let target = tmp.path().join("output");

        let cancel = Arc::new(AtomicBool::new(false));
        let progress_called = Arc::new(AtomicBool::new(false));
        let progress_flag = Arc::clone(&progress_called);

        let on_progress: ProgressCallback = Box::new(move |_bytes, _total| {
            progress_flag.store(true, Ordering::Relaxed);
        });

        let result = extract_zip(&zip_path, &target, cancel, on_progress);
        assert!(result.is_ok(), "extraction failed: {:?}", result);

        let out = result.unwrap();
        assert_eq!(out, target);

        // Files must exist with correct content.
        let hello = std::fs::read_to_string(target.join("hello.txt")).unwrap();
        assert_eq!(hello, "hello world");

        let nested = std::fs::read_to_string(target.join("subdir/nested.txt")).unwrap();
        assert_eq!(nested, "nested content");

        // Progress callback must have been invoked at least once.
        assert!(
            progress_called.load(Ordering::Relaxed),
            "progress callback was never called"
        );

        // Temp dir must be gone.
        assert!(!target.with_extension("extracting").exists());
    }

    #[test]
    fn test_extract_zip_cancel() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = create_test_zip(tmp.path());
        let target = tmp.path().join("output");

        // Set cancel flag to true before extraction begins.
        let cancel = Arc::new(AtomicBool::new(true));
        let on_progress: ProgressCallback = Box::new(|_, _| {});

        let result = extract_zip(&zip_path, &target, cancel, on_progress);

        assert!(result.is_err(), "expected error when cancelled");
        let err = result.unwrap_err();
        assert!(
            err.contains("cancelled"),
            "error message should contain 'cancelled', got: {err}"
        );

        // Temp dir must be cleaned up.
        assert!(
            !target.with_extension("extracting").exists(),
            "temp dir should be cleaned up after cancellation"
        );
    }
}
