use super::super::llama_cpp::LlmWorker;
use super::{LlmProvider, ProviderKind};
use crate::core::events::VoxEvent;
use crate::core::settings::RemoteModelInfo;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::mpsc;
use crate::services::traits::LlmEngine as _;

pub struct EmbeddedProvider {
    worker: LlmWorker,
    model_path: PathBuf,
}

impl EmbeddedProvider {
    pub fn new(model_path: &Path, ctx_size: u32, n_threads: u32) -> anyhow::Result<Self> {
        let worker = LlmWorker::new(model_path, ctx_size, n_threads)?;
        Ok(Self {
            worker,
            model_path: model_path.to_path_buf(),
        })
    }
}

impl LlmProvider for EmbeddedProvider {
    fn generate(
        &self,
        text: &str,
        system_prompt: &str,
        turn_id: u32,
        cancel_flag: &Arc<AtomicBool>,
        tx: &mpsc::Sender<VoxEvent>,
    ) -> anyhow::Result<()> {
        self.worker.generate(text, system_prompt, turn_id, cancel_flag, tx)
    }

    fn health_check(&self) -> bool {
        self.model_path.exists()
    }

    fn list_models(&self) -> anyhow::Result<Vec<RemoteModelInfo>> {
        if let Some(parent) = self.model_path.parent() {
            Self::list_models_in_dir(parent)
        } else {
            Ok(Vec::new())
        }
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Embedded
    }
}

impl EmbeddedProvider {
    pub fn list_models_in_dir(dir: &Path) -> anyhow::Result<Vec<RemoteModelInfo>> {
        let mut models = Vec::new();
        if dir.exists() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if ext.to_lowercase() == "gguf" {
                            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                                let metadata = entry.metadata().ok();
                                let size_bytes = metadata.map(|m| m.len());

                                let filename_lower = filename.to_lowercase();
                                let quantization = if filename_lower.contains("q4_k_m") {
                                    Some("Q4_K_M".to_string())
                                } else if filename_lower.contains("q6_k") {
                                    Some("Q6_K".to_string())
                                } else if filename_lower.contains("q2_k") {
                                    Some("Q2_K".to_string())
                                } else if filename_lower.contains("fp16") {
                                    Some("FP16".to_string())
                                } else {
                                    None
                                };

                                let family = if filename_lower.contains("gemma") {
                                    Some("Gemma".to_string())
                                } else if filename_lower.contains("llama") {
                                    Some("Llama".to_string())
                                } else {
                                    None
                                };

                                let clean_name = filename
                                    .strip_suffix(".gguf")
                                    .unwrap_or(filename)
                                    .replace('_', " ")
                                    .replace('-', " ");

                                models.push(RemoteModelInfo {
                                    id: filename.to_string(),
                                    name: clean_name,
                                    size_bytes,
                                    quantization,
                                    family,
                                    provider_kind: "embedded".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(models)
    }
}
