use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::Result;
use crate::services::secure;

fn config_path() -> PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("backseat");
    std::fs::create_dir_all(&p).ok();
    p.push("config.json");
    p
}

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)] pub steam_api_key: String,
    #[serde(default)] pub steam_id: String,
    #[serde(default)] pub steam_path: String,
    #[serde(default)] pub backup_path: String,
    #[serde(default)] pub active_cloud_provider: Option<String>,
    #[serde(default = "default_json_obj")] pub cloud_providers: serde_json::Value,
    #[serde(default)] pub shared_games: Vec<SharedGame>,
    #[serde(skip_serializing_if = "Option::is_none")] pub debug_logging: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedGame {
    pub app_id: String,
    pub name: String,
    pub save_path: String,
    pub save_path_detection_method: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub icon_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub has_local_backup: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] pub backup_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub launcher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub hero_platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub cover_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub background_url: Option<String>,
}

fn default_json_obj() -> serde_json::Value { serde_json::json!({}) }

// ─── Load / Save ──────────────────────────────────────────────────────────────

// ─── Secrets ──────────────────────────────────────────────────────────────────

/// (JSON parent pointer, field) of every secret: kept in the OS keychain, never in config.json
const SECRETS: &[(&str, &str)] = &[
    ("", "steamApiKey"),
    ("/cloudProviders/nextcloud/nextcloud", "password"),
    ("/cloudProviders/kdrive/kdrive", "appPassword"),
    ("/cloudProviders/google_drive/googleDrive", "clientSecret"),
    ("/cloudProviders/google_drive/googleDrive", "accessToken"),
    ("/cloudProviders/google_drive/googleDrive", "refreshToken"),
];

fn keychain_key(parent: &str, field: &str) -> String {
    format!("{}/{}", parent, field)
}

/// Move secrets out of `json` into the keychain. A secret the keychain refuses
/// stays in the JSON so it is not lost.
fn extract_secrets(json: &mut serde_json::Value) {
    for (parent, field) in SECRETS {
        let key = keychain_key(parent, field);
        let Some(obj) = json.pointer_mut(parent).and_then(|v| v.as_object_mut()) else {
            secure::remove(&key);
            continue;
        };
        match obj.get(*field).and_then(|v| v.as_str()).filter(|v| !v.is_empty()).map(str::to_string) {
            Some(value) => match secure::store(&key, &value) {
                Ok(()) => { obj.remove(*field); }
                Err(err) => {
                    log::warn!("keychain unavailable, {key} kept in config.json: {err}");
                }
            },
            None => {
                obj.remove(*field);
                secure::remove(&key);
            }
        }
    }
}

/// Fill secrets from the keychain. Returns true if a plaintext secret was found
/// in the JSON (legacy config to migrate).
fn inject_secrets(json: &mut serde_json::Value) -> bool {
    let mut plaintext = false;
    for (parent, field) in SECRETS {
        let Some(obj) = json.pointer_mut(parent).and_then(|v| v.as_object_mut()) else { continue };
        if obj.get(*field).and_then(|v| v.as_str()).is_some_and(|v| !v.is_empty()) {
            plaintext = true;
        } else if let Some(value) = secure::retrieve(&keychain_key(parent, field)) {
            obj.insert(field.to_string(), serde_json::Value::String(value));
        }
    }
    plaintext
}

// ─── Load / Save ──────────────────────────────────────────────────────────────

pub fn load() -> AppSettings {
    let Ok(txt) = std::fs::read_to_string(config_path()) else {
        return AppSettings::default();
    };
    let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&txt) else {
        return AppSettings::default();
    };
    let has_plaintext = inject_secrets(&mut json);
    let settings: AppSettings = serde_json::from_value(json).unwrap_or_default();
    if has_plaintext {
        // Migrate secrets from an older plaintext config.json
        if let Err(err) = save(&settings) {
            log::warn!("secret migration failed: {err}");
        }
    }
    settings
}

pub fn save(settings: &AppSettings) -> Result<()> {
    let mut json = serde_json::to_value(settings)?;
    extract_secrets(&mut json);
    std::fs::write(config_path(), serde_json::to_string_pretty(&json)?)?;
    Ok(())
}

/// Merge a partial JSON patch into current settings and persist
pub fn patch(partial: serde_json::Value) -> Result<AppSettings> {
    let mut current = serde_json::to_value(load())?;
    if let (Some(cur), Some(patch)) = (current.as_object_mut(), partial.as_object()) {
        for (k, v) in patch {
            cur.insert(k.clone(), v.clone());
        }
    }
    let merged: AppSettings = serde_json::from_value(current)?;
    save(&merged)?;
    Ok(merged)
}
