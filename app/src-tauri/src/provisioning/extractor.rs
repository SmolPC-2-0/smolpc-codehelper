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
    // Retry a few times with a delay — antivirus can briefly lock files
    // after a crash, making the first removal attempt fail.
    if temp_dir.exists() {
        let mut last_err = None;
        for attempt in 0..3 {
            match fs::remove_dir_all(&temp_dir) {
                Ok(()) => {
                    last_err = None;
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    if attempt < 2 {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                }
            }
        }
        if let Some(e) = last_err {
            return Err(format!(
                "cannot remove stale temp dir after retries (antivirus may be locking files): {e}"
            ));
        }
    }

    fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("cannot create temp dir: {e}"))?;

    let file = fs::File::open(archive_path)
        .map_err(|e| format!("cannot open archive: {e}"))?;

    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("invalid ZIP archive: {e}"))?;

    // Detect a common top-level directory prefix in the archive to strip it.
    // Many model archives contain `{backend}/file.bin` internally, but the
    // caller already includes the backend in `target_dir`, which causes path
    // doubling (e.g. `models/id/backend/backend/file.bin`).
    let strip_prefix = detect_common_prefix(&mut archive);

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
            Some(p) => p.to_owned(),
            None => continue,
        };

        // Strip the common top-level directory if present.
        let rel_path = if let Some(prefix) = &strip_prefix {
            match rel_path.strip_prefix(prefix) {
                Ok(stripped) => stripped.to_owned(),
                Err(_) => rel_path,
            }
        } else {
            rel_path
        };

        // Skip empty paths (the prefix directory entry itself).
        if rel_path.as_os_str().is_empty() {
            continue;
        }

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

            match io::copy(&mut entry, &mut out_file) {
                Ok(_) => {}
                Err(e) => {
                    // Check if this is a disk-full error. On Windows,
                    // ERROR_DISK_FULL (112) or ERROR_HANDLE_DISK_FULL (39)
                    // can occur when another app consumed space mid-extraction.
                    let is_disk_full = e.raw_os_error() == Some(112)
                        || e.raw_os_error() == Some(39)
                        || e.kind() == io::ErrorKind::StorageFull;
                    if is_disk_full {
                        let _ = fs::remove_dir_all(&temp_dir);
                        return Err(format!(
                            "disk full during extraction of {} — another application \
                             may have consumed disk space. Free up space and try again.",
                            rel_path.display()
                        ));
                    }
                    return Err(format!(
                        "IO error extracting {}: {e}",
                        rel_path.display()
                    ));
                }
            }

            bytes_written += out_path
                .metadata()
                .map(|m| m.len())
                .unwrap_or(0);

            on_progress(bytes_written.min(total_bytes), total_bytes);
        }
    }

    // Move temp → target. Use rename first (fast, atomic on same volume),
    // falling back to recursive copy + delete for cross-drive moves.
    move_dir(&temp_dir, target_dir)?;

    Ok(target_dir.to_owned())
}

/// If every entry in the archive shares a single top-level directory, return it
/// so we can strip it during extraction. Returns `None` if entries are at the
/// root or if there are multiple top-level directories.
fn detect_common_prefix(archive: &mut zip::ZipArchive<fs::File>) -> Option<PathBuf> {
    let mut common: Option<PathBuf> = None;
    for i in 0..archive.len() {
        let entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => return None,
        };
        let name = match entry.enclosed_name() {
            Some(p) => p.to_owned(),
            None => continue,
        };
        let mut components = name.components();
        let first = match components.next() {
            Some(c) => PathBuf::from(c.as_os_str()),
            None => continue,
        };
        // Only consider it a prefix if the entry has more components (i.e. it
        // is inside a directory, not a root-level file).
        if components.next().is_none() && !entry.is_dir() {
            // Root-level file — no common prefix.
            return None;
        }
        match &common {
            Some(existing) if existing != &first => return None,
            None => common = Some(first),
            _ => {}
        }
    }
    common
}

/// Move `src` directory to `dst`. Tries `fs::rename` first (atomic, same-volume),
/// then falls back to recursive copy + delete for cross-device moves.
fn move_dir(src: &Path, dst: &Path) -> Result<(), String> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) => {
            // On Windows, cross-drive rename fails with ERROR_NOT_SAME_DEVICE.
            // Fall back to recursive copy.
            let is_cross_device = e.raw_os_error() == Some(17) // POSIX EXDEV
                || e.raw_os_error() == Some(0x11) // Windows ERROR_NOT_SAME_DEVICE
                || e.kind() == io::ErrorKind::Other;

            if !is_cross_device {
                return Err(format!("cannot rename temp dir to target: {e}"));
            }

            copy_dir_recursive(src, dst)?;
            fs::remove_dir_all(src)
                .map_err(|e| format!("cannot clean up temp dir after copy: {e}"))?;
            Ok(())
        }
    }
}

/// Recursively copy all files and directories from `src` to `dst`.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst)
        .map_err(|e| format!("cannot create target dir {}: {e}", dst.display()))?;

    for entry in fs::read_dir(src)
        .map_err(|e| format!("cannot read dir {}: {e}", src.display()))?
    {
        let entry = entry.map_err(|e| format!("cannot read dir entry: {e}"))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| {
                format!(
                    "cannot copy {} -> {}: {e}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
    }
    Ok(())
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

    /// Create a flat ZIP (no common prefix directory).
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

    /// Create a ZIP with all entries under a common prefix directory.
    fn create_prefixed_zip(dir: &Path, prefix: &str) -> PathBuf {
        let zip_path = dir.join("prefixed.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file(format!("{prefix}/model.bin"), options)
            .unwrap();
        writer.write_all(b"model data").unwrap();
        writer
            .start_file(format!("{prefix}/config.json"), options)
            .unwrap();
        writer.write_all(b"{}").unwrap();
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
    fn test_extract_zip_strips_common_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = create_prefixed_zip(tmp.path(), "openvino_npu");
        let target = tmp.path().join("output");

        let cancel = Arc::new(AtomicBool::new(false));
        let result = extract_zip(&zip_path, &target, cancel, Box::new(|_, _| {}));
        assert!(result.is_ok(), "extraction failed: {:?}", result);

        // Files should be directly under target, NOT under target/openvino_npu/
        assert!(
            target.join("model.bin").exists(),
            "model.bin should be at target root after prefix strip"
        );
        assert!(
            target.join("config.json").exists(),
            "config.json should be at target root after prefix strip"
        );
        assert!(
            !target.join("openvino_npu").exists(),
            "prefix dir should NOT exist — it should have been stripped"
        );
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

    #[test]
    fn test_copy_dir_recursive() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");

        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), "aaa").unwrap();
        std::fs::write(src.join("sub/b.txt"), "bbb").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "aaa");
        assert_eq!(
            std::fs::read_to_string(dst.join("sub/b.txt")).unwrap(),
            "bbb"
        );
    }
}
