use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::path::PathBuf;
use crate::error::{AppError, Result};
use log::{info, debug, error};

const API: &str = "https://api.steampowered.com";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SteamGame {
    pub app_id: String,
    pub name: String,
    pub playtime: u64,
    pub icon_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SteamPlayer {
    pub steam_id: String,
    pub persona_name: String,
    pub avatar: String,
    pub profile_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedSavePath {
    pub path: String,
    pub method: String,
}

pub fn detect_steam_path() -> String {
    #[cfg(target_os = "windows")]
    {
        let candidates = [
            r"C:\Program Files (x86)\Steam",
            r"C:\Program Files\Steam",
        ];
        for p in &candidates {
            if std::path::Path::new(p).exists() { return p.to_string(); }
        }
        r"C:\Program Files (x86)\Steam".to_string()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = dirs::home_dir().unwrap_or_default();
        let candidates = [
            home.join(".steam").join("steam"),
            home.join(".local").join("share").join("Steam"),
            home.join(".steam").join("root"),
        ];
        for p in &candidates {
            if p.exists() { return p.to_string_lossy().to_string(); }
        }
        home.join(".steam").join("steam").to_string_lossy().to_string()
    }
}

/// Convert a 64-bit Steam ID to the 32-bit account ID used in userdata folders.
fn steam_id64_to_account_id(steam_id: &str) -> String {
    const STEAM_BASE: u64 = 76561197960265728;
    if let Ok(id64) = steam_id.parse::<u64>() {
        if id64 > STEAM_BASE {
            return (id64 - STEAM_BASE).to_string();
        }
    }
    // Already a 32-bit ID or unparseable — return as-is
    steam_id.to_string()
}

pub async fn detect_save_path(steam_path: &str, steam_id: &str, app_id: &str) -> Option<DetectedSavePath> {
    debug!("[Steam] detect_save_path appId={} steamPath={}", app_id, steam_path);
    // Convert 64-bit Steam ID → 32-bit account ID for userdata path
    let account_id = steam_id64_to_account_id(steam_id);
    debug!("[Steam] steamId64={} → accountId32={}", steam_id, account_id);

    // 1. Steam userdata — try account_id first, then steam_id as fallback
    let userdata = PathBuf::from(steam_path).join("userdata").join(&account_id).join(app_id).join("remote");
    if userdata.exists() {
        let has_files = std::fs::read_dir(&userdata).ok()
            .map(|mut d| d.next().is_some()).unwrap_or(false);
        if has_files {
            info!("[Steam] save path detected (steam_userdata): {}", userdata.display());
            return Some(DetectedSavePath {
                path: userdata.to_string_lossy().to_string(),
                method: "steam_userdata".into(),
            });
        }
    }

    // 2. Proton compatdata (Linux)
    #[cfg(not(target_os = "windows"))]
    {
        let compat = PathBuf::from(steam_path)
            .join("steamapps").join("compatdata").join(app_id)
            .join("pfx").join("drive_c").join("users").join("steamuser");
        if compat.exists() {
            return Some(DetectedSavePath {
                path: compat.to_string_lossy().to_string(),
                method: "proton".into(),
            });
        }
    }

    // Fallback: return userdata path even if empty
    Some(DetectedSavePath {
        path: PathBuf::from(steam_path).join("userdata").join(&account_id).join(app_id).join("remote")
            .to_string_lossy().to_string(),
        method: "steam_userdata".into(),
    })
}

pub async fn get_owned_games(api_key: &str, steam_id: &str) -> Result<Vec<SteamGame>> {
    if api_key.is_empty() {
        return Err(AppError::Config("Clé API Steam manquante — configurez-la dans les paramètres.".into()));
    }
    if steam_id.is_empty() {
        return Err(AppError::Config("Steam ID manquant — configurez-le dans les paramètres.".into()));
    }
    info!("[Steam] get_owned_games → steamId={}", steam_id);
    let client = Client::new();
    let resp = client.get(format!("{}/IPlayerService/GetOwnedGames/v1/", API))
        .query(&[
            ("key", api_key), ("steamid", steam_id),
            ("include_appinfo", "1"), ("include_played_free_games", "1"), ("format", "json"),
        ])
        .send().await?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| AppError::Http(e.to_string()))?;
    if !status.is_success() {
        return Err(AppError::Http(format!("Steam API erreur {}: {}", status, &text[..text.len().min(200)])));
    }
    let j: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        error!("[Steam] JSON parse error: {} — response: {}", e, &text[..text.len().min(300)]);
        AppError::Http(format!("Réponse Steam invalide ({}): {}", e, &text[..text.len().min(200)]))
    })?;
    let games = j["response"]["games"].as_array().cloned().unwrap_or_default();
    info!("[Steam] get_owned_games ← {} jeux trouvés", games.len());
    Ok(games.iter().map(|g| SteamGame {
        app_id: g["appid"].to_string().trim_matches('"').to_string(),
        name: g["name"].as_str().unwrap_or("Unknown").to_string(),
        playtime: g["playtime_forever"].as_u64().unwrap_or(0),
        icon_hash: g["img_icon_url"].as_str().map(|s| s.to_string()),
    }).collect())
}

pub async fn get_friends_list(api_key: &str, steam_id: &str) -> Result<Vec<SteamPlayer>> {
    let client = Client::new();
    let resp = client.get(format!("{}/ISteamUser/GetFriendList/v1/", API))
        .query(&[("key", api_key), ("steamid", steam_id), ("relationship", "friend"), ("format", "json")])
        .send().await?;
    let j: serde_json::Value = resp.json().await?;
    let friends = j["friendslist"]["friends"].as_array().cloned().unwrap_or_default();
    if friends.is_empty() { return Ok(vec![]); }
    let ids: Vec<String> = friends.iter()
        .filter_map(|f| f["steamid"].as_str().map(|s| s.to_string())).collect();
    get_player_summaries(api_key, &ids).await
}

pub async fn get_player_summaries(api_key: &str, steam_ids: &[String]) -> Result<Vec<SteamPlayer>> {
    if steam_ids.is_empty() { return Ok(vec![]); }
    let client = Client::new();
    let mut result = Vec::new();
    for chunk in steam_ids.chunks(100) {
        let ids = chunk.join(",");
        let resp = client.get(format!("{}/ISteamUser/GetPlayerSummaries/v2/", API))
            .query(&[("key", api_key), ("steamids", ids.as_str()), ("format", "json")])
            .send().await?;
        let j: serde_json::Value = resp.json().await?;
        let players = j["response"]["players"].as_array().cloned().unwrap_or_default();
        for p in players {
            result.push(SteamPlayer {
                steam_id: p["steamid"].as_str().unwrap_or("").to_string(),
                persona_name: p["personaname"].as_str().unwrap_or("").to_string(),
                avatar: p["avatarmedium"].as_str().or_else(|| p["avatar"].as_str()).unwrap_or("").to_string(),
                profile_url: p["profileurl"].as_str().unwrap_or("").to_string(),
            });
        }
    }
    Ok(result)
}
