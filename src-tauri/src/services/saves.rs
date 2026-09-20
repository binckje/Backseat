use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use crate::services::cloud::{CloudProvider, game_saves_path, game_notes_path, game_images_path};
use crate::error::{AppError, Result};
use log::{info, warn, error};
use futures::future::join_all;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotesData {
    pub description: String,
    pub uploaded_by: String,
    pub uploader_name: String,
    pub uploaded_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSaveMetadata {
    pub notes: Option<NotesData>,
    pub image_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub current: u32,
    pub total: u32,
    pub current_file: Option<String>,
    pub phase: String,
}

/// Collect all files under a directory recursively, returning relative paths
fn collect_files(dir: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().strip_prefix(dir).unwrap_or(e.path()).to_path_buf())
        .collect()
}

pub async fn upload(
    cloud: &dyn CloudProvider,
    game_name: &str,
    app_id: &str,
    save_path: &str,
    description: &str,
    image_paths: &[String],
    uploader_id: &str,
    uploader_name: &str,
    on_progress: impl Fn(ProgressEvent),
) -> Result<()> {
    let cloud_saves = game_saves_path(game_name, app_id);
    let cloud_images = game_images_path(game_name, app_id);

    info!("[Saves] upload game='{}' appId='{}' savePath='{}'", game_name, app_id, save_path);
    on_progress(ProgressEvent { current: 0, total: 0, current_file: None, phase: "scanning".into() });

    let save_dir = Path::new(save_path);
    if !save_dir.exists() {
        error!("[Saves] save dir not found: {}", save_path);
        return Err(AppError::NotFound(format!(
            "Le dossier de sauvegarde n'existe pas : {}", save_path
        )));
    }
    let local_files = collect_files(save_dir);
    if local_files.is_empty() {
        warn!("[Saves] no files found in: {}", save_path);
        return Err(AppError::NotFound(format!(
            "Aucun fichier de sauvegarde trouvé dans : {}", save_path
        )));
    }
    info!("[Saves] {} files to upload (parallel batches of {})", local_files.len(), 4usize);
    let total = (local_files.len() + image_paths.len() + 1) as u32;
    let mut current = 0u32;

    // Upload save files — parallel batches of CONCURRENCY
    const CONCURRENCY: usize = 4;
    let batches: Vec<&[std::path::PathBuf]> = local_files.chunks(CONCURRENCY).collect();
    let batch_count = batches.len();

    for (batch_idx, chunk) in batches.into_iter().enumerate() {
        on_progress(ProgressEvent {
            current,
            total,
            current_file: Some(format!(
                "Lot {}/{} ({} fichier{})",
                batch_idx + 1, batch_count,
                chunk.len(), if chunk.len() > 1 { "s" } else { "" }
            )),
            phase: "uploading".into(),
        });

        let pairs: Vec<(String, String)> = chunk.iter().map(|rel| {
            let local = save_dir.join(rel).to_string_lossy().into_owned();
            let remote = format!("{}/{}", cloud_saves, rel.to_string_lossy().replace('\\', "/"));
            (local, remote)
        }).collect();

        let upload_futs = pairs.iter().map(|(l, r)| cloud.upload_file(l, r));
        let results = join_all(upload_futs).await;
        for result in results { result?; }
        current += chunk.len() as u32;
    }

    // Upload images
    for (i, img) in image_paths.iter().enumerate() {
        let ext = Path::new(img).extension().and_then(|e| e.to_str()).unwrap_or("jpg");
        let remote = format!("{}/{}.{}", cloud_images, i, ext);
        on_progress(ProgressEvent {
            current, total,
            current_file: Some(format!("image {}", i + 1)),
            phase: "uploading".into(),
        });
        cloud.upload_file(img, &remote).await?;
        current += 1;
    }

    // Upload notes.json
    on_progress(ProgressEvent { current, total, current_file: Some("notes.json".into()), phase: "uploading".into() });
    let notes = NotesData {
        description: description.into(),
        uploaded_by: uploader_id.into(),
        uploader_name: uploader_name.into(),
        uploaded_at: chrono::Utc::now().to_rfc3339(),
    };
    cloud.upload_text(&serde_json::to_string_pretty(&notes)?, &game_notes_path(game_name, app_id)).await?;
    on_progress(ProgressEvent { current: total, total, current_file: None, phase: "finalizing".into() });
    info!("[Saves] upload complete — {} files", local_files.len());
    Ok(())
}

