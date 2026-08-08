use anyhow::{anyhow, Result};
use query_sieve::{MemoryScope, MemoryScopeClassifier};
use std::path::Path;
use std::sync::OnceLock;

pub const MEMORY_SCOPE_MODEL_DIR: &str = "modernbert_memory_scope";
pub const CLASSIFIER_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const CLASSIFIER_TOKENIZER_FILENAME: &str = "tokenizer.json";

/// Engine wrapper for the ModernBERT 4-Class MemoryScope Classifier.
pub struct QueryScopeClassifier {
    engine: MemoryScopeClassifier,
}

impl QueryScopeClassifier {
    /// Loads the ModernBERT MemoryScope model and tokenizer from the specified directory.
    pub fn load(classifier_dir: &Path) -> Result<Self> {
        let model_path = classifier_dir.join(CLASSIFIER_MODEL_FILENAME);
        let tokenizer_path = classifier_dir.join(CLASSIFIER_TOKENIZER_FILENAME);

        if !model_path.exists() || !tokenizer_path.exists() {
            return Err(anyhow!(
                "MemoryScope classifier assets missing at {:?} (model: {}, tokenizer: {})",
                classifier_dir,
                CLASSIFIER_MODEL_FILENAME,
                CLASSIFIER_TOKENIZER_FILENAME
            ));
        }

        let engine = MemoryScopeClassifier::load(&model_path, &tokenizer_path)?;
        Ok(Self { engine })
    }

    /// Classifies an input query into 4-class `MemoryScope`.
    pub fn classify(&self, text: &str) -> MemoryScope {
        match self.engine.classify(text) {
            Ok(scope) => scope,
            Err(e) => {
                log::warn!(
                    "[QueryScopeClassifier] Classification error for query '{}': {}. Defaulting to Domain.",
                    text,
                    e
                );
                MemoryScope::Domain
            }
        }
    }
}

static SCOPE_CLASSIFIER_INSTANCE: OnceLock<QueryScopeClassifier> = OnceLock::new();

/// Initializes the `QueryScopeClassifier` singleton from the specified model directory.
/// Returns `Ok(true)` if loaded, `Ok(false)` if model files do not exist.
pub fn init_scope_classifier(classifier_dir: &Path) -> Result<bool> {
    let model_path = classifier_dir.join(CLASSIFIER_MODEL_FILENAME);
    let tokenizer_path = classifier_dir.join(CLASSIFIER_TOKENIZER_FILENAME);

    if !model_path.exists() || !tokenizer_path.exists() {
        log::warn!(
            "[QueryScopeClassifier] Model assets missing at {:?}. Scope classification will fall back to default (Domain).",
            classifier_dir
        );
        return Ok(false);
    }

    let instance = QueryScopeClassifier::load(classifier_dir)?;
    if SCOPE_CLASSIFIER_INSTANCE.set(instance).is_err() {
        log::warn!("[QueryScopeClassifier] Singleton already initialized.");
    } else {
        log::info!(
            "[QueryScopeClassifier] Successfully initialized ModernBERT MemoryScope classifier from {:?}",
            classifier_dir
        );
    }

    Ok(true)
}

/// Lazily loads the `QueryScopeClassifier` singleton into memory if not already initialized.
pub fn ensure_scope_classifier_loaded() -> Result<bool> {
    if SCOPE_CLASSIFIER_INSTANCE.get().is_some() {
        return Ok(true);
    }

    let models_dir = if let Some(p) = crate::utils::paths::try_get() {
        p.models.clone()
    } else {
        dirs::home_dir()
            .unwrap_or_default()
            .join(".vox")
            .join("models")
    };

    let classifier_dir = models_dir.join("classifier").join(MEMORY_SCOPE_MODEL_DIR);

    init_scope_classifier(&classifier_dir)
}

/// Classifies a turn query into a 4-class `MemoryScope`.
/// Default fallback: `MemoryScope::Domain` if model missing or error occurs.
pub fn classify_scope(text: &str) -> MemoryScope {
    if let Err(e) = ensure_scope_classifier_loaded() {
        log::warn!(
            "[QueryScopeClassifier] Scope classifier lazy initialization error: {}",
            e
        );
    }
    if let Some(classifier) = SCOPE_CLASSIFIER_INSTANCE.get() {
        classifier.classify(text)
    } else {
        MemoryScope::Domain
    }
}

/// Returns true if the query scope classifier model is initialized and ready in memory.
pub fn is_scope_classifier_loaded() -> bool {
    SCOPE_CLASSIFIER_INSTANCE.get().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uninitialized_classifier_fallback() {
        let res = classify_scope("hello how are you");
        if is_scope_classifier_loaded() {
            assert_eq!(res, MemoryScope::ChitChat);
        } else {
            assert_eq!(res, MemoryScope::Domain);
        }
    }

    #[test]
    fn test_classifier_path_constants() {
        assert_eq!(MEMORY_SCOPE_MODEL_DIR, "modernbert_memory_scope");
        assert_eq!(CLASSIFIER_MODEL_FILENAME, "model_quantized.onnx");
        assert_eq!(CLASSIFIER_TOKENIZER_FILENAME, "tokenizer.json");
    }
}
