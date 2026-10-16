//! All Tauri commands — equivalent of Electron's ipc/index.ts
use tauri::{AppHandle, Manager, State};
use serde::Deserialize;
use crate::{
    state::{AppState, CloudState},
    settings::{self, AppSettings, SharedGame},
    services::{steam, saves, backup, permissions},
    services::cloud::{CloudProvider, UserInfo, game_cloud_path},
    services::cloud::nextcloud::NextcloudProvider,
    services::cloud::googledrive::{GoogleDriveProvider, GoogleDriveConfig},
};

type Res<T> = Result<T, String>;
fn e(err: impl ToString) -> String { err.to_string() }

// ─── App ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ─── Settings ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_settings() -> Res<AppSettings> {
    Ok(settings::load())
}

#[tauri::command]
pub fn save_settings_cmd(partial: serde_json::Value) -> Res<AppSettings> {
    settings::patch(partial).map_err(e)
}

#[tauri::command]
pub fn detect_steam_path() -> String {
    steam::detect_steam_path()
}

// ─── Steam ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_owned_games() -> Res<Vec<steam::SteamGame>> {
    let s = settings::load();
    steam::get_owned_games(&s.steam_api_key, &s.steam_id).await.map_err(e)
}

#[tauri::command]
pub async fn get_friends_list() -> Res<Vec<steam::SteamPlayer>> {
    let s = settings::load();
    steam::get_friends_list(&s.steam_api_key, &s.steam_id).await.map_err(e)
}

#[tauri::command]
pub async fn get_player_info(steam_id: String) -> Res<Option<steam::SteamPlayer>> {
    let s = settings::load();
    let players = steam::get_player_summaries(&s.steam_api_key, &[steam_id]).await.map_err(e)?;
    Ok(players.into_iter().next())
}

#[tauri::command]
pub async fn detect_save_path(app_id: String) -> Res<Option<steam::DetectedSavePath>> {
    let s = settings::load();
    Ok(steam::detect_save_path(&s.steam_path, &s.steam_id, &app_id).await)
}

// ─── Cloud ────────────────────────────────────────────────────────────────────

fn build_provider(provider_type: &str, s: &AppSettings) -> Result<Box<dyn CloudProvider>, String> {
    let providers = s.cloud_providers.as_object()
        .ok_or("No cloud providers configured")?;
    match provider_type {
        "nextcloud" => {
            let cfg = &providers["nextcloud"]["nextcloud"];
            Ok(Box::new(NextcloudProvider::new(
                cfg["serverUrl"].as_str().unwrap_or("").to_string(),
                cfg["username"].as_str().unwrap_or("").to_string(),
                cfg["password"].as_str().unwrap_or("").to_string(),
            )))
        }
        "kdrive" => {
            let cfg = &providers["kdrive"]["kdrive"];
            Ok(Box::new(NextcloudProvider::new_kdrive(
                cfg["username"].as_str().unwrap_or("").to_string(),
                cfg["appPassword"].as_str().unwrap_or("").to_string(),
            )))
        }
        "google_drive" => {
            let cfg = &providers["google_drive"]["googleDrive"];
            let config: GoogleDriveConfig = serde_json::from_value(cfg.clone())
                .map_err(|e| e.to_string())?;
            Ok(Box::new(GoogleDriveProvider::new(config)))
        }
        _ => Err(format!("Unknown provider: {}", provider_type)),
    }
}

#[tauri::command]
pub async fn cloud_connect(provider_type: String, state: State<'_, AppState>) -> Res<UserInfo> {
    let s = settings::load();
    let mut provider = build_provider(&provider_type, &s)?;
    let info = provider.connect().await.map_err(e)?;
    let mut cloud = state.cloud.lock().await;
    *cloud = CloudState { provider: Some(provider), provider_type: Some(provider_type.clone()) };
    // Persist active provider
    let mut new_s = s; new_s.active_cloud_provider = Some(provider_type);
    settings::save(&new_s).map_err(e)?;
    Ok(info)
}

