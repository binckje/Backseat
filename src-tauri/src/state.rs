use std::sync::Arc;
use tokio::sync::Mutex;
use crate::services::cloud::CloudProvider;

pub struct CloudState {
    pub provider: Option<Arc<dyn CloudProvider>>,
    pub provider_type: Option<String>,
}

impl Default for CloudState {
    fn default() -> Self {
        Self { provider: None, provider_type: None }
    }
}

pub struct AppState {
    pub cloud: Mutex<CloudState>,
}

impl AppState {
    pub fn new() -> Self {
        Self { cloud: Mutex::new(CloudState::default()) }
    }

    /// Current provider, if connected. The lock is released immediately so
    /// several cloud operations can run concurrently.
    pub async fn provider_opt(&self) -> Option<Arc<dyn CloudProvider>> {
        self.cloud.lock().await.provider.clone()
    }

    pub async fn provider(&self) -> Result<Arc<dyn CloudProvider>, String> {
        self.provider_opt().await.ok_or_else(|| "Not connected".to_string())
    }
}
