use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::Result;

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

pub fn load() -> AppSettings {
    let path = config_path();
    if let Ok(txt) = std::fs::read_to_string(&path) {
        serde_json::from_str(&txt).unwrap_or_default()
    } else {
        AppSettings::default()
    }
}

pub fn save(settings: &AppSettings) -> Result<()> {
    let path = config_path();
    let json = serde_json::to_string_pretty(settings)?;
    std::fs::write(&path, json)?;
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