#[tauri::command]
pub async fn cloud_disconnect(state: State<'_, AppState>) -> Res<()> {
    let mut cloud = state.cloud.lock().await;
    *cloud = CloudState::default();
    let mut s = settings::load(); s.active_cloud_provider = None;
    settings::save(&s).map_err(e)
}

#[tauri::command]
pub async fn cloud_get_status(state: State<'_, AppState>) -> Res<serde_json::Value> {
    let cloud = state.cloud.lock().await;
    Ok(serde_json::json!({
        "connected": cloud.provider.is_some(),
        "providerType": cloud.provider_type
    }))
}

#[tauri::command]
pub fn gdrive_get_auth_url() -> Res<String> {
    let s = settings::load();
    let client_id = s.cloud_providers["google_drive"]["googleDrive"]["clientId"]
        .as_str().ok_or("Client ID missing")?;
    Ok(GoogleDriveProvider::auth_url(client_id))
}

#[tauri::command]
pub async fn gdrive_exchange_code(code: String, state: State<'_, AppState>) -> Res<()> {
    let s = settings::load();
    let cfg: GoogleDriveConfig = serde_json::from_value(
        s.cloud_providers["google_drive"]["googleDrive"].clone()
    ).map_err(e)?;
    let mut provider = GoogleDriveProvider::new(cfg);
    provider.exchange_code(&code).await.map_err(e)?;
    let updated_cfg = provider.get_config().clone();
    let mut new_s = s;
    new_s.cloud_providers["google_drive"]["googleDrive"] = serde_json::to_value(updated_cfg).map_err(e)?;
    settings::save(&new_s).map_err(e)
}

// ─── Games ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_shared_games() -> Res<Vec<SharedGame>> {
    Ok(settings::load().shared_games)
}

#[tauri::command]
pub async fn add_game(game: SharedGame, state: State<'_, AppState>) -> Res<Vec<SharedGame>> {
    let mut s = settings::load();
    if s.shared_games.iter().any(|g| g.app_id == game.app_id) {
        return Err("Game already added".into());
    }
    let cloud = state.cloud.lock().await;
    if let Some(ref p) = cloud.provider {
        permissions::init(p.as_ref(), &game.app_id, &game.name, &s.steam_id).await.map_err(e)?;
    }
    s.shared_games.push(game);
    settings::save(&s).map_err(e)?;
    Ok(s.shared_games)
}

#[tauri::command]
pub async fn remove_game(app_id: String, state: State<'_, AppState>) -> Res<()> {
    let mut s = settings::load();
    if let Some(game) = s.shared_games.iter().find(|g| g.app_id == app_id) {
        let cloud = state.cloud.lock().await;
        if let Some(ref p) = cloud.provider {
            let folder = game_cloud_path(&game.name, &app_id);
            let _ = p.delete(&folder).await;
        }
    }
    s.shared_games.retain(|g| g.app_id != app_id);
    settings::save(&s).map_err(e)
}

#[tauri::command]
pub fn update_save_path(app_id: String, new_path: String) -> Res<()> {
    let mut s = settings::load();
    if let Some(g) = s.shared_games.iter_mut().find(|g| g.app_id == app_id) {
        g.save_path = new_path;
        g.save_path_detection_method = "manual".into();
    }
    settings::save(&s).map_err(e)
}

// ─── Permissions ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_permissions(app_id: String, state: State<'_, AppState>) -> Res<Option<permissions::GamePermissions>> {
    let s = settings::load();
    let game = s.shared_games.iter().find(|g| g.app_id == app_id)
        .ok_or("Game not found")?.clone();
    let cloud = state.cloud.lock().await;
    let p = cloud.provider.as_ref().ok_or("Not connected")?;
    let mut perms = permissions::read(p.as_ref(), &game.name, &app_id).await;
    if perms.is_none() {
        permissions::init(p.as_ref(), &app_id, &game.name, &s.steam_id).await.map_err(e)?;
        perms = permissions::read(p.as_ref(), &game.name, &app_id).await;
    }
    Ok(perms)
}

