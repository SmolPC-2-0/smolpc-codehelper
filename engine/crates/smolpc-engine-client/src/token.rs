use rand::distributions::{Alphanumeric, DistString};
use rand::rngs::OsRng;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::Path;

use crate::EngineClientError;

pub(crate) fn load_or_create_token(path: &Path) -> Result<String, EngineClientError> {
    if let Some(token) = read_non_empty_token(path)? {
        return Ok(token);
    }

    for _ in 0..3 {
        let token = Alphanumeric.sample_string(&mut OsRng, 48);

        match create_new_token_file(path, &token) {
            Ok(()) => return Ok(token),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                if let Some(existing) = read_non_empty_token(path)? {
                    return Ok(existing);
                }
                let _ = std::fs::remove_file(path);
            }
            Err(error) => return Err(error.into()),
        }
    }

    Err(EngineClientError::Message(
        "Failed to initialize engine token file after retrying".to_string(),
    ))
}

fn read_non_empty_token(path: &Path) -> Result<Option<String>, EngineClientError> {
    match std::fs::read_to_string(path) {
        Ok(token) => {
            let trimmed = token.trim().to_string();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed))
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn create_new_token_file(path: &Path, token: &str) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(token.as_bytes())?;
        return Ok(());
    }

    #[cfg(not(unix))]
    {
        // TODO: Harden Windows ACLs for engine-token.txt so only the current user can read it.
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(token.as_bytes())?;
        Ok(())
    }
}
