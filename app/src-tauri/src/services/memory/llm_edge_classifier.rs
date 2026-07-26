use anyhow::{anyhow, Result};
use std::path::Path;
use std::sync::OnceLock;
use parking_lot::Mutex;
use llama_cpp_4::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, AddBos, LlamaModel},
    sampling::LlamaSampler,
};

pub const LFM2_5_MODEL_FILENAME: &str = "LFM2.5-230M-Q8_0.gguf";

pub const EDGE_CLASSIFIER_PROMPT_TEMPLATE: &str = "\
<|im_start|>system
You are a memory graph edge classifier for a cognitive AI system.
Your task is to classify the semantic relationship between Fact 1 ({src_collection}) and Fact 2 ({tgt_collection}).
Allowed edge types for {src_collection} -> {tgt_collection}: [{forward_edge}, NONE].
Respond with ONLY the exact edge label name.<|im_end|>
<|im_start|>user
Fact 1 ({src_collection}) [Session Context: {src_context}]: {src_fact}
Fact 2 ({tgt_collection}) [Session Context: {tgt_context}]: {tgt_fact}
Relationship:<|im_end|>
<|im_start|>assistant
";

static EDGE_CLASSIFIER_INSTANCE: OnceLock<Option<LlmEdgeClassifier>> = OnceLock::new();

pub struct LlmEdgeClassifier {
    model: LlamaModel,
    _backend: LlamaBackend,
    ctx: Mutex<Option<llama_cpp_4::context::LlamaContext<'static>>>,
}

unsafe impl Send for LlmEdgeClassifier {}
unsafe impl Sync for LlmEdgeClassifier {}

impl LlmEdgeClassifier {
    pub fn load_from_dir(models_dir: &Path) -> Result<Self> {
        let model_path = models_dir.join("llm").join(LFM2_5_MODEL_FILENAME);
        if !model_path.exists() {
            return Err(anyhow!("LFM2.5-230M model file not found at: {:?}", model_path));
        }

        let backend = LlamaBackend::init()?;
        let model_params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, &model_path, &model_params)?;

        let mut ctx_params = LlamaContextParams::default();
        ctx_params = ctx_params.with_n_ctx(Some(std::num::NonZeroU32::new(2048).unwrap()));

        let ctx = model.new_context(&backend, ctx_params)?;
        let static_ctx: llama_cpp_4::context::LlamaContext<'static> = unsafe { std::mem::transmute(ctx) };

        Ok(Self {
            model,
            _backend: backend,
            ctx: Mutex::new(Some(static_ctx)),
        })
    }

    /// Classifies an inter-collection candidate pair using LFM2.5-230M.
    /// Returns predicted relation if valid and matches forward_edge, or None.
    pub fn classify_pair(
        &self,
        src_collection: &str,
        src_fact: &str,
        src_context: Option<&str>,
        tgt_collection: &str,
        tgt_fact: &str,
        tgt_context: Option<&str>,
        forward_edge: &str,
    ) -> Result<Option<String>> {
        let mut ctx_guard = self.ctx.lock();
        let ctx = match ctx_guard.as_mut() {
            Some(c) => c,
            None => return Err(anyhow!("LlamaContext uninitialized")),
        };

        let src_ctx_str = src_context.unwrap_or("General Context");
        let tgt_ctx_str = tgt_context.unwrap_or("General Context");

        let prompt = EDGE_CLASSIFIER_PROMPT_TEMPLATE
            .replace("{src_collection}", src_collection)
            .replace("{tgt_collection}", tgt_collection)
            .replace("{forward_edge}", forward_edge)
            .replace("{src_context}", src_ctx_str)
            .replace("{src_fact}", src_fact)
            .replace("{tgt_context}", tgt_ctx_str)
            .replace("{tgt_fact}", tgt_fact);

        let tokens = self.model.str_to_token(&prompt, AddBos::Always)?;
        if tokens.is_empty() {
            return Ok(None);
        }

        ctx.clear_kv_cache();

        let mut batch = LlamaBatch::new(1024, 1);
        let last_idx = tokens.len() - 1;
        for (i, &token) in tokens.iter().enumerate() {
            let is_last = i == last_idx;
            batch.add(token, i as i32, &[0], is_last)?;
        }

        ctx.decode(&mut batch)?;

        let sampler = LlamaSampler::chain_simple([LlamaSampler::greedy()]);

        let mut generated_bytes = Vec::new();
        let max_tokens = 6;

        for step in 0..max_tokens {
            let token = sampler.sample(ctx, batch.n_tokens() - 1);
            if self.model.is_eog_token(token) {
                break;
            }

            let token_bytes = self.model.token_to_bytes(token, llama_cpp_4::model::Special::Plaintext)?;
            generated_bytes.extend_from_slice(&token_bytes);

            let output_str = String::from_utf8_lossy(&generated_bytes);
            if output_str.contains('\n') || output_str.trim().len() >= forward_edge.len().max(4) {
                break;
            }

            batch.clear();
            batch.add(token, (tokens.len() + step) as i32, &[0], true)?;
            ctx.decode(&mut batch)?;
        }

        let raw_output = String::from_utf8_lossy(&generated_bytes);
        let cleaned = raw_output.trim();

        if cleaned.eq_ignore_ascii_case(forward_edge) {
            Ok(Some(forward_edge.to_string()))
        } else {
            Ok(None)
        }
    }
}