#[tauri::command]
pub async fn grant_access(app_id: String, friend_steam_id: String, state: State<'_, AppState>) -> Res<()> {
    let s = settings::load();
    let game = s.shared_games.iter().find(|g| g.app_id == app_id).ok_or("Game not found")?;
    let cloud = state.cloud.lock().await;
    let p = cloud.provider.as_ref().ok_or("Not connected")?;
    permissions::grant(p.as_ref(), &game.name, &app_id, &friend_steam_id, &s.steam_id).await.map_err(e)
}

#[tauri::command]
pub async fn revoke_access(app_id: String, friend_steam_id: String, state: State<'_, AppState>) -> Res<()> {
    let s = settings::load();
    let game = s.shared_games.iter().find(|g| g.app_id == app_id).ok_or("Game not found")?;
    let cloud = state.cloud.lock().await;
    let p = cloud.provider.as_ref().ok_or("Not connected")?;
    permissions::revoke(p.as_ref(), &game.name, &app_id, &friend_steam_id, &s.steam_id).await.map_err(e)
}

#[tauri::command]
pub async fn release_lock(app_id: String, state: State<'_, AppState>) -> Res<()> {
    let s = settings::load();
    let game = s.shared_games.iter().find(|g| g.app_id == app_id).ok_or("Game not found")?;
    let cloud = state.cloud.lock().await;
    let p = cloud.provider.as_ref().ok_or("Not connected")?;
    permissions::release_lock(p.as_ref(), &game.name, &app_id, &s.steam_id).await.map_err(e)
}

// ─── Saves ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadPayload {
    pub app_id: String,
    pub description: String,
    pub image_paths: Vec<String>,
}

#[tauri::command]
pub async fn upload_save(payload: UploadPayload, on_progress: tauri::ipc::Channel<serde_json::Value>, state: State<'_, AppState>) -> Res<()> {
    let s = settings::load();
    let game = s.shared_games.iter().find(|g| g.app_id == payload.app_id)
        .ok_or("Game not found")?.clone();
    let cloud = state.cloud.lock().await;
    let p = cloud.provider.as_ref().ok_or("Not connected")?;

    // Permission + lock check
    let mut perms = permissions::read(p.as_ref(), &game.name, &payload.app_id).await;
    if perms.is_none() {
        permissions::init(p.as_ref(), &payload.app_id, &game.name, &s.steam_id).await.map_err(e)?;
        perms = permissions::read(p.as_ref(), &game.name, &payload.app_id).await;
    }
    let perms = perms.ok_or("Permissions not found")?;
    if !permissions::can_write(&perms, &s.steam_id) { return Err("Permission denied".into()); }
    if permissions::locked_by_other(&perms, &s.steam_id) { return Err("Save is locked by another player".into()); }

    // Resolve actual Steam persona name for the uploader display
    let uploader_name = if !s.steam_api_key.is_empty() && !s.steam_id.is_empty() {
        steam::get_player_summaries(&s.steam_api_key, &[s.steam_id.clone()]).await
            .ok()
            .and_then(|players| players.into_iter().next())
            .map(|p| p.persona_name)
            .unwrap_or_else(|| s.steam_id.clone())
    } else {
        s.steam_id.clone()
    };

    saves::upload(
        p.as_ref(), &game.name, &payload.app_id, &game.save_path,
        &payload.description, &payload.image_paths, &s.steam_id, &uploader_name,
        |ev| { let _ = on_progress.send(serde_json::to_value(&ev).unwrap_or_default()); },
    ).await.map_err(e)?;

    permissions::mark_uploaded(p.as_ref(), &game.name, &payload.app_id, &s.steam_id).await.map_err(e)
}

