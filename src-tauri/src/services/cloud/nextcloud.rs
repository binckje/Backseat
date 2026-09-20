//! WebDAV provider for Nextcloud and kDrive.
//! Uses reqwest directly — no webdav library — to avoid the /../ path issues
//! encountered with the TypeScript version.

use reqwest::{Client, Method};
use std::path::Path;
use async_trait::async_trait;
use super::{CloudProvider, CloudFileInfo, UserInfo};
use crate::error::{AppError, Result};
use log::{info, debug, error};

pub struct NextcloudProvider {
    server_url: String,
    username: String,
    password: String,
    client: Client,
}

impl NextcloudProvider {
    pub fn new(server_url: String, username: String, password: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap_or_default();
        Self {
            server_url: server_url.trim_end_matches('/').to_string(),
            username, password, client,
        }
    }

    /// kDrive factory — uses Infomaniak's WebDAV endpoint
    pub fn new_kdrive(username: String, app_password: String) -> Self {
        Self::new("https://connect.drive.infomaniak.com".to_string(), username, app_password)
    }

    // ── URL builders ────────────────────────────────────────────────────────

    /// Full WebDAV URL for a given path  
    /// e.g. /backseat/game/saves/file → https://server/remote.php/dav/files/user/backseat/...
    fn url(&self, path: &str) -> String {
        format!(
            "{}/remote.php/dav/files/{}/{}",
            self.server_url,
            urlencoding::encode(&self.username),
            path.trim_start_matches('/')
        )
    }

    /// The DAV path prefix returned in PROPFIND hrefs
    fn dav_prefix(&self) -> String {
        format!("/remote.php/dav/files/{}", &self.username)
    }

    // ── PROPFIND parsing ─────────────────────────────────────────────────────

    fn parse_propfind(&self, xml: &str, request_path: &str) -> Vec<CloudFileInfo> {
        let mut entries = Vec::new();
        let prefix = self.dav_prefix();
        let req = request_path.trim_end_matches('/');
        let mut pos = 0;

        while pos < xml.len() {
            // Find next <d:response> (case-insensitive via lowercase scan)
            let lower = xml[pos..].to_lowercase();
            let Some(s) = lower.find("<d:response>") else { break };
            let abs = pos + s;
            let Some(e) = xml[abs..].to_lowercase().find("</d:response>") else { break };
            let block = &xml[abs..abs + e + 13];
            pos = abs + e + 13;

            let block_lo = block.to_lowercase();

            // Extract href from ORIGINAL block (not lowercased) to preserve path casing.
            // We use block_lo only to locate the tag offsets.
            let href_decoded = {
                let open_tag = "<d:href>";
                let close_tag = "</d:href>";
                let Some(start) = block_lo.find(open_tag) else { continue };
                let content_start = start + open_tag.len();
                let Some(end_rel) = block_lo[content_start..].find(close_tag) else { continue };
                // Extract from original block at same byte positions
                let raw = &block[content_start..content_start + end_rel];
                urlencoding::decode(raw).unwrap_or_default().to_string()
            };
            let relative = if href_decoded.starts_with(&prefix) {
                href_decoded[prefix.len()..].trim_end_matches('/').to_string()
            } else {
                // Fallback: find where /backseat starts
                if let Some(idx) = href_decoded.find("/backseat/") {
                    href_decoded[idx..].trim_end_matches('/').to_string()
                } else {
                    href_decoded.trim_end_matches('/').to_string()
                }
            };

            // Skip the directory itself
            if relative == req || relative.is_empty() { continue; }

            let is_dir = block_lo.contains("<d:collection") ;
            let size = Self::inner(&block_lo, "<d:getcontentlength>", "</d:getcontentlength>")
                .and_then(|s| s.parse().ok()).unwrap_or(0u64);
            let modified_at = Self::inner(block, "<d:getlastmodified>", "</d:getlastmodified>")
                .map(|s| s.to_string());
            let name = relative.rsplit('/').next().unwrap_or("").to_string();

            entries.push(CloudFileInfo { name, path: relative, size, is_directory: is_dir, modified_at });
        }
        entries
    }

    fn inner<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
        let s = text.find(open)?;
        let cs = s + open.len();
        let e = text[cs..].find(close)?;
        Some(&text[cs..cs + e])
    }
}

#[async_trait]
impl CloudProvider for NextcloudProvider {
    async fn connect(&mut self) -> Result<UserInfo> {
        info!("[Nextcloud] Connecting to {} as {}", self.server_url, self.username);
        let url = format!("{}/ocs/v2.php/cloud/users/{}",
            self.server_url, urlencoding::encode(&self.username));
        let resp = self.client.get(&url)
            .header("OCS-APIRequest", "true")
            .header("Accept", "application/json")
            .basic_auth(&self.username, Some(&self.password))
            .send().await.map_err(|e| AppError::Cloud(e.to_string()))?;

        if resp.status().is_success() {
            let j: serde_json::Value = resp.json().await.unwrap_or_default();
            let d = &j["ocs"]["data"];
            let info = UserInfo {
                name: d["displayname"].as_str().unwrap_or(&self.username).to_string(),
                email: d["email"].as_str().map(|s| s.to_string()),
                quota: None,
            };
            info!("[Nextcloud] Connected as '{}'", info.name);
            return Ok(info);
        }
        // Fallback: test WebDAV
        debug!("[Nextcloud] OCS failed ({}), trying WebDAV fallback", resp.status());
        self.exists("/backseat").await?;
        Ok(UserInfo { name: self.username.clone(), email: Some(self.username.clone()), quota: None })
    }

