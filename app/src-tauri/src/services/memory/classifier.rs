use anyhow::{anyhow, Result};
use query_sieve::{Classification, GenericSemanticClassifier};
use std::path::Path;
use std::sync::OnceLock;

pub const CLASSIFIER_MODEL_DIR: &str = "distilbert-query-classifier";
pub const CLASSIFIER_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const CLASSIFIER_TOKENIZER_FILENAME: &str = "tokenizer.json";

/// Engine wrapper for the DistilBERT Query Classifier.
pub struct QueryClassifier {
    engine: GenericSemanticClassifier,
}

impl QueryClassifier {
    /// Loads the DistilBERT model and tokenizer from the specified directory.
    pub fn load(classifier_dir: &Path) -> Result<Self> {
        let model_path = classifier_dir.join(CLASSIFIER_MODEL_FILENAME);
        let tokenizer_path = classifier_dir.join(CLASSIFIER_TOKENIZER_FILENAME);

        if !model_path.exists() || !tokenizer_path.exists() {
            return Err(anyhow!(
                "Classifier assets missing at {:?} (model: {}, tokenizer: {})",
                classifier_dir,
                CLASSIFIER_MODEL_FILENAME,
                CLASSIFIER_TOKENIZER_FILENAME
            ));
        }

        let engine = GenericSemanticClassifier::load(
            model_path.to_string_lossy().as_ref(),
            tokenizer_path.to_string_lossy().as_ref(),
        )?;

        Ok(Self { engine })
    }

    /// Classifies an input query into `Classification::Generic` or `Classification::Semantic`.
    pub fn classify(&self, text: &str) -> Classification {
        match self.engine.classify(text) {
            Ok(result) => result,
            Err(e) => {
                log::warn!(
                    "[QueryClassifier] Classification error for query '{}': {}. Defaulting to SEMANTIC.",
                    text,
                    e
                );
                Classification::Semantic
            }
        }
    }
}

static CLASSIFIER_INSTANCE: OnceLock<QueryClassifier> = OnceLock::new();

/// Initializes the `QueryClassifier` singleton from the specified model directory.
/// Returns `Ok(true)` if loaded, `Ok(false)` if model files do not exist.
pub fn init_classifier(classifier_dir: &Path) -> Result<bool> {
    let model_path = classifier_dir.join(CLASSIFIER_MODEL_FILENAME);
    let tokenizer_path = classifier_dir.join(CLASSIFIER_TOKENIZER_FILENAME);

    if !model_path.exists() || !tokenizer_path.exists() {
        log::warn!(
            "[QueryClassifier] Model assets missing at {:?}. Classification will fall back to default (SEMANTIC).",
            classifier_dir
        );
        return Ok(false);
    }

    let instance = QueryClassifier::load(classifier_dir)?;
    if CLASSIFIER_INSTANCE.set(instance).is_err() {
        log::warn!("[QueryClassifier] Singleton already initialized.");
    } else {
        log::info!(
            "[QueryClassifier] Successfully initialized DistilBERT query classifier from {:?}",
            classifier_dir
        );
    }

    Ok(true)
}

/// Lazily loads the `QueryClassifier` singleton into memory if not already initialized.
pub fn ensure_classifier_loaded() -> Result<bool> {
    if CLASSIFIER_INSTANCE.get().is_some() {
        return Ok(true);
    }

    let models_dir = if let Some(p) = crate::utils::paths::try_get() {
        p.models.clone()
    } else {
        dirs::home_dir().unwrap_or_default().join(".vox").join("models")
    };

    let classifier_dir = models_dir
        .join("classifier")
        .join(CLASSIFIER_MODEL_DIR);

    init_classifier(&classifier_dir)
}

/// Classifies a string query as Generic or Semantic.
/// Returns `Classification::Semantic` if classifier is not loaded (safe fallback for memory routing).
pub fn classify_query(text: &str) -> Classification {
    let _ = ensure_classifier_loaded();
    if let Some(classifier) = CLASSIFIER_INSTANCE.get() {
        classifier.classify(text)
    } else {
        Classification::Semantic
    }
}

/// Returns true if the query classifier model is initialized and ready in memory.
pub fn is_classifier_loaded() -> bool {
    CLASSIFIER_INSTANCE.get().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uninitialized_classifier_fallback() {
        let res = classify_query("hello how are you");
        assert_eq!(res, Classification::Semantic);
    }

    #[test]
    fn test_classifier_path_constants() {
        assert_eq!(CLASSIFIER_MODEL_DIR, "distilbert-query-classifier");
        assert_eq!(CLASSIFIER_MODEL_FILENAME, "model_quantized.onnx");
        assert_eq!(CLASSIFIER_TOKENIZER_FILENAME, "tokenizer.json");
    }
}