#[tauri::command]
pub async fn download_save(app_id: String, on_progress: tauri::ipc::Channel<serde_json::Value>, state: State<'_, AppState>) -> Res<serde_json::Value> {
    let s = settings::load();
    let game = s.shared_games.iter().find(|g| g.app_id == app_id)
        .ok_or("Game not found")?.clone();
    let cloud = state.cloud.lock().await;
    let p = cloud.provider.as_ref().ok_or("Not connected")?;

    let perms = permissions::read(p.as_ref(), &game.name, &app_id).await
        .ok_or("Permissions not found")?;
    if !permissions::can_read(&perms, &s.steam_id) { return Err("Permission denied".into()); }
    if permissions::locked_by_other(&perms, &s.steam_id) {
        return Err(format!("Save locked by {}", perms.locked_by.as_deref().unwrap_or("unknown")));
    }

    // Backup existing saves
    // Abort if the backup fails: downloading would overwrite saves with no way back.
    let ts = backup::create(&s.steam_id, &app_id, &game.save_path,
        if s.backup_path.is_empty() { None } else { Some(s.backup_path.as_str()) })
        .await.map_err(|err| format!("Backup failed, download aborted: {}", err))?;

    // Lock
    permissions::lock(p.as_ref(), &game.name, &app_id, &s.steam_id).await.map_err(e)?;

    // Download
    let result = saves::download(
        p.as_ref(), &game.name, &app_id, &game.save_path,
        |ev| { let _ = on_progress.send(serde_json::to_value(&ev).unwrap_or_default()); },
    ).await;

    if let Err(ref err) = result {
        let _ = permissions::release_lock(p.as_ref(), &game.name, &app_id, &s.steam_id).await;
        return Err(err.to_string());
    }

    Ok(serde_json::json!({ "backupTimestamp": ts }))
}

#[tauri::command]
pub async fn get_cloud_info(app_id: String, state: State<'_, AppState>) -> Res<saves::CloudSaveMetadata> {
    let s = settings::load();
    let game = s.shared_games.iter().find(|g| g.app_id == app_id).ok_or("Game not found")?.clone();
    let cloud = state.cloud.lock().await;
    let p = cloud.provider.as_ref().ok_or("Not connected")?;
    Ok(saves::fetch_metadata(p.as_ref(), &game.name, &app_id).await)
}

#[tauri::command]
pub async fn download_images(app_id: String, app: AppHandle, state: State<'_, AppState>) -> Res<Vec<String>> {
    let s = settings::load();
    let game = s.shared_games.iter().find(|g| g.app_id == app_id).ok_or("Game not found")?.clone();
    let cloud = state.cloud.lock().await;
    let p = cloud.provider.as_ref().ok_or("Not connected")?;
    let cache_dir = app.path().app_cache_dir().map_err(e)?
        .join("image-cache").join(&app_id);
    saves::download_images(p.as_ref(), &game.name, &app_id,
        cache_dir.to_str().unwrap_or("")).await.map_err(e)
}

// ─── Backup ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_backups(app_id: String) -> Res<Vec<backup::BackupEntry>> {
    let s = settings::load();
    let custom = if s.backup_path.is_empty() { None } else { Some(s.backup_path.as_str()) };
    Ok(backup::list(&s.steam_id, &app_id, custom).await)
}

#[tauri::command]
pub async fn restore_backup(app_id: String, timestamp: Option<String>) -> Res<()> {
    let s = settings::load();
    let game = s.shared_games.iter().find(|g| g.app_id == app_id).ok_or("Game not found")?.clone();
    let custom = if s.backup_path.is_empty() { None } else { Some(s.backup_path.as_str()) };
    backup::restore(&s.steam_id, &app_id, &game.save_path, timestamp.as_deref(), custom).await.map_err(e)
}

