pub mod nextcloud;
pub mod googledrive;

use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use crate::error::Result;

// ─── Path helpers ──────────────────────────────────────────────────────────────

pub const CLOUD_ROOT: &str = "/backseat";

pub fn game_cloud_path(game_name: &str, app_id: &str) -> String {
    let slug: String = game_name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(40)
        .collect();
    format!("{}/{}_{}", CLOUD_ROOT, slug, app_id)
}
pub fn game_saves_path(game_name: &str, app_id: &str) -> String {
    format!("{}/saves", game_cloud_path(game_name, app_id))
}
pub fn game_notes_path(game_name: &str, app_id: &str) -> String {
    format!("{}/notes.json", game_cloud_path(game_name, app_id))
}
pub fn game_images_path(game_name: &str, app_id: &str) -> String {
    format!("{}/images", game_cloud_path(game_name, app_id))
}
pub fn game_permissions_path(game_name: &str, app_id: &str) -> String {
    let game_path = game_cloud_path(game_name, app_id);
    let slug = game_path.split('/').last().unwrap_or("game").to_string();
    format!("{}/permissions_{}.json", game_path, slug)
}

// ─── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudFileInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_directory: bool,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub email: Option<String>,
    pub quota: Option<String>,
}

// ─── Trait ─────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait CloudProvider: Send + Sync {
    async fn connect(&mut self) -> Result<UserInfo>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn exists(&self, remote_path: &str) -> Result<bool>;
    async fn list_files(&self, remote_path: &str) -> Result<Vec<CloudFileInfo>>;
    async fn upload_file(&self, local_path: &str, remote_path: &str) -> Result<()>;
    async fn download_file(&self, remote_path: &str, local_path: &str) -> Result<()>;
    async fn upload_text(&self, content: &str, remote_path: &str) -> Result<()>;
    async fn download_text(&self, remote_path: &str) -> Result<String>;
    async fn delete(&self, remote_path: &str) -> Result<()>;
    async fn create_directory(&self, remote_path: &str) -> Result<()>;
}
