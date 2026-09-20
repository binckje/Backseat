use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use log::{info, debug};

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HeroicPlatform { Epic, Gog }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeroicGame {
    pub app_name:       String,
    pub title:          String,
    pub platform:       HeroicPlatform,
    pub install_path:   Option<String>,
    pub save_path:      Option<String>,
    pub wine_prefix:    Option<String>,
    pub is_native:      bool,
    /// Square/cover art URL (for sidebar thumbnail)
    pub cover_url:      Option<String>,
    /// Wide banner art URL (for game card hero)
    pub background_url: Option<String>,
}

// ─── Config path ──────────────────────────────────────────────────────────────

pub fn detect_heroic_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    { return dirs::config_dir().map(|p| p.join("heroic")); }

    #[cfg(not(target_os = "windows"))]
    {
        let candidates = [
            dirs::config_dir().map(|p| p.join("heroic")),
            dirs::home_dir().map(|h| h.join(".var/app/com.heroicgameslauncher.hgl/config/heroic")),
        ];
        for c in candidates.into_iter().flatten() {
            if c.exists() { return Some(c); }
        }
        dirs::config_dir().map(|p| p.join("heroic"))
    }
}

// ─── GOG library (titles + save locations) ───────────────────────────────────

#[derive(Deserialize, Default)]
struct GogLibEntry {
    #[serde(default)] title: String,
    #[serde(default)] folder_name: String,
    #[serde(default)] is_linux_native: bool,
    #[serde(default)] gog_save_location: Vec<GogSaveLocation>,
    #[serde(default)] art_square: String,
    #[serde(default)] art_cover: String,
    #[serde(default)] art_background: String,
}

#[derive(Deserialize, Clone)]
struct GogSaveLocation {
    #[serde(default)] location: String,
}

/// Build a map of app_name → library entry from gog_library.json
fn load_gog_library(config_path: &Path) -> HashMap<String, GogLibEntry> {
    let path = config_path.join("store_cache").join("gog_library.json");
    let Ok(txt) = std::fs::read_to_string(&path) else { return HashMap::new(); };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&txt) else { return HashMap::new(); };

    // The file is an array at root level
    let games = match val.as_array() {
        Some(a) => a.clone(),
        None => match val.get("games").and_then(|g| g.as_array()) {
            Some(a) => a.clone(),
            None => return HashMap::new(),
        }
    };

    let mut map = HashMap::new();
    for g in &games {
        let Some(app_name) = g["app_name"].as_str() else { continue };
        let entry: GogLibEntry = serde_json::from_value(g.clone()).unwrap_or_default();
        map.insert(app_name.to_string(), entry);
    }
    map
}

/// Resolve GOG Windows-style save path variables using the Wine prefix.
/// Handles both Proton prefixes ({prefix}/pfx/drive_c) and plain Wine ({prefix}/drive_c).
fn resolve_gog_save_path(template: &str, wine_prefix: &str) -> String {
    let prefix = normalize_path(wine_prefix);
    let base = wine_user_base(&prefix);
    let resolved = template
        .replace("<?SAVED_GAMES?>",   &base.join("Saved Games").to_string_lossy())
        .replace("<?DOCUMENTS?>",     &base.join("Documents").to_string_lossy())
        .replace("<?APPDATA?>",       &base.join("AppData").join("Roaming").to_string_lossy())
        .replace("<?LOCAL_APPDATA?>", &base.join("AppData").join("Local").to_string_lossy())
        .replace("<?LOCALAPPDATA?>",  &base.join("AppData").join("Local").to_string_lossy())
        .replace('\\', "/")
        .replace('\\', "/"); // double-pass for escaped backslashes
    normalize_path(&resolved)
}

// ─── Epic ─────────────────────────────────────────────────────────────────────