#[tauri::command]
pub async fn delete_backup_cmd(app_id: String, timestamp: String) -> Res<()> {
    let s = settings::load();
    let custom = if s.backup_path.is_empty() { None } else { Some(s.backup_path.as_str()) };
    backup::delete_backup(&s.steam_id, &app_id, &timestamp, custom).await.map_err(e)
}

// ─── Dialogs ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn open_folder_dialog(app: AppHandle) -> Res<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    use tokio::sync::oneshot;
    let (tx, rx) = oneshot::channel();
    app.dialog().file().pick_folder(move |f| {
        let _ = tx.send(f.map(|p| p.to_string()));
    });
    Ok(rx.await.ok().flatten())
}

#[tauri::command]
pub async fn open_images_dialog(app: AppHandle) -> Res<Vec<String>> {
    use tauri_plugin_dialog::DialogExt;
    use tokio::sync::oneshot;
    let (tx, rx) = oneshot::channel();
    app.dialog().file()
        .add_filter("Images", &["jpg", "jpeg", "png", "gif", "webp"])
        .set_can_create_directories(true)
        .pick_files(move |files| {
            let _ = tx.send(files.unwrap_or_default().iter()
                .map(|f| f.to_string()).collect());
        });
    Ok(rx.await.unwrap_or_default())
}

// ─── Cloud scan ───────────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudGameSuggestion {
    pub app_id: String,
    pub game_name: String,
    pub is_owner: bool,
}

/// Scan /backseat/ on the cloud, return games the user has access to
/// but hasn't added to Backseat yet.
#[tauri::command]
pub async fn scan_cloud_games(state: State<'_, AppState>) -> Res<Vec<CloudGameSuggestion>> {
    let s = settings::load();
    let cloud = state.cloud.lock().await;
    let p = cloud.provider.as_ref().ok_or("Not connected")?;

    let entries = p.list_files("/backseat").await.map_err(e)?;
    let already_added: std::collections::HashSet<String> =
        s.shared_games.iter().map(|g| g.app_id.clone()).collect();

    let mut suggestions = Vec::new();

    for entry in entries {
        if !entry.is_directory { continue; }

        // Folder name is GameSlug_AppID — permissions file mirrors this
        let folder = &entry.name;
        let perms_path = format!("/backseat/{}/permissions_{}.json", folder, folder);

        let Ok(txt) = p.download_text(&perms_path).await else { continue };
        let Ok(perms) = serde_json::from_str::<permissions::GamePermissions>(&txt) else { continue };

        let has_access = perms.owner == s.steam_id
            || perms.allowed_users.contains(&s.steam_id);

        if has_access && !already_added.contains(&perms.app_id) {
            suggestions.push(CloudGameSuggestion {
                is_owner: perms.owner == s.steam_id,
                app_id: perms.app_id,
                game_name: perms.game_name,
            });
        }
    }

    Ok(suggestions)
}

// ─── Heroic Games Launcher ────────────────────────────────────────────────────

use crate::services::heroic::{self, HeroicGame};
use log::{info};

