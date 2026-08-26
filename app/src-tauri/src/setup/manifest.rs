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
    ///
    /// Overhead is estimated as 1.5x the size of archived models.
    /// Buffer is a fixed 1GB as per Part 2 Directive.
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

    /// Finds a model entry by ID.
    pub fn get_model(&self, id: &str) -> Option<&ModelEntry> {
        for group in &self.model_groups {
            if let Some(m) = group.files.iter().find(|m| m.id == id) {
                return Some(m);
            }
        }
        None
    }

    /// Fetches the manifest from the Hugging Face repository.
    pub async fn fetch() -> anyhow::Result<Self> {
        let url = "https://huggingface.co/addyo07/vox-models/resolve/main/models_manifest.json";
        log::info!("[VoxManifest] Initiating fetch from: {}", url);

        let client = reqwest::Client::builder()
            .user_agent("Vox-App/0.8.1")
            .timeout(std::time::Duration::from_secs(15))
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

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
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
        let url = "https://addy-47.github.io/vox/manifests/app_manifest.json";
        log::info!("[AppManifest] Initiating fetch from: {}", url);

        let client = reqwest::Client::builder()
            .user_agent("Vox-App/0.8.1")
            .timeout(std::time::Duration::from_secs(10))
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
