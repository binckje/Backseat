use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use crate::services::cloud::{CloudProvider, game_notes_path, game_images_path};
use crate::error::{AppError, Result};
use futures::{StreamExt, future::join_all};

/// Number of files transferred at the same time
pub const CONCURRENCY: usize = 6;

/// Run `op` on every (a, b) pair with up to `CONCURRENCY` transfers in flight.
/// `on_done(n, name)` is called after each completed transfer with the running
/// count. Stops at the first error (pending transfers are dropped).
pub async fn transfer_all<F, Fut>(
    jobs: Vec<(String, String)>,
    op: F,
    mut on_done: impl FnMut(u32, &str),
) -> Result<()>
where
    F: Fn(String, String) -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    let mut stream = futures::stream::iter(jobs)
        .map(|(a, b)| {
            let label = b.rsplit('/').next().unwrap_or(&b).to_string();
            let fut = op(a, b);
            async move { fut.await.map(|_| label) }
        })
        .buffer_unordered(CONCURRENCY);
    let mut done = 0u32;
    while let Some(res) = stream.next().await {
        let label = res?;
        done += 1;
        on_done(done, &label);
    }
    Ok(())
}

/// Joins a cloud-provided relative path onto `root`, rejecting anything that
/// could escape it (`..`, absolute paths, drive prefixes). Cloud content is
/// shared with friends and must not be trusted to stay inside the save folder.
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf> {
    use std::path::Component;
    let rel_path = Path::new(rel);
    let mut out = root.to_path_buf();
    let mut has_part = false;
    for part in rel.split(['/', '\\']) {
        if part.is_empty() || part == "." { continue; }
        for comp in Path::new(part).components() {
            match comp {
                Component::Normal(c) => { out.push(c); has_part = true; }
                _ => return Err(AppError::Permission(format!("Unsafe path rejected: {}", rel))),
            }
        }
    }
    if !has_part || rel_path.is_absolute() {
        return Err(AppError::Permission(format!("Unsafe path rejected: {}", rel)));
    }
    Ok(out)
}

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

pub async fn fetch_metadata(cloud: &dyn CloudProvider, game_name: &str, app_id: &str) -> CloudSaveMetadata {
    let notes_path = game_notes_path(game_name, app_id);
    let images_path = game_images_path(game_name, app_id);
    let (notes, images) = futures::join!(cloud.download_text(&notes_path), cloud.list_files(&images_path));
    let notes = notes.ok().and_then(|t| serde_json::from_str(&t).ok());
    let image_count = images.unwrap_or_default().iter().filter(|f| !f.is_directory).count() as u32;
    CloudSaveMetadata { notes, image_count }
}

pub async fn download_images(cloud: &dyn CloudProvider, game_name: &str, app_id: &str, cache_dir: &str) -> Result<Vec<String>> {
    let images_path = game_images_path(game_name, app_id);
    tokio::fs::create_dir_all(cache_dir).await?;
    let files = cloud.list_files(&images_path).await.unwrap_or_default();
    let downloads = files.iter().filter(|f| !f.is_directory).map(|f| async move {
        let local = safe_join(Path::new(cache_dir), &f.name).ok()?.to_string_lossy().into_owned();
        cloud.download_file(&f.path, &local).await.ok().map(|_| local)
    });
    Ok(join_all(downloads).await.into_iter().flatten().collect())
}