#[tauri::command]
pub fn detect_heroic_path() -> Option<String> {
    heroic::detect_heroic_config_path()
        .map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn get_heroic_games() -> Res<Vec<HeroicGame>> {
    let config_path = heroic::detect_heroic_config_path()
        .ok_or("Heroic Games Launcher non détecté")?;
    Ok(heroic::get_all_games(&config_path))
}

#[tauri::command]
pub fn suggest_heroic_save_paths(app_name: String, platform: String) -> Res<Vec<String>> {
    let config_path = heroic::detect_heroic_config_path()
        .ok_or("Heroic Games Launcher non détecté")?;
    let games = heroic::get_all_games(&config_path);
    let game = games.iter()
        .find(|g| g.app_name == app_name && format!("{:?}", g.platform).to_lowercase() == platform.to_lowercase())
        .ok_or("Jeu non trouvé")?;
    Ok(heroic::suggest_save_paths(game))
}

// ─── Bulk status refresh ──────────────────────────────────────────────────────

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStatusUpdate {
    pub app_id: String,
    pub status: String,
    pub locked_by: Option<String>,
}

/// Fetch cloud status for all shared games in one call (for sidebar badges).
#[tauri::command]
pub async fn refresh_all_games_status(state: State<'_, AppState>) -> Res<Vec<GameStatusUpdate>> {
    let s = settings::load();
    let cloud = state.cloud.lock().await;
    let p = cloud.provider.as_ref().ok_or("Not connected")?;

    let mut updates = Vec::new();
    for game in &s.shared_games {
        if let Some(perms) = permissions::read(p.as_ref(), &game.name, &game.app_id).await {
            updates.push(GameStatusUpdate {
                app_id: game.app_id.clone(),
                status: perms.status,
                locked_by: perms.locked_by,
            });
        }
    }
    Ok(updates)
}

// ─── Heroic art enrichment ────────────────────────────────────────────────────

/// Enrich existing Heroic games in config with art URLs from the current
/// Heroic library (for games added before art URL support was added).
#[tauri::command]
pub fn enrich_heroic_art() -> Res<bool> {
    use crate::services::heroic;

    let mut s = settings::load();
    let config_path = match heroic::detect_heroic_config_path() {
        Some(p) => p,
        None => return Ok(false),
    };
    let heroic_games = heroic::get_all_games(&config_path);
    let mut changed = false;

    for sg in s.shared_games.iter_mut() {
        if sg.launcher.as_deref() != Some("heroic") { continue; }
        if sg.cover_url.is_some() && sg.background_url.is_some() { continue; }

        if let Some(hg) = heroic_games.iter().find(|g| g.app_name == sg.app_id) {
            if sg.cover_url.is_none() && hg.cover_url.is_some() {
                sg.cover_url = hg.cover_url.clone();
                changed = true;
            }
            if sg.background_url.is_none() && hg.background_url.is_some() {
                sg.background_url = hg.background_url.clone();
                changed = true;
            }
        }
    }

    if changed {
        settings::save(&s).map_err(e)?;
    }
    Ok(changed)
}

// ─── Debug / Logging ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_log_path() -> String {
    crate::logger::log_path().to_string_lossy().to_string()
}

#[tauri::command]
pub fn set_debug_logging(enabled: bool, app: AppHandle) -> Res<()> {
    let mut s = settings::load();
    s.debug_logging = Some(enabled);
    settings::save(&s).map_err(e)?;
    info!("[Debug] debug_logging set to {} — restart required to take effect", enabled);
    Ok(())
}

#[tauri::command]
pub fn open_log_folder(app: AppHandle) -> Res<()> {
    use tauri_plugin_shell::ShellExt;
    let path = crate::logger::log_path();
    let folder = path.parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());
    app.shell().open(&folder, None).map_err(|e| e.to_string())
}

// ─── File tree ────────────────────────────────────────────────────────────────

use crate::filetree::{self, FileTreeNode};
use futures::future::join_all;

#[tauri::command]
pub async fn get_local_save_tree(app_id: String) -> Res<Vec<FileTreeNode>> {
    let s = settings::load();
    let game = s.shared_games.iter().find(|g| g.app_id == app_id).ok_or("Game not found")?.clone();
    let nodes = filetree::build_local_tree(std::path::Path::new(&game.save_path)).map_err(e)?;
    Ok(nodes)
}