pub async fn download(
    cloud: &dyn CloudProvider,
    game_name: &str,
    app_id: &str,
    local_save_path: &str,
    on_progress: impl Fn(ProgressEvent),
) -> Result<()> {
    info!("[Saves] download game='{}' appId='{}' → '{}'", game_name, app_id, local_save_path);
    let cloud_saves = game_saves_path(game_name, app_id);
    on_progress(ProgressEvent { current: 0, total: 0, current_file: None, phase: "scanning".into() });

    let files = list_recursive(cloud, &cloud_saves, &cloud_saves).await;
    if files.is_empty() {
        error!("[Saves] no files found on cloud at {}", cloud_saves);
        return Err(AppError::NotFound("No save files found on cloud for this game".into()));
    }
    info!("[Saves] {} files to download (parallel batches of {})", files.len(), 4usize);

    let total = files.len() as u32;
    let mut current = 0u32;

    // Download save files — parallel batches
    const CONCURRENCY: usize = 4;
    let batches: Vec<&[(String, String)]> = files.chunks(CONCURRENCY).collect();
    let batch_count = batches.len();

    for (batch_idx, chunk) in batches.into_iter().enumerate() {
        on_progress(ProgressEvent {
            current,
            total,
            current_file: Some(format!(
                "Lot {}/{} ({} fichier{})",
                batch_idx + 1, batch_count,
                chunk.len(), if chunk.len() > 1 { "s" } else { "" }
            )),
            phase: "downloading".into(),
        });

        let pairs: Vec<(String, String)> = chunk.iter().map(|(remote_path, local_rel)| {
            let local = Path::new(local_save_path)
                .join(local_rel.replace('/', std::path::MAIN_SEPARATOR_STR))
                .to_string_lossy().into_owned();
            (remote_path.clone(), local)
        }).collect();

        let download_futs = pairs.iter().map(|(r, l)| cloud.download_file(r, l));
        let results = join_all(download_futs).await;
        for result in results { result?; }
        current += chunk.len() as u32;
    }

    on_progress(ProgressEvent { current: total, total, current_file: None, phase: "finalizing".into() });
    Ok(())
}

/// Returns (remote_full_path, local_relative_path) pairs
fn list_recursive<'a>(
    cloud: &'a dyn CloudProvider,
    path: &'a str,
    base: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<(String, String)>> + Send + 'a>> {
    Box::pin(async move {
        let Ok(items) = cloud.list_files(path).await else { return vec![]; };
        let mut result = Vec::new();
        for item in items {
            if item.is_directory {
                let sub = list_recursive(cloud, &item.path, base).await;
                result.extend(sub);
            } else {
                let local_rel = if item.path.starts_with(base) {
                    item.path[base.len()..].trim_start_matches('/').to_string()
                } else if let Some(idx) = item.path.find(base) {
                    item.path[idx + base.len()..].trim_start_matches('/').to_string()
                } else {
                    item.name.clone()
                };
                let clean_remote = format!("{}/{}", base, local_rel);
                result.push((clean_remote, local_rel));
            }
        }
        result
    })
}

pub async fn fetch_metadata(cloud: &dyn CloudProvider, game_name: &str, app_id: &str) -> CloudSaveMetadata {
    let notes = cloud.download_text(&game_notes_path(game_name, app_id)).await.ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let image_count = cloud.list_files(&game_images_path(game_name, app_id)).await
        .unwrap_or_default().iter().filter(|f| !f.is_directory).count() as u32;
    CloudSaveMetadata { notes, image_count }
}

pub async fn download_images(cloud: &dyn CloudProvider, game_name: &str, app_id: &str, cache_dir: &str) -> Result<Vec<String>> {
    let images_path = game_images_path(game_name, app_id);
    tokio::fs::create_dir_all(cache_dir).await?;
    let files = cloud.list_files(&images_path).await.unwrap_or_default();
    let mut local_paths = Vec::new();
    for f in files.iter().filter(|f| !f.is_directory) {
        let local = Path::new(cache_dir).join(&f.name);
        if cloud.download_file(&f.path, local.to_str().unwrap_or("")).await.is_ok() {
            local_paths.push(local.to_string_lossy().to_string());
        }
    }
    Ok(local_paths)
}
