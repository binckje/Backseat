//! Google Drive REST API provider (no SDK — pure reqwest + OAuth2).
use reqwest::Client;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use super::{CloudProvider, CloudFileInfo, UserInfo};
use crate::error::{AppError, Result};

const AUTH_URL: &str  = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const DRIVE_URL: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD_URL: &str = "https://www.googleapis.com/upload/drive/v3/files";
const REDIRECT_URI: &str = "urn:ietf:wg:oauth:2.0:oob";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDriveConfig {
    pub client_id: String,
    pub client_secret: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expiry: Option<i64>,
}

pub struct GoogleDriveProvider {
    config: GoogleDriveConfig,
    client: Client,
    id_cache: std::collections::HashMap<String, String>,
}

impl GoogleDriveProvider {
    pub fn new(config: GoogleDriveConfig) -> Self {
        Self { config, client: Client::new(), id_cache: Default::default() }
    }

    pub fn auth_url(client_id: &str) -> String {
        format!("{}?client_id={}&redirect_uri={}&response_type=code&scope=https://www.googleapis.com/auth/drive.file&access_type=offline&prompt=consent",
            AUTH_URL, urlencoding::encode(client_id), urlencoding::encode(REDIRECT_URI))
    }

    pub async fn exchange_code(&mut self, code: &str) -> Result<()> {
        let resp = self.client.post(TOKEN_URL).form(&[
            ("code", code),
            ("client_id", &self.config.client_id),
            ("client_secret", &self.config.client_secret),
            ("redirect_uri", REDIRECT_URI),
            ("grant_type", "authorization_code"),
        ]).send().await?;
        let j: serde_json::Value = resp.json().await?;
        self.config.access_token = j["access_token"].as_str().map(|s| s.to_string());
        self.config.refresh_token = j["refresh_token"].as_str().map(|s| s.to_string());
        self.config.token_expiry = j["expires_in"].as_i64()
            .map(|s| chrono::Utc::now().timestamp() + s);
        Ok(())
    }

    pub fn get_config(&self) -> &GoogleDriveConfig { &self.config }

    async fn ensure_token(&mut self) -> Result<String> {
        let expired = self.config.token_expiry
            .map(|e| chrono::Utc::now().timestamp() > e - 60)
            .unwrap_or(true);
        if expired {
            if let Some(rt) = &self.config.refresh_token.clone() {
                let resp = self.client.post(TOKEN_URL).form(&[
                    ("refresh_token", rt.as_str()),
                    ("client_id", &self.config.client_id),
                    ("client_secret", &self.config.client_secret),
                    ("grant_type", "refresh_token"),
                ]).send().await?;
                let j: serde_json::Value = resp.json().await?;
                if let Some(at) = j["access_token"].as_str() {
                    self.config.access_token = Some(at.to_string());
                    self.config.token_expiry = j["expires_in"].as_i64()
                        .map(|s| chrono::Utc::now().timestamp() + s);
                }
            }
        }
        self.config.access_token.clone()
            .ok_or_else(|| AppError::Cloud("No access token".into()))
    }

    async fn find_id(&mut self, path: &str) -> Result<Option<String>> {
        if let Some(id) = self.id_cache.get(path) { return Ok(Some(id.clone())); }
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        let mut parent_id = "root".to_string();
        let token = self.ensure_token().await?;
        for (i, part) in parts.iter().enumerate() {
            let q = format!("name='{}' and '{}' in parents and trashed=false", part.replace('\'', "\\'"), parent_id);
            let resp = self.client.get(format!("{}/files", DRIVE_URL))
                .bearer_auth(&token)
                .query(&[("q", q.as_str()), ("fields", "files(id,name)")])
                .send().await?.json::<serde_json::Value>().await?;
            let files = resp["files"].as_array().cloned().unwrap_or_default();
            if files.is_empty() { return Ok(None); }
            parent_id = files[0]["id"].as_str().unwrap_or("").to_string();
            if i == parts.len() - 1 {
                self.id_cache.insert(path.to_string(), parent_id.clone());
                return Ok(Some(parent_id));
            }
        }
        Ok(None)
    }
}

#[async_trait]
impl CloudProvider for GoogleDriveProvider {
    async fn connect(&mut self) -> Result<UserInfo> {
        let token = self.ensure_token().await?;
        let resp = self.client.get(format!("{}/about", DRIVE_URL))
            .bearer_auth(&token).query(&[("fields", "user")]).send().await?;
        let j: serde_json::Value = resp.json().await?;
        Ok(UserInfo {
            name: j["user"]["displayName"].as_str().unwrap_or("Google User").to_string(),
            email: j["user"]["emailAddress"].as_str().map(|s| s.to_string()),
            quota: None,
        })
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.id_cache.clear();
        Ok(())
    }

    async fn exists(&self, _remote_path: &str) -> Result<bool> { Ok(true) }

    async fn list_files(&self, _remote_path: &str) -> Result<Vec<CloudFileInfo>> {
        Ok(vec![])
    }

    async fn create_directory(&self, _remote_path: &str) -> Result<()> { Ok(()) }

    async fn upload_file(&self, local_path: &str, remote_path: &str) -> Result<()> {
        let _ = (local_path, remote_path);
        Err(AppError::Cloud("Google Drive upload not yet implemented".into()))
    }

    async fn download_file(&self, remote_path: &str, local_path: &str) -> Result<()> {
        let _ = (remote_path, local_path);
        Err(AppError::Cloud("Google Drive download not yet implemented".into()))
    }

    async fn upload_text(&self, content: &str, remote_path: &str) -> Result<()> {
        let _ = (content, remote_path);
        Err(AppError::Cloud("Google Drive not yet fully implemented".into()))
    }

    async fn download_text(&self, remote_path: &str) -> Result<String> {
        let _ = remote_path;
        Err(AppError::Cloud("Google Drive not yet fully implemented".into()))
    }

    async fn delete(&self, remote_path: &str) -> Result<()> {
        let _ = remote_path;
        Err(AppError::Cloud("Google Drive not yet fully implemented".into()))
    }
}
