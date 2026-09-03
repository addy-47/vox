use super::{
    APP_MANIFEST_FETCH_TIMEOUT_SECS, APP_MANIFEST_URL, MANIFEST_FETCH_TIMEOUT_SECS,
    MODELS_MANIFEST_URL,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub path: String,
    #[serde(rename = "size")]
    pub size_bytes: u64,
    pub sha256: String,
    #[serde(rename = "archive")]
    pub archive_type: Option<String>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelGroup {
    pub id: String,
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub subcategory: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub parameters: Option<String>,
    #[serde(default)]
    pub ram_usage: Option<String>,
    #[serde(default)]
    pub tradeoffs: Option<String>,
    pub version: String,
    #[serde(default)]
    pub is_built_in: bool,
    #[serde(default)]
    pub is_cloud: bool,
    #[serde(default)]
    pub is_remote: bool,
    #[serde(default)]
    pub required: bool,
    pub files: Vec<ModelEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoxManifest {
    pub models_version: String,
    pub release_notes: Option<Vec<String>>,
    pub total_size_bytes: u64,
    pub model_groups: Vec<ModelGroup>,
}

impl VoxManifest {
    /// Calculates total required space including extraction overhead and safety buffer.
    pub fn calculate_required_space(&self) -> u64 {
        let mut required = 0;
        for group in &self.model_groups {
            for model in &group.files {
                required += model.size_bytes;
                if model.archive_type.is_some() {
                    required += model.size_bytes / 2;
                }
            }
        }

        required + (1024 * 1024 * 1024)
    }

    /// Fetches the manifest from the Hugging Face repository.
    pub async fn fetch() -> anyhow::Result<Self> {
        let url = MODELS_MANIFEST_URL;
        log::info!("[VoxManifest] Initiating fetch from: {}", url);

        let client = reqwest::Client::builder()
            .user_agent("Vox-App/0.8.1")
            .timeout(std::time::Duration::from_secs(MANIFEST_FETCH_TIMEOUT_SECS))
            .build()?;

        let response = client.get(url).send().await?;
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to fetch manifest: {}",
                response.status()
            ));
        }

        let text = response.text().await?;
        match serde_json::from_str::<VoxManifest>(&text) {
            Ok(m) => Ok(m),
            Err(e) => {
                log::error!("[VoxManifest] JSON Parse Error: {}. Content: {}", e, text);
                Err(anyhow::anyhow!("JSON Parse Error: {}", e))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedMarker {
    pub model_id: Option<String>,
    pub sha256: String,
    pub verified_at: u64,
    pub expected_size: u64,
}

impl VerifiedMarker {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let marker = serde_json::from_str(&content)?;
        Ok(marker)
    }

    pub async fn load_async(path: &Path) -> anyhow::Result<Self> {
        let p = path.to_path_buf();
        tokio::task::spawn_blocking(move || Self::load(&p))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to join marker load task: {}", e))?
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub async fn save_async(&self, path: &Path) -> anyhow::Result<()> {
        let p = path.to_path_buf();
        let marker = self.clone();
        tokio::task::spawn_blocking(move || marker.save(&p))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to join marker save task: {}", e))?
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxInfo {
    pub package: String,
    pub update_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppManifest {
    pub latest_version: String,
    pub release_notes: Vec<String>,
    pub linux: LinuxInfo,
}

impl AppManifest {
    /// Fetches the application manifest from GitHub Pages.
    pub async fn fetch() -> anyhow::Result<Self> {
        let url = APP_MANIFEST_URL;
        log::info!("[AppManifest] Initiating fetch from: {}", url);

        let client = reqwest::Client::builder()
            .user_agent("Vox-App/0.8.1")
            .timeout(std::time::Duration::from_secs(
                APP_MANIFEST_FETCH_TIMEOUT_SECS,
            ))
            .build()?;

        let response = client.get(url).send().await?;
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to fetch app manifest: {}",
                response.status()
            ));
        }

        let text = response.text().await?;
        match serde_json::from_str::<AppManifest>(&text) {
            Ok(m) => Ok(m),
            Err(e) => {
                log::error!("[AppManifest] JSON Parse Error: {}. Content: {}", e, text);
                Err(anyhow::anyhow!("JSON Parse Error: {}", e))
            }
        }
    }
}
