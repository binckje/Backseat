use tokio::sync::Mutex;
use crate::services::cloud::CloudProvider;

pub struct CloudState {
    pub provider: Option<Box<dyn CloudProvider>>,
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
}
