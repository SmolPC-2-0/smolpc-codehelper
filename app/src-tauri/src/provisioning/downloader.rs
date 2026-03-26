use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub type AsyncProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

/// Downloads a file with resume support via HTTP Range requests.
///
/// Downloads to `dest_path.with_extension("partial")` first. If a partial
/// file exists, resumes from where it left off. On completion, renames the
/// partial file to `dest_path`.
///
/// If `cancel` is set to `true` during download, the partial file is kept
/// so the download can be resumed in a future call.
pub async fn download_file(
    client: &reqwest::Client,
    url: &str,
    dest_path: &Path,
    cancel: Arc<AtomicBool>,
    on_progress: AsyncProgressCallback,
) -> Result<PathBuf, String> {
    let partial_path = dest_path.with_extension("partial");

    // Check existing partial file size for resume
    let existing_bytes = match tokio::fs::metadata(&partial_path).await {
        Ok(meta) => meta.len(),
        Err(_) => 0,
    };

    // Build request, adding Range header if we have a partial file
    let mut request = client.get(url);
    if existing_bytes > 0 {
        request = request.header("Range", format!("bytes={}-", existing_bytes));
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    let status = response.status().as_u16();

    if status != 200 && status != 206 {
        return Err(format!("Unexpected HTTP status: {status}"));
    }

    // Determine total size for progress reporting.
    // For 206 responses, Content-Range contains the full file size.
    // For 200 responses, Content-Length is the full file size.
    let total_bytes: u64 = if status == 206 {
        // Content-Range: bytes <start>-<end>/<total>
        response
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split('/').last())
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0)
    } else {
        response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0)
    };

    // Open the partial file for writing
    let mut opts = tokio::fs::OpenOptions::new();
    opts.create(true).write(true);
    if existing_bytes > 0 && status == 206 {
        opts.append(true);
    } else {
        opts.truncate(true);
    }

    let mut file = opts
        .open(&partial_path)
        .await
        .map_err(|e| format!("Failed to open partial file: {e}"))?;

    let mut bytes_downloaded = if status == 206 { existing_bytes } else { 0u64 };

    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            // Keep partial file for future resume
            return Err("Download cancelled".to_string());
        }

        let chunk = chunk_result.map_err(|e| format!("Stream error: {e}"))?;

        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("Write error: {e}"))?;

        bytes_downloaded += chunk.len() as u64;
        on_progress(bytes_downloaded, total_bytes);
    }

    // Flush and close the file before renaming
    tokio::io::AsyncWriteExt::flush(&mut file)
        .await
        .map_err(|e| format!("Flush error: {e}"))?;
    drop(file);

    // Rename partial file to final destination
    tokio::fs::rename(&partial_path, dest_path)
        .await
        .map_err(|e| format!("Failed to rename partial file to destination: {e}"))?;

    Ok(dest_path.to_path_buf())
}

/// Checks internet connectivity by sending a HEAD request to the given URL.
///
/// Returns `true` if the server responds with a successful status code,
/// `false` on any error (network failure, timeout, non-success status).
pub async fn check_internet(client: &reqwest::Client, url: &str) -> bool {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.head(url).send(),
    )
    .await;

    match result {
        Ok(Ok(response)) => response.status().is_success(),
        _ => false,
    }
}
