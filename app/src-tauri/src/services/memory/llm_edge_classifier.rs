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
