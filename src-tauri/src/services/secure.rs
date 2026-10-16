//! OS keychain wrapper for sensitive credentials
use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const SERVICE: &str = "backseat";

/// In-memory cache: settings are loaded very often, keychain access is slow
/// (D-Bus on Linux) and may prompt the user on macOS.
fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn entry(key: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, key).map_err(|e| AppError::Config(e.to_string()))
}

pub fn store(key: &str, value: &str) -> Result<()> {
    if cache().lock().unwrap().get(key) == Some(&Some(value.to_string())) {
        return Ok(());
    }
    entry(key)?
        .set_password(value)
        .map_err(|e| AppError::Config(e.to_string()))?;
    cache().lock().unwrap().insert(key.to_string(), Some(value.to_string()));
    Ok(())
}

/// Returns `None` when no secret is stored (or the keychain is unavailable).
pub fn retrieve(key: &str) -> Option<String> {
    if let Some(v) = cache().lock().unwrap().get(key) {
        return v.clone();
    }
    let v = match entry(key).and_then(|e| e.get_password().map_err(|e| AppError::Config(e.to_string()))) {
        Ok(v) => Some(v),
        Err(err) => {
            if !err.to_string().contains("No matching entry") {
                log::warn!("keychain read failed for {key}: {err}");
            }
            None
        }
    };
    cache().lock().unwrap().insert(key.to_string(), v.clone());
    v
}

pub fn remove(key: &str) {
    if cache().lock().unwrap().get(key) == Some(&None) {
        return;
    }
    let _ = entry(key).map(|e| e.delete_credential());
    cache().lock().unwrap().insert(key.to_string(), None);
}