pub fn read_epic_games(config_path: &Path) -> Vec<HeroicGame> {
    let path = config_path.join("legendaryConfig").join("legendary").join("installed.json");
    let Ok(txt) = std::fs::read_to_string(&path) else { return vec![]; };
    let Ok(map) = serde_json::from_str::<serde_json::Value>(&txt) else { return vec![]; };
    let Some(obj) = map.as_object() else { return vec![]; };

    let library = load_epic_library(config_path);

    obj.values().filter_map(|e| {
        if e["is_dlc"].as_bool().unwrap_or(false) { return None; }
        let app_name = e["app_name"].as_str()?.to_string();
        let title    = e["title"].as_str().unwrap_or(&app_name).to_string();

        let lib = library.get(&app_name);
        let cover_url = lib.map(|l| {
            if !l.art_square.is_empty() { l.art_square.clone() }
            else { l.art_cover.clone() }
        }).filter(|s| !s.is_empty());
        let background_url = lib
            .map(|l| {
                if !l.art_background.is_empty() { l.art_background.clone() }
                else { l.art_cover.clone() }
            })
            .filter(|s| !s.is_empty());

        Some(HeroicGame {
            app_name, title,
            platform:       HeroicPlatform::Epic,
            install_path:   e["install_path"].as_str().map(|s| s.to_string()),
            save_path:      e["save_path"].as_str().filter(|s| !s.is_empty()).map(|s| s.to_string()),
            wine_prefix:    None,
            is_native:      false,
            cover_url,
            background_url,
        })
    }).collect()
}

// ─── Epic library (art URLs) ──────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct EpicLibEntry {
    #[serde(default)] art_cover:      String,
    #[serde(default)] art_square:     String,
    #[serde(default)] art_background: String,
}

fn load_epic_library(config_path: &Path) -> HashMap<String, EpicLibEntry> {
    let path = config_path.join("store_cache").join("legendary_library.json");
    let Ok(txt) = std::fs::read_to_string(&path) else { return HashMap::new(); };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&txt) else { return HashMap::new(); };

    // legendary_library.json wraps its array under "library" key
    let games = ["library", "games"]
        .iter()
        .find_map(|key| val.get(key).and_then(|v| v.as_array()).cloned())
        .or_else(|| val.as_array().cloned())
        .unwrap_or_default();

    let mut map = HashMap::new();
    for g in &games {
        let Some(app_name) = g["app_name"].as_str() else { continue };
        let entry: EpicLibEntry = serde_json::from_value(g.clone()).unwrap_or_default();
        map.insert(app_name.to_string(), entry);
    }
    map
}

// ─── GOG ──────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GogInstalledFile {
    #[serde(default)] installed: Vec<GogInstalledEntry>,
}

#[derive(Deserialize)]
struct GogInstalledEntry {
    #[serde(rename = "appName")]  app_name:     String,
    #[serde(rename = "install_path", default)] install_path: Option<String>,
    // platform field in installed.json (may say "linux" or "windows")
    #[serde(default)] platform: Option<String>,
}

pub fn read_gog_games(config_path: &Path) -> Vec<HeroicGame> {
    let installed_path = config_path.join("gog_store").join("installed.json");
    let Ok(txt) = std::fs::read_to_string(&installed_path) else { return vec![]; };
    let Ok(data) = serde_json::from_str::<GogInstalledFile>(&txt) else { return vec![]; };

    let library = load_gog_library(config_path);

    data.installed.into_iter().map(|entry| {
        let lib = library.get(&entry.app_name);

        // Prefer library data for native detection; fallback to installed.json platform field
        let is_native = lib.map(|l| l.is_linux_native).unwrap_or_else(|| {
            entry.platform.as_deref().map(|p| p.to_lowercase() == "linux").unwrap_or(false)
        });

        let title = lib
            .map(|l| l.title.clone())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| entry.app_name.clone());

        let folder_name = lib
            .map(|l| l.folder_name.clone())
            .filter(|f| !f.is_empty())
            .unwrap_or_else(|| title.clone());

        let wine_prefix = if !is_native {
            read_gog_wine_prefix(config_path, &entry.app_name)
        } else {
            None
        };

        // Resolve save path
        let save_path = if is_native {
            // Try XDG detection with folder_name and title
            detect_gog_native_save_path(&folder_name, &title)
        } else if let Some(ref prefix) = wine_prefix {
            // Try to resolve gog_save_location from library
            lib.and_then(|l| l.gog_save_location.first().cloned())
                .map(|loc| resolve_gog_save_path(&loc.location, prefix))
                .filter(|s| !s.is_empty())
        } else {
            None
        };

        let cover_url = lib.map(|l| {
            if !l.art_square.is_empty() { l.art_square.clone() }
            else { l.art_cover.clone() }
        }).filter(|s| !s.is_empty());

        let background_url = lib
            .map(|l| l.art_background.clone())
            .filter(|s| !s.is_empty());

        HeroicGame {
            app_name: entry.app_name,
            title,
            platform:       HeroicPlatform::Gog,
            install_path:   entry.install_path,
            save_path,
            wine_prefix,
            is_native,
            cover_url,
            background_url,
        }
    }).collect()
}

