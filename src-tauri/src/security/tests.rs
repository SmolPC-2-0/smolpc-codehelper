use super::*;
use std::fs;

#[cfg(test)]
mod content_size_tests {
    use super::*;

    #[test]
    fn test_valid_content() {
        assert!(validate_content_size("hello world").is_ok());
    }

    #[test]
    fn test_empty_content() {
        assert!(validate_content_size("").is_ok());
    }

    #[test]
    fn test_content_at_limit() {
        let at_limit = "a".repeat(MAX_FILE_SIZE as usize);
        assert!(validate_content_size(&at_limit).is_ok());
    }

    #[test]
    fn test_content_over_limit() {
        let over_limit = "a".repeat((MAX_FILE_SIZE + 1) as usize);
        let result = validate_content_size(&over_limit);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too large"));
    }

    #[test]
    fn test_large_content_error_message() {
        let large = "x".repeat(20 * 1024 * 1024); // 20 MB
        let result = validate_content_size(&large);
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("MB"));
        assert!(err_msg.contains("max"));
    }
}


#[tokio::test]
async fn test_file_size_validation() {
    use tempfile::NamedTempFile;

    // Create a small temporary file
    let small_file = NamedTempFile::new().unwrap();
    fs::write(small_file.path(), "small content").unwrap();

    let result = validate_file_size(small_file.path()).await;
    assert!(result.is_ok());

    // Create a large temporary file (over 10 MB)
    let large_file = NamedTempFile::new().unwrap();
    let large_content = vec![0u8; (MAX_FILE_SIZE + 1024) as usize];
    fs::write(large_file.path(), &large_content).unwrap();

    let result = validate_file_size(large_file.path()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("too large"));
}

#[tokio::test]
async fn test_file_size_validation_error_message() {
    use tempfile::NamedTempFile;

    // Create a file that's 15 MB
    let file = NamedTempFile::new().unwrap();
    let content = vec![0u8; 15 * 1024 * 1024];
    fs::write(file.path(), &content).unwrap();

    let result = validate_file_size(file.path()).await;
    assert!(result.is_err());

    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("15.00 MB") || err_msg.contains("15 MB"));
    assert!(err_msg.contains("max"));
    assert!(err_msg.contains("10 MB"));
}

// Integration tests for path validation would require a running Tauri app.
// For manual testing:
// 1. Run the app in dev mode: npm run tauri dev
// 2. Try to call read() with paths like:
//    - "../../etc/passwd" (should fail)
//    - "C:\\Windows\\System32\\config\\SAM" (should fail on Windows)
//    - Valid paths in app data directory (should succeed)