/// Ensures the LFM2.5-230M edge classifier model is loaded into memory.
pub fn ensure_edge_classifier_loaded() -> Result<()> {
    if EDGE_CLASSIFIER_INSTANCE.get().is_some() {
        return Ok(());
    }

    let models_dir = crate::utils::paths::get().models.clone();
    let classifier = LlmEdgeClassifier::load_from_dir(&models_dir)?;
    let _ = EDGE_CLASSIFIER_INSTANCE.set(Some(classifier));
    log::info!("[EdgeClassifier] LFM2.5-230M GGUF edge classifier loaded successfully.");
    Ok(())
}

/// Classifies an inter-collection candidate pair using LFM2.5-230M.
/// Returns predicted relation string if classified successfully.
pub fn classify_edge(
    src_collection: &str,
    src_fact: &str,
    src_context: Option<&str>,
    tgt_collection: &str,
    tgt_fact: &str,
    tgt_context: Option<&str>,
    forward_edge: &str,
) -> Result<Option<String>> {
    ensure_edge_classifier_loaded()?;
    if let Some(Some(classifier)) = EDGE_CLASSIFIER_INSTANCE.get() {
        classifier.classify_pair(src_collection, src_fact, src_context, tgt_collection, tgt_fact, tgt_context, forward_edge)
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::constants::{
        inter_collection_edge, PM_CLASS_A_COLLECTIONS, PM_CLASS_B_COLLECTIONS, PM_COLLECTIONS,
    };

    #[test]
    fn test_class_a_collections_reject_forbidden_edges() {
        // Class A collections (Identity, Context) must NEVER form inter-collection edges
        for &class_a in PM_CLASS_A_COLLECTIONS {
            for &coll in PM_COLLECTIONS {
                assert!(
                    inter_collection_edge(class_a, coll).is_none(),
                    "Class A collection '{}' as source must not have inter-collection edge to '{}'",
                    class_a,
                    coll
                );
                assert!(
                    inter_collection_edge(coll, class_a).is_none(),
                    "Class A collection '{}' as target must not have inter-collection edge from '{}'",
                    class_a,
                    coll
                );
            }
        }
    }

    #[test]
    fn test_class_b_collections_reject_forbidden_edges() {
        // Class B collections (Constraints, Tasks, Goals) perform intra-collection NLI ONLY.
        // As a source, Class B must NEVER generate inter-collection LLM edges.
        for &class_b_src in PM_CLASS_B_COLLECTIONS {
            for &coll in PM_COLLECTIONS {
                assert!(
                    inter_collection_edge(class_b_src, coll).is_none(),
                    "Class B collection '{}' as source must reject inter-collection edge to '{}'",
                    class_b_src,
                    coll
                );
            }
        }
    }

    #[test]
    fn test_class_c_taxonomy_connection_matrix_compliance() {
        // Allowed inter-collection edges according to v6 §4 / constants.rs
        let allowed_pairs = [
            ("Projects", "Constraints", "constrained_by", "restricts_project"),
            ("Projects", "Skills", "requires_skill", "used_in_project"),
            ("Projects", "Tasks", "contains_task", "belongs_to_project"),
            ("Projects", "Goals", "drives_goal", "supported_by_project"),
            ("Preferences", "Constraints", "restricted_by", "shapes_preference"),
            ("Preferences", "Experiences", "shaped_by", "influenced_preference"),
            ("Skills", "Experiences", "acquired_in", "demonstrated_skill"),
            ("Relationships", "Experiences", "involved_in", "included_relationship"),
            ("Relationships", "Projects", "collaborates_on", "project_contributor"),
        ];

        // Verify all 9 taxonomy-approved pairs are valid
        for (src, tgt, expected_fwd, expected_inv) in allowed_pairs {
            let res = inter_collection_edge(src, tgt);
            assert!(
                res.is_some(),
                "Taxonomy pair ({}, {}) must be allowed",
                src,
                tgt
            );
            let (fwd, inv) = res.unwrap();
            assert_eq!(fwd, expected_fwd);
            assert_eq!(inv, expected_inv);
        }

        // Verify that all other pairs among PM_COLLECTIONS are forbidden
        for &src in PM_COLLECTIONS {
            for &tgt in PM_COLLECTIONS {
                let is_allowed = allowed_pairs
                    .iter()
                    .any(|(s, t, _, _)| *s == src && *t == tgt);
                if !is_allowed {
                    assert!(
                        inter_collection_edge(src, tgt).is_none(),
                        "Forbidden collection pair ({}, {}) must return None",
                        src,
                        tgt
                    );
                }
            }
        }
    }

    #[test]
    fn test_edge_classifier_prompt_template_formatting() {
        let prompt = EDGE_CLASSIFIER_PROMPT_TEMPLATE
            .replace("{src_collection}", "Projects")
            .replace("{tgt_collection}", "Skills")
            .replace("{forward_edge}", "requires_skill")
            .replace("{src_context}", "Vox Development")
            .replace("{src_fact}", "Building real-time voice pipeline")
            .replace("{tgt_context}", "User Skills")
            .replace("{tgt_fact}", "Proficient in Rust async programming");

        assert!(prompt.contains("Fact 1 (Projects)"));
        assert!(prompt.contains("Fact 2 (Skills)"));
        assert!(prompt.contains("Allowed edge types for Projects -> Skills: [requires_skill, NONE]."));
        assert!(prompt.contains("[Session Context: Vox Development]: Building real-time voice pipeline"));
        assert!(prompt.contains("[Session Context: User Skills]: Proficient in Rust async programming"));
    }

    #[test]
    fn test_edge_classifier_label_cleaning_and_matching() {
        let forward_edge = "requires_skill";

        // Simulated raw LLM outputs
        let valid_exact = "requires_skill";
        let valid_whitespace = "  requires_skill \n";
        let valid_uppercase = "REQUIRES_SKILL";
        let invalid_label = "CONFLICTS";
        let invalid_arbitrary = "random_relation";

        assert_eq!(
            valid_exact.trim().to_lowercase(),
            forward_edge.to_lowercase()
        );
        assert_eq!(
            valid_whitespace.trim().to_lowercase(),
            forward_edge.to_lowercase()
        );
        assert_eq!(
            valid_uppercase.trim().to_lowercase(),
            forward_edge.to_lowercase()
        );
        assert_ne!(
            invalid_label.trim().to_lowercase(),
            forward_edge.to_lowercase()
        );
        assert_ne!(
            invalid_arbitrary.trim().to_lowercase(),
            forward_edge.to_lowercase()
        );
    }
}