fn read_gog_wine_prefix(config_path: &Path, app_name: &str) -> Option<String> {
    let path = config_path.join("GamesConfig").join(format!("{}.json", app_name));
    let txt = std::fs::read_to_string(&path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&txt).ok()?;
    val[app_name]["winePrefix"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| normalize_path(s))
}

/// Remove duplicate slashes from a path string
fn normalize_path(p: &str) -> String {
    let mut result = String::with_capacity(p.len());
    let mut last_slash = false;
    for c in p.chars() {
        if c == '/' {
            if !last_slash { result.push(c); }
            last_slash = true;
        } else {
            result.push(c);
            last_slash = false;
        }
    }
    // Remove trailing slash
    if result.ends_with('/') && result.len() > 1 { result.pop(); }
    result
}

/// Get the Wine drive_c base path, handling both Proton (pfx/drive_c) and plain Wine (drive_c)
fn wine_user_base(wine_prefix: &str) -> PathBuf {
    let pfx_base = Path::new(wine_prefix).join("pfx").join("drive_c").join("users").join("steamuser");
    if pfx_base.exists() {
        pfx_base
    } else {
        Path::new(wine_prefix).join("drive_c").join("users").join("steamuser")
    }
}

fn detect_gog_native_save_path(folder_name: &str, title: &str) -> Option<String> {
    let home = dirs::home_dir()?;
    let candidates = [
        home.join(".local").join("share").join(folder_name),
        home.join(".local").join("share").join(title),
        home.join(".local").join("share").join(folder_name.to_lowercase()),
        home.join(".config").join(folder_name),
        home.join(".config").join(title),
        home.join("Documents").join(folder_name),
    ];
    candidates.into_iter().find(|p| p.exists()).map(|p| p.to_string_lossy().to_string())
}

// ─── Save path suggestions ────────────────────────────────────────────────────

pub fn suggest_save_paths(game: &HeroicGame) -> Vec<String> {
    debug!("[Heroic] suggest_save_paths game='{}' platform={:?} native={} winePrefix={:?}",
        game.title, game.platform, game.is_native, game.wine_prefix);
    match game.platform {
        HeroicPlatform::Epic => {
            game.save_path.iter().cloned().collect()
        }
        HeroicPlatform::Gog => {
            if game.is_native {
                let home = dirs::home_dir().unwrap_or_default();
                let title = &game.title;
                let candidates = vec![
                    home.join(".local").join("share").join(title).to_string_lossy().to_string(),
                    home.join(".local").join("share").join(title.to_lowercase()).to_string_lossy().to_string(),
                    home.join(".config").join(title).to_string_lossy().to_string(),
                    home.join("Documents").join(title).to_string_lossy().to_string(),
                ];
                // Existing paths first, then the rest as fallbacks
                let mut existing: Vec<String> = candidates.iter().filter(|p| Path::new(p).exists()).cloned().collect();
                let rest: Vec<String> = candidates.iter().filter(|p| !Path::new(p).exists()).cloned().collect();
                existing.extend(rest);
                existing
            } else {
                // Wine game: use resolved path from gog_save_location if available,
                // then add standard Wine AppData dirs as fallbacks
                if let Some(ref prefix) = game.wine_prefix {
                    let norm = normalize_path(prefix);
                    let base = wine_user_base(&norm);
                    let mut suggestions = Vec::new();
                    // Resolved path first (most precise)
                    if let Some(ref sp) = game.save_path {
                        suggestions.push(sp.clone());
                    }
                    // Standard fallbacks
                    for sub in &["Saved Games", "Documents", "AppData/Roaming", "AppData/Local", "AppData/LocalLow"] {
                        let p = base.join(sub).to_string_lossy().to_string();
                        if !suggestions.contains(&p) { suggestions.push(p); }
                    }
                    suggestions
                } else {
                    game.save_path.iter().cloned().collect()
                }
            }
        }
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

pub fn get_all_games(config_path: &Path) -> Vec<HeroicGame> {
    info!("[Heroic] Loading games from {}", config_path.display());
    let epic = read_epic_games(config_path);
    info!("[Heroic] {} Epic games found", epic.len());
    let gog = read_gog_games(config_path);
    info!("[Heroic] {} GOG games found", gog.len());
    let mut games = epic;
    games.extend(gog);
    games.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    info!("[Heroic] Total: {} games", games.len());
    games
}
