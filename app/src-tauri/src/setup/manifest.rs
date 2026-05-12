use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub path: String, // Relative path in repo
    #[serde(rename = "size")]
    pub size_bytes: u64,
    pub sha256: String,
    #[serde(rename = "archive")]
    pub archive_type: Option<String>, // "zip", "tar.gz", or None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoxManifest {
    pub version: String,
    pub total_size_bytes: u64,
    pub models: Vec<ModelEntry>,
}

impl VoxManifest {
    /// Calculates total required space including extraction overhead and safety buffer.
    ///
    /// Overhead is estimated as 1.5x the size of archived models.
    /// Buffer is a fixed 1GB as per Part 2 Directive.
    pub fn calculate_required_space(&self) -> u64 {
        let mut required = 0;
        for model in &self.models {
            required += model.size_bytes;
            // If it's an archive, assume it needs another ~50% space for extraction
            if model.archive_type.is_some() {
                required += model.size_bytes / 2;
            }
        }
        
        // Add 1GB safety buffer
        required + (1024 * 1024 * 1024)
    }

    /// Finds a model entry by ID.
    pub fn get_model(&self, id: &str) -> Option<&ModelEntry> {
        self.models.iter().find(|m| m.id == id)
    }

    /// Fetches the manifest from the Hugging Face repository.
    pub async fn fetch() -> anyhow::Result<Self> {
        let url = "https://huggingface.co/addyo07/Vox/resolve/main/manifest.json";
        log::info!("[VoxManifest] Initiating fetch from: {}", url);
        
        let client = reqwest::Client::builder()
            .user_agent("Vox-App/0.6.0")
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        let response = client.get(url).send().await?;
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to fetch manifest: {}", response.status()));
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
    pub sha256: String,
    pub verified_at: u64, // epoch ms
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
