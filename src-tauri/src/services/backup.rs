use std::path::{Path, PathBuf};
use crate::error::Result;

fn backup_root(custom: Option<&str>) -> PathBuf {
    if let Some(p) = custom.filter(|s| !s.is_empty()) {
        PathBuf::from(p)
    } else {
        dirs::home_dir().unwrap_or_default().join(".backseat").join("backups")
    }
}

fn backup_dir(steam_id: &str, app_id: &str, custom: Option<&str>) -> PathBuf {
    backup_root(custom).join(steam_id).join(app_id)
}

fn ts_now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S-%3fZ").to_string()
}

pub async fn create(steam_id: &str, app_id: &str, save_path: &str, custom: Option<&str>) -> Result<String> {
    if !Path::new(save_path).exists() { return Ok(String::new()); }
    let ts = ts_now();
    let dest = backup_dir(steam_id, app_id, custom).join(&ts);
    tokio::fs::create_dir_all(&dest).await?;
    copy_dir(Path::new(save_path), &dest).await?;
    prune(steam_id, app_id, 3, custom).await;
    Ok(ts)
}

pub async fn restore(steam_id: &str, app_id: &str, save_path: &str, ts: Option<&str>, custom: Option<&str>) -> Result<()> {
    let bk_path = if let Some(t) = ts {
        backup_dir(steam_id, app_id, custom).join(t)
    } else {
        let entries = list(steam_id, app_id, custom).await;
        let latest = entries.first().ok_or_else(|| crate::error::AppError::NotFound("No backup found".into()))?;
        PathBuf::from(&latest.path)
    };
    if !bk_path.exists() { return Err(crate::error::AppError::NotFound("Backup path not found".into())); }
    // Clear save dir
    if Path::new(save_path).exists() {
        tokio::fs::remove_dir_all(save_path).await?;
    }
    tokio::fs::create_dir_all(save_path).await?;
    copy_dir(&bk_path, Path::new(save_path)).await
}

pub async fn delete_backup(steam_id: &str, app_id: &str, ts: &str, custom: Option<&str>) -> Result<()> {
    let path = backup_dir(steam_id, app_id, custom).join(ts);
    if path.exists() { tokio::fs::remove_dir_all(&path).await?; }
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct BackupEntry { pub timestamp: String, pub path: String }

pub async fn list(steam_id: &str, app_id: &str, custom: Option<&str>) -> Vec<BackupEntry> {
    let dir = backup_dir(steam_id, app_id, custom);
    let mut entries = Vec::new();
    let Ok(mut rd) = tokio::fs::read_dir(&dir).await else { return entries; };
    while let Ok(Some(entry)) = rd.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("20") {
            entries.push(BackupEntry { timestamp: name.clone(), path: dir.join(&name).to_string_lossy().to_string() });
        }
    }
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    entries
}

async fn prune(steam_id: &str, app_id: &str, keep: usize, custom: Option<&str>) {
    let entries = list(steam_id, app_id, custom).await;
    for old in entries.iter().skip(keep) {
        let _ = tokio::fs::remove_dir_all(&old.path).await;
    }
}

fn copy_dir<'a>(src: &'a Path, dst: &'a Path) -> std::pin::Pin<Box<dyn std::future::Future<Output=Result<()>> + Send + 'a>> {
    Box::pin(async move {
        tokio::fs::create_dir_all(dst).await?;
        let mut rd = tokio::fs::read_dir(src).await?;
        while let Some(entry) = rd.next_entry().await? {
            let target = dst.join(entry.file_name());
            if entry.file_type().await?.is_dir() {
                copy_dir(&entry.path(), &target).await?;
            } else {
                tokio::fs::copy(entry.path(), target).await?;
            }
        }
        Ok(())
    })
}