    async fn disconnect(&mut self) -> Result<()> { Ok(()) }

    async fn exists(&self, remote_path: &str) -> Result<bool> {
        let resp = self.client
            .request(Method::from_bytes(b"PROPFIND").unwrap(), self.url(remote_path))
            .header("Depth", "0")
            .basic_auth(&self.username, Some(&self.password))
            .send().await.map_err(|e| AppError::Http(e.to_string()))?;
        Ok(resp.status().is_success() || resp.status().as_u16() == 207)
    }

    async fn list_files(&self, remote_path: &str) -> Result<Vec<CloudFileInfo>> {
        debug!("[Nextcloud] list_files {}", remote_path);
        let resp = self.client
            .request(Method::from_bytes(b"PROPFIND").unwrap(), self.url(remote_path))
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .basic_auth(&self.username, Some(&self.password))
            .body(r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/><d:getcontentlength/><d:getlastmodified/></d:prop></d:propfind>"#)
            .send().await.map_err(|e| AppError::Http(e.to_string()))?;

        if !resp.status().is_success() && resp.status().as_u16() != 207 {
            return Ok(vec![]);
        }
        let xml = resp.text().await.map_err(|e| AppError::Http(e.to_string()))?;
        let entries = self.parse_propfind(&xml, remote_path);
        debug!("[Nextcloud] list_files {} → {} entries", remote_path, entries.len());
        Ok(entries)
    }

    async fn create_directory(&self, remote_path: &str) -> Result<()> {
        let parts: Vec<&str> = remote_path.trim_start_matches('/').split('/').collect();
        let mut current = String::new();
        for part in &parts {
            if part.is_empty() { continue; }
            current = format!("{}/{}", current, part);
            let _ = self.client
                .request(Method::from_bytes(b"MKCOL").unwrap(), self.url(&current))
                .basic_auth(&self.username, Some(&self.password))
                .send().await;
        }
        Ok(())
    }

    async fn upload_file(&self, local_path: &str, remote_path: &str) -> Result<()> {
        if let Some(parent) = Path::new(remote_path).parent() {
            self.create_directory(parent.to_str().unwrap_or("/")).await?;
        }
        let bytes = tokio::fs::read(local_path).await?;
        let size = bytes.len();
        debug!("[Nextcloud] upload {} → {} ({} bytes)", local_path, remote_path, size);
        let start = std::time::Instant::now();
        let resp = self.client.put(self.url(remote_path))
            .basic_auth(&self.username, Some(&self.password))
            .body(bytes).send().await.map_err(|e| AppError::Http(e.to_string()))?;
        let elapsed = start.elapsed().as_millis();
        let rate = if elapsed > 0 { size as u128 * 1000 / elapsed / 1024 } else { 0 };
        debug!("[Nextcloud] upload done {} → status={} time={}ms rate={}KB/s",
            remote_path, resp.status(), elapsed, rate);
        Ok(())
    }

    async fn download_file(&self, remote_path: &str, local_path: &str) -> Result<()> {
        if let Some(parent) = Path::new(local_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        debug!("[Nextcloud] download {} → {}", remote_path, local_path);
        let start = std::time::Instant::now();
        let resp = self.client.get(self.url(remote_path))
            .basic_auth(&self.username, Some(&self.password))
            .send().await.map_err(|e| AppError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            error!("[Nextcloud] download failed {} status={}", remote_path, resp.status());
            return Err(AppError::NotFound(format!("Download failed {} ({})", remote_path, resp.status())));
        }
        let bytes = resp.bytes().await.map_err(|e| AppError::Http(e.to_string()))?;
        let size = bytes.len();
        tokio::fs::write(local_path, &bytes).await?;
        let elapsed = start.elapsed().as_millis();
        let rate = if elapsed > 0 { size as u128 * 1000 / elapsed / 1024 } else { 0 };
        debug!("[Nextcloud] download done {} → {} bytes in {}ms ({}KB/s)",
            remote_path, size, elapsed, rate);
        Ok(())
    }

    async fn upload_text(&self, content: &str, remote_path: &str) -> Result<()> {
        if let Some(parent) = Path::new(remote_path).parent() {
            self.create_directory(parent.to_str().unwrap_or("/")).await?;
        }
        self.client.put(self.url(remote_path))
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "application/json")
            .body(content.to_string()).send().await.map_err(|e| AppError::Http(e.to_string()))?;
        Ok(())
    }

    async fn download_text(&self, remote_path: &str) -> Result<String> {
        let resp = self.client.get(self.url(remote_path))
            .basic_auth(&self.username, Some(&self.password))
            .send().await.map_err(|e| AppError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(AppError::NotFound(format!("Not found: {}", remote_path)));
        }
        resp.text().await.map_err(|e| AppError::Http(e.to_string()))
    }

    async fn delete(&self, remote_path: &str) -> Result<()> {
        let resp = self.client.delete(self.url(remote_path))
            .basic_auth(&self.username, Some(&self.password))
            .send().await.map_err(|e| AppError::Http(e.to_string()))?;
        let s = resp.status().as_u16();
        if !resp.status().is_success() && s != 404 {
            return Err(AppError::Cloud(format!("Delete failed: {}", s)));
        }
        Ok(())
    }
}
