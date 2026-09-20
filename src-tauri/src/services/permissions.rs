use serde::{Deserialize, Serialize};
use crate::services::cloud::CloudProvider;
use crate::services::cloud::{game_cloud_path, game_permissions_path};
use crate::error::{AppError, Result};
use log::{info, debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePermissions {
    pub version: u32,
    pub last_updated: String,
    pub app_id: String,
    pub game_name: String,
    pub owner: String,
    pub allowed_users: Vec<String>,
    pub status: String,
    pub locked_by: Option<String>,
    pub locked_at: Option<String>,
    pub last_upload: Option<String>,
    pub last_uploader_steam_id: Option<String>,
}

fn now() -> String { chrono::Utc::now().to_rfc3339() }

pub async fn read(cloud: &dyn CloudProvider, game_name: &str, app_id: &str) -> Option<GamePermissions> {
    let path = game_permissions_path(game_name, app_id);
    let txt = cloud.download_text(&path).await.ok()?;
    let perms: Option<GamePermissions> = serde_json::from_str(&txt).ok();
    if let Some(ref p) = perms {
        debug!("[Perms] read game='{}' owner='{}' status='{}' allowed={:?}",
            game_name, p.owner, p.status, p.allowed_users);
    }
    perms
}

pub async fn write(cloud: &dyn CloudProvider, game_name: &str, app_id: &str, mut p: GamePermissions) -> Result<()> {
    p.last_updated = now();
    let game_dir = game_cloud_path(game_name, app_id);
    cloud.create_directory(&game_dir).await?;
    cloud.upload_text(&serde_json::to_string_pretty(&p)?, &game_permissions_path(game_name, app_id)).await
}

pub fn can_read(p: &GamePermissions, steam_id: &str) -> bool {
    p.owner == steam_id || p.allowed_users.contains(&steam_id.to_string())
}
pub fn can_write(p: &GamePermissions, steam_id: &str) -> bool {
    p.owner == steam_id || p.allowed_users.contains(&steam_id.to_string())
}
pub fn locked_by_other(p: &GamePermissions, steam_id: &str) -> bool {
    p.locked_by.as_deref().map(|l| l != steam_id).unwrap_or(false)
}

pub async fn init(cloud: &dyn CloudProvider, app_id: &str, game_name: &str, owner: &str) -> Result<()> {
    if read(cloud, game_name, app_id).await.is_none() {
        write(cloud, game_name, app_id, GamePermissions {
            version: 1, last_updated: now(),
            app_id: app_id.into(), game_name: game_name.into(), owner: owner.into(),
            allowed_users: vec![], status: "not_uploaded".into(),
            locked_by: None, locked_at: None, last_upload: None, last_uploader_steam_id: None,
        }).await?;
    }
    Ok(())
}

pub async fn grant(cloud: &dyn CloudProvider, game_name: &str, app_id: &str, steam_id: &str, requester: &str) -> Result<()> {
    let mut p = read(cloud, game_name, app_id).await
        .ok_or_else(|| AppError::NotFound("Permissions not found".into()))?;
    if p.owner != requester { return Err(AppError::Permission("Only owner can grant access".into())); }
    if !p.allowed_users.contains(&steam_id.to_string()) { p.allowed_users.push(steam_id.into()); }
    info!("[Perms] grant game='{}' steamId='{}' by '{}'", game_name, steam_id, requester);
    write(cloud, game_name, app_id, p).await
}

pub async fn revoke(cloud: &dyn CloudProvider, game_name: &str, app_id: &str, steam_id: &str, requester: &str) -> Result<()> {
    let mut p = read(cloud, game_name, app_id).await
        .ok_or_else(|| AppError::NotFound("Permissions not found".into()))?;
    if p.owner != requester { return Err(AppError::Permission("Only owner can revoke access".into())); }
    p.allowed_users.retain(|u| u != steam_id);
    if p.locked_by.as_deref() == Some(steam_id) {
        p.locked_by = None; p.locked_at = None; p.status = "available".into();
    }
    write(cloud, game_name, app_id, p).await
}

pub async fn lock(cloud: &dyn CloudProvider, game_name: &str, app_id: &str, steam_id: &str) -> Result<()> {
    let mut p = read(cloud, game_name, app_id).await
        .ok_or_else(|| AppError::NotFound("Permissions not found".into()))?;
    if locked_by_other(&p, steam_id) {
        warn!("[Perms] lock refused game='{}' lockedBy='{}'", game_name, p.locked_by.as_deref().unwrap_or("?"));
        return Err(AppError::Permission(format!("Locked by {}", p.locked_by.as_deref().unwrap_or("unknown"))));
    }
    info!("[Perms] lock game='{}' by '{}'", game_name, steam_id);
    p.locked_by = Some(steam_id.into()); p.locked_at = Some(now()); p.status = "locked".into();
    write(cloud, game_name, app_id, p).await
}

pub async fn release_lock(cloud: &dyn CloudProvider, game_name: &str, app_id: &str, steam_id: &str) -> Result<()> {
    let mut p = read(cloud, game_name, app_id).await
        .ok_or_else(|| AppError::NotFound("Permissions not found".into()))?;
    let is_locker = p.locked_by.as_deref() == Some(steam_id);
    if !is_locker && p.owner != steam_id {
        return Err(AppError::Permission("Cannot release a lock you don't own".into()));
    }
    p.locked_by = None; p.locked_at = None; p.status = "available".into();
    write(cloud, game_name, app_id, p).await
}

pub async fn mark_uploaded(cloud: &dyn CloudProvider, game_name: &str, app_id: &str, steam_id: &str) -> Result<()> {
    let mut p = read(cloud, game_name, app_id).await
        .ok_or_else(|| AppError::NotFound("Permissions not found".into()))?;
    p.status = "available".into(); p.locked_by = None; p.locked_at = None;
    p.last_upload = Some(now()); p.last_uploader_steam_id = Some(steam_id.into());
    write(cloud, game_name, app_id, p).await
}