#[tauri::command]
pub async fn get_cloud_save_tree(app_id: String, state: State<'_, AppState>) -> Res<Vec<FileTreeNode>> {
    let s = settings::load();
    let game = s.shared_games.iter().find(|g| g.app_id == app_id).ok_or("Game not found")?.clone();
    let cloud = state.cloud.lock().await;
    let p = cloud.provider.as_ref().ok_or("Not connected")?;
    let cloud_saves = crate::services::cloud::game_saves_path(&game.name, &app_id);
    let nodes = filetree::build_cloud_tree(p.as_ref(), &cloud_saves).await;
    Ok(nodes)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadSelectedPayload {
    pub app_id:        String,
    pub selected_paths: Vec<String>,  // relative paths of FILES to upload
    pub description:   String,
    pub image_paths:   Vec<String>,
}

#[tauri::command]
pub async fn upload_selected_saves(
    payload: UploadSelectedPayload,
    on_progress: tauri::ipc::Channel<serde_json::Value>,
    state: State<'_, AppState>,
) -> Res<()> {
    use crate::services::{permissions, saves};
    use std::path::Path;

    let s = settings::load();
    let game = s.shared_games.iter().find(|g| g.app_id == payload.app_id)
        .ok_or("Game not found")?.clone();
    let cloud = state.cloud.lock().await;
    let p = cloud.provider.as_ref().ok_or("Not connected")?;

    // Permission check
    let mut perms = permissions::read(p.as_ref(), &game.name, &payload.app_id).await;
    if perms.is_none() {
        permissions::init(p.as_ref(), &payload.app_id, &game.name, &s.steam_id).await.map_err(e)?;
        perms = permissions::read(p.as_ref(), &game.name, &payload.app_id).await;
    }
    let perms = perms.ok_or("Permissions not found")?;
    if !permissions::can_write(&perms, &s.steam_id) { return Err("Permission denied".into()); }
    if permissions::locked_by_other(&perms, &s.steam_id) { return Err("Save is locked by another player".into()); }

    let cloud_saves = crate::services::cloud::game_saves_path(&game.name, &payload.app_id);
    let save_root = Path::new(&game.save_path);
    let total = (payload.selected_paths.len() + payload.image_paths.len() + 1) as u32;
    let mut current = 0u32;

    on_progress.send(serde_json::json!({ "current": 0, "total": total, "currentFile": null, "phase": "uploading" })).ok();

    const CONCURRENCY: usize = 4;
    let valid_files: Vec<&String> = payload.selected_paths.iter()
        .filter(|rel| {
            let local = save_root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
            local.exists() && !local.is_dir()
        }).collect();

    for (batch_idx, chunk) in valid_files.chunks(CONCURRENCY).enumerate() {
        on_progress.send(serde_json::json!({
            "current": current, "total": total,
            "currentFile": format!("Lot {}/{}", batch_idx + 1, (valid_files.len() + CONCURRENCY - 1) / CONCURRENCY),
            "phase": "uploading"
        })).ok();
        let pairs: Vec<(String, String)> = chunk.iter().map(|rel| {
            let local = save_root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR))
                .to_string_lossy().into_owned();
            let remote = format!("{}/{}", cloud_saves, rel);
            (local, remote)
        }).collect();
        let futs = pairs.iter().map(|(l, r)| p.upload_file(l, r));
        let results = join_all(futs).await;
        for r in results { r.map_err(e)?; }
        current += chunk.len() as u32;
    }

    // Images
    for (i, img) in payload.image_paths.iter().enumerate() {
        let ext = Path::new(img).extension().and_then(|x| x.to_str()).unwrap_or("jpg");
        let remote = format!("{}/{}.{}", crate::services::cloud::game_images_path(&game.name, &payload.app_id), i, ext);
        on_progress.send(serde_json::json!({ "current": current, "total": total, "currentFile": format!("image {}", i+1), "phase": "uploading" })).ok();
        p.upload_file(img, &remote).await.map_err(e)?;
        current += 1;
    }

    // notes.json
    let uploader_name = s.steam_id.clone();
    let notes = saves::NotesData {
        description: payload.description,
        uploaded_by: s.steam_id.clone(),
        uploader_name,
        uploaded_at: chrono::Utc::now().to_rfc3339(),
    };
    p.upload_text(&serde_json::to_string_pretty(&notes).map_err(e)?,
        &crate::services::cloud::game_notes_path(&game.name, &payload.app_id)).await.map_err(e)?;

    on_progress.send(serde_json::json!({ "current": total, "total": total, "currentFile": null, "phase": "finalizing" })).ok();
    permissions::mark_uploaded(p.as_ref(), &game.name, &payload.app_id, &s.steam_id).await.map_err(e)
}

