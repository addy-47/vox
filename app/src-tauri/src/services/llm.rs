//! LLM Runtime — llama.cpp inference worker.
//!
//! Runs on a dedicated OS thread (never tokio). Uses llama-cpp-2 Rust bindings
//! to load a GGUF model and stream tokens. Cancellation is checked on every
//! token via an `Arc<AtomicBool>` — the only safe way to interrupt a blocking
//! C++ inference loop.

use anyhow::{anyhow, Result};
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, AddBos, LlamaModel},
    token::data_array::LlamaTokenDataArray,
};

use crate::core::events::VoxEvent;

// ─── System Prompt ────────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = "\
You are Vox, a concise real-time voice assistant. \
Respond in short, natural sentences. \
Be direct and conversational. \
Never use markdown, bullet points, or formatting.";

// ─── LLM Worker ───────────────────────────────────────────────────────────────

/// Owns a loaded llama.cpp model and context.
///
/// Must live on the same OS thread where it was created (llama.cpp is not Send).
/// Spawn with `std::thread::spawn` — never tokio.
pub struct LlmWorker {
    model:   LlamaModel,
    backend: LlamaBackend,
    ctx_size: u32,
    n_threads: u32,
}

impl LlmWorker {
    /// Load the GGUF model. Blocking — run on a dedicated OS thread.
    pub fn new(model_path: &Path, ctx_size: u32, n_threads: u32) -> Result<Self> {
        log::info!("[LLM] >>> Initializing llama.cpp backend...");

        // Resolve symlinks (handles HuggingFace hub symlink layouts)
        let resolved = model_path.canonicalize()
            .unwrap_or_else(|_| model_path.to_path_buf());

        if !resolved.exists() {
            return Err(anyhow!("[LLM] GGUF not found: {:?}", resolved));
        }

        let backend = LlamaBackend::init()?;

        let model_params = LlamaModelParams::default()
            .with_n_gpu_layers(0); // CPU-only per architecture constraint

        log::info!("[LLM] Loading GGUF: {:?}", resolved);
        let model = LlamaModel::load_from_file(&backend, &resolved, &model_params)
            .map_err(|e| anyhow!("[LLM] Failed to load model: {}", e))?;

        log::info!("[LLM] Model loaded. ctx_size={} n_threads={}", ctx_size, n_threads);
        Ok(Self { model, backend, ctx_size, n_threads })
    }

    /// Generate a response to `user_text`, streaming tokens via `tx`.
    ///
    /// Checks `cancel_flag` on every token — aborts cleanly if set.
    /// Returns when generation finishes, EOS is reached, or cancellation fires.
    pub fn generate(
        &self,
        user_text: &str,
        session_id: u32,
        cancel_flag: &Arc<AtomicBool>,
        tx: &Sender<VoxEvent>,
    ) -> Result<()> {
        // Build chat-formatted prompt (Gemma instruct format)
        let prompt = format!(
            "<start_of_turn>system\n{}<end_of_turn>\n\
             <start_of_turn>user\n{}<end_of_turn>\n\
             <start_of_turn>model\n",
            SYSTEM_PROMPT, user_text
        );

        // Tokenize
        let tokens = self.model
            .str_to_token(&prompt, AddBos::Always)
            .map_err(|e| anyhow!("[LLM] Tokenize failed: {}", e))?;

        if tokens.is_empty() {
            return Err(anyhow!("[LLM] Empty token list for prompt"));
        }

        // Build context
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(NonZeroU32::new(self.ctx_size).unwrap()))
            .with_n_threads(self.n_threads as i32)
            .with_n_threads_batch(self.n_threads as i32);

        let mut ctx = self.model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| anyhow!("[LLM] Context creation failed: {}", e))?;

        // Prefill batch
        let max_tokens = tokens.len() + 512;
        let mut batch = LlamaBatch::new(max_tokens, 1);

        for (i, &tok) in tokens.iter().enumerate() {
            let is_last = i == tokens.len() - 1;
            batch.add(tok, i as i32, &[0], is_last)
                .map_err(|e| anyhow!("[LLM] Batch add failed: {}", e))?;
        }

        ctx.decode(&mut batch)
            .map_err(|e| anyhow!("[LLM] Prefill decode failed: {}", e))?;

        let mut n_cur = tokens.len() as i32;

        let mut decoder = encoding_rs::UTF_8.new_decoder();

        log::info!("[LLM] >>> Generating (session: {})...", session_id);

        // ── Decode loop ───────────────────────────────────────────────────────
        loop {
            // Atomic cancellation check — first thing every iteration
            if cancel_flag.load(Ordering::Relaxed) {
                log::info!("[LLM] Cancelled at token {} (session: {})", n_cur, session_id);
                let _ = tx.blocking_send(VoxEvent::Cancelled { session_id });
                return Ok(());
            }

            // Sample next token (greedy) from the last evaluated batch
            let candidates = ctx.candidates_ith(batch.n_tokens() as i32 - 1);
            let mut candidates_p = LlamaTokenDataArray::from_iter(candidates, false);
            let token = candidates_p.sample_token_greedy();

            // End of generation
            if self.model.is_eog_token(token) {
                log::info!("[LLM] EOS reached (session: {})", session_id);
                break;
            }

            // Decode token to string using a fresh decoder or existing one if we tracked it
            // For simplicity in this loop, we can just use a local decoder for each token
            // Wait, multi-byte characters span multiple tokens, so we should keep the decoder in the generate scope
            let token_str = self.model
                .token_to_piece(token, &mut decoder, false, None)
                .unwrap_or_default();

            if !token_str.is_empty() {
                let _ = tx.blocking_send(VoxEvent::LlmToken {
                    session_id,
                    token: token_str,
                });
            }

            // Advance
            if n_cur >= self.ctx_size as i32 {
                log::warn!("[LLM] Context limit reached ({} tokens). Stopping generation.", self.ctx_size);
                break;
            }

            batch.clear();
            batch.add(token, n_cur, &[0], true)
                .map_err(|e| anyhow!("[LLM] Batch add (gen) failed: {}", e))?;
            n_cur += 1;

            ctx.decode(&mut batch)
                .map_err(|e| anyhow!("[LLM] Decode step failed: {}", e))?;
        }

        let _ = tx.blocking_send(VoxEvent::LlmFinished { session_id });
        log::info!("[LLM] Generation complete (session: {}, tokens: {})", session_id, n_cur);
        Ok(())
    }
}
