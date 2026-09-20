//! OS keychain wrapper for sensitive credentials
use crate::error::{AppError, Result};

const SERVICE: &str = "backseat";

pub fn store(key: &str, value: &str) -> Result<()> {
    keyring::Entry::new(SERVICE, key)
        .map_err(|e| AppError::Config(e.to_string()))?
        .set_password(value)
        .map_err(|e| AppError::Config(e.to_string()))
}

pub fn retrieve(key: &str) -> Result<String> {
    keyring::Entry::new(SERVICE, key)
        .map_err(|e| AppError::Config(e.to_string()))?
        .get_password()
        .map_err(|e| AppError::Config(e.to_string()))
}

pub fn remove(key: &str) {
    let _ = keyring::Entry::new(SERVICE, key).map(|e| e.delete_credential());
}