#[tauri::command]
pub async fn download_selected_saves(
    app_id: String,
    selected_paths: Vec<String>,  // relative paths of FILES to download
    on_progress: tauri::ipc::Channel<serde_json::Value>,
    state: State<'_, AppState>,
) -> Res<serde_json::Value> {
    use crate::services::{permissions, backup};
    use std::path::Path;

    let s = settings::load();
    let game = s.shared_games.iter().find(|g| g.app_id == app_id)
        .ok_or("Game not found")?.clone();
    let cloud = state.cloud.lock().await;
    let p = cloud.provider.as_ref().ok_or("Not connected")?;

    let perms = permissions::read(p.as_ref(), &game.name, &app_id).await
        .ok_or("Permissions not found")?;
    if !permissions::can_read(&perms, &s.steam_id) { return Err("Permission denied".into()); }
    if permissions::locked_by_other(&perms, &s.steam_id) {
        return Err(format!("Save locked by {}", perms.locked_by.as_deref().unwrap_or("unknown")));
    }

    // Validate every target path before touching anything on disk
    let save_root = Path::new(&game.save_path);
    let locals: Vec<String> = selected_paths.iter()
        .map(|rel| saves::safe_join(save_root, rel).map(|p| p.to_string_lossy().into_owned()))
        .collect::<std::result::Result<_, _>>().map_err(e)?;

    // Full backup before any download — abort if it fails, saves would be lost otherwise
    let custom = if s.backup_path.is_empty() { None } else { Some(s.backup_path.as_str()) };
    let ts = backup::create(&s.steam_id, &app_id, &game.save_path, custom).await
        .map_err(|err| format!("Backup failed, download aborted: {}", err))?;

    // Lock
    permissions::lock(p.as_ref(), &game.name, &app_id, &s.steam_id).await.map_err(e)?;

    let cloud_saves = crate::services::cloud::game_saves_path(&game.name, &app_id);
    let total = selected_paths.len() as u32;
    let mut current = 0u32;

    on_progress.send(serde_json::json!({ "current": 0, "total": total, "currentFile": null, "phase": "downloading" })).ok();

    const CONCURRENCY: usize = 4;
    let indexed: Vec<(&String, &String)> = selected_paths.iter().zip(locals.iter()).collect();
    for (batch_idx, chunk) in indexed.chunks(CONCURRENCY).enumerate() {
        on_progress.send(serde_json::json!({
            "current": current, "total": total,
            "currentFile": format!("Lot {}/{}", batch_idx + 1, (selected_paths.len() + CONCURRENCY - 1) / CONCURRENCY),
            "phase": "downloading"
        })).ok();
        let pairs: Vec<(String, String)> = chunk.iter().map(|(rel, local)| {
            (format!("{}/{}", cloud_saves, rel), (*local).clone())
        }).collect();
        let futs = pairs.iter().map(|(r, l)| p.download_file(r, l));
        let results = join_all(futs).await;
        for r in results {
            if let Err(err) = r {
                let _ = permissions::release_lock(p.as_ref(), &game.name, &app_id, &s.steam_id).await;
                return Err(err.to_string());
            }
        }
        current += chunk.len() as u32;
    }

    on_progress.send(serde_json::json!({ "current": total, "total": total, "currentFile": null, "phase": "finalizing" })).ok();
    Ok(serde_json::json!({ "backupTimestamp": ts }))
}
