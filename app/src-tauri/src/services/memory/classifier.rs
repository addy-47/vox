use anyhow::Result;
use query_sieve::{Classification, GenericSemanticClassifier};
use std::path::Path;
use std::sync::OnceLock;

static CLASSIFIER: OnceLock<GenericSemanticClassifier> = OnceLock::new();

/// Initializes the `GenericSemanticClassifier` singleton.
/// Reads model from `~/.vox/models/classifier/distilbert-query-classifier/model_quantized.onnx`
/// and tokenizer from `tokenizer.json`.
/// If files do not exist, logs a warning and gracefully skips initialization.
pub fn init_classifier(classifier_dir: &Path) -> Result<()> {
    let model_path = classifier_dir.join("model_quantized.onnx");
    let tokenizer_path = classifier_dir.join("tokenizer.json");

    if !model_path.exists() || !tokenizer_path.exists() {
        log::warn!(
            "[QueryClassifier] Model files missing at {:?}. Classification will fall back to default (SEMANTIC).",
            classifier_dir
        );
        return Ok(());
    }

    let classifier = GenericSemanticClassifier::load(
        model_path.to_string_lossy().as_ref(),
        tokenizer_path.to_string_lossy().as_ref(),
    )?;

    if CLASSIFIER.set(classifier).is_err() {
        log::warn!("[QueryClassifier] Classifier singleton already set.");
    } else {
        log::info!(
            "[QueryClassifier] Successfully loaded DistilBERT query classifier from {:?}",
            classifier_dir
        );
    }

    Ok(())
}

/// Lazily loads the `GenericSemanticClassifier` singleton into RAM when needed (e.g. on pipeline engage).
pub fn ensure_classifier_loaded() -> Result<()> {
    if CLASSIFIER.get().is_some() {
        return Ok(());
    }
    let models_dir = if let Some(p) = crate::utils::paths::try_get() {
        p.models.clone()
    } else {
        dirs::home_dir().unwrap_or_default().join(".vox").join("models")
    };
    let classifier_dir = models_dir
        .join("classifier")
        .join("distilbert-query-classifier");
    init_classifier(&classifier_dir)
}

/// Classifies a string query as Generic or Semantic.
/// Returns `Classification::Semantic` if classifier is not loaded (safe default for knowledge retention).
pub fn classify_query(text: &str) -> Classification {
    let _ = ensure_classifier_loaded();
    if let Some(classifier) = CLASSIFIER.get() {
        match classifier.classify(text) {
            Ok(result) => result,
            Err(e) => {
                log::warn!(
                    "[QueryClassifier] Classification error for '{}': {}. Defaulting to SEMANTIC.",
                    text, e
                );
                Classification::Semantic
            }
        }
    } else {
        // Fallback: Default to Semantic if classifier model is not initialized
        Classification::Semantic
    }
}

/// Returns true if the query classifier model is loaded and ready.
pub fn is_classifier_loaded() -> bool {
    CLASSIFIER.get().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uninitialized_classifier_fallback() {
        // Before initialization, classify_query returns Classification::Semantic by default
        let res = classify_query("hello how are you");
        assert_eq!(res, Classification::Semantic);
    }
}
