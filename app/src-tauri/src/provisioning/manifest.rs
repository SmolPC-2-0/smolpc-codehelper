use std::io::Read;
use std::path::Path;

use hex;
use sha2::{Digest, Sha256};

use crate::provisioning::types::ModelArchivesManifest;

/// Parse a `model-archives.json` manifest file.
/// Returns an error if the file cannot be read, is not valid JSON, or has `version != 1`.
pub fn parse_manifest(path: &Path) -> Result<ModelArchivesManifest, String> {
    let data = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read manifest: {e}"))?;

    let manifest: ModelArchivesManifest = serde_json::from_str(&data)
        .map_err(|e| format!("Failed to parse manifest JSON: {e}"))?;

    if manifest.version != 1 {
        return Err(format!(
            "Unsupported manifest version: {}",
            manifest.version
        ));
    }

    Ok(manifest)
}

/// Read `path` in 8 KB chunks and verify its SHA-256 digest matches `expected_hex`.
/// Returns `Ok(true)` on match, `Ok(false)` on mismatch, `Err` on I/O failure.
pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<bool, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open file for verification: {e}"))?;

    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];

    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("Failed to read file during verification: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    let digest = hasher.finalize();
    let actual_hex = hex::encode(digest);

    Ok(actual_hex.eq_ignore_ascii_case(expected_hex))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_valid_manifest() {
        let json = r#"{
            "version": 1,
            "models": [
                {
                    "id": "qwen2.5-1.5b-instruct",
                    "backend": "openvino_npu",
                    "archive_name": "qwen2.5-1.5b-instruct-int4-ov.zip",
                    "archive_path": "models/qwen2.5-1.5b-instruct-int4-ov.zip",
                    "sha256": "abc123"
                }
            ]
        }"#;

        let mut tmp = NamedTempFile::new().expect("create temp file");
        tmp.write_all(json.as_bytes()).expect("write json");

        let manifest = parse_manifest(tmp.path()).expect("parse should succeed");

        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.models.len(), 1);
        assert_eq!(manifest.models[0].id, "qwen2.5-1.5b-instruct");
        assert_eq!(manifest.models[0].backend, "openvino_npu");
        assert_eq!(
            manifest.models[0].archive_name,
            "qwen2.5-1.5b-instruct-int4-ov.zip"
        );
        assert_eq!(manifest.models[0].sha256, "abc123");
    }

    #[test]
    fn test_reject_unsupported_version() {
        let json = r#"{"version": 99, "models": []}"#;

        let mut tmp = NamedTempFile::new().expect("create temp file");
        tmp.write_all(json.as_bytes()).expect("write json");

        let err = parse_manifest(tmp.path()).expect_err("should fail for version 99");
        assert!(
            err.contains("Unsupported manifest version"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn test_verify_sha256_correct() {
        // SHA-256("hello world") — standard known value
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";

        let mut tmp = NamedTempFile::new().expect("create temp file");
        tmp.write_all(b"hello world").expect("write content");

        let result = verify_sha256(tmp.path(), expected).expect("io should succeed");
        assert!(result, "SHA-256 should match for 'hello world'");
    }

    #[test]
    fn test_verify_sha256_mismatch() {
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let mut tmp = NamedTempFile::new().expect("create temp file");
        tmp.write_all(b"hello world").expect("write content");

        let result = verify_sha256(tmp.path(), wrong_hash).expect("io should succeed");
        assert!(!result, "SHA-256 should not match with wrong hash");
    }
}
