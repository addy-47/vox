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

use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, AddBos, LlamaModel},
    token::data_array::LlamaTokenDataArray,
};

use crate::core::events::VoxEvent;

// ─── LLM Worker Entry Point ──────────────────────────────────────────────────

pub fn spawn_llm_worker(
    app: tauri::AppHandle,
    rx: std::sync::mpsc::Receiver<LlmCommand>,
    model_path: std::path::PathBuf,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
    is_loaded: Arc<AtomicBool>,
) {
    use tauri::Emitter;
    let _ = app.emit(crate::core::constants::EVENT_MODEL_LOADING, "LLM");

    let settings = {
        // We need to know ctx_size and threads
        // This is a bit awkward as we don't have AppState here.
        // We'll use defaults if not provided, or better, the orchestrator should provide them.
        // For now, we'll keep the existing LlmWorker::new logic.
        (2048, 4) // Defaults
    };

    let worker = match LlmWorker::new(&model_path, settings.0, settings.1) {
        Ok(w) => {
            is_loaded.store(true, Ordering::Relaxed);
            let _ = app.emit(crate::core::constants::EVENT_MODEL_READY, "LLM");
            w
        },
        Err(e) => {
            log::error!("[LLM] CRITICAL: Failed to load model: {}", e);
            let _ = app.emit(crate::core::constants::EVENT_MODEL_FAILED, format!("LLM: {}", e));
            return;
        }
    };

    worker.run_loop(rx, event_tx);
    is_loaded.store(false, Ordering::Relaxed);
}

// ─── LLM Worker ───────────────────────────────────────────────────────────────

/// Commands sent from the Pipeline Orchestrator to the LLM background thread.
pub enum LlmCommand {
    /// Start generating a response to `text`.
    Generate {
        text: String,
        system_prompt: String,
        turn_id: u32,
        cancel_flag: Arc<AtomicBool>,
    },
    /// Stop the background thread and deallocate the model.
    Shutdown,
}

/// Owns a loaded llama.cpp model and context.
///
/// Must live on the same OS thread where it was created (llama.cpp is not Send).
/// Spawn with `std::thread::spawn` — never tokio.
pub struct LlmWorker {
    model:    LlamaModel,
    backend:  &'static LlamaBackend,
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

        // Initialize global backend exactly once
        static BACKEND: std::sync::OnceLock<LlamaBackend> = std::sync::OnceLock::new();
        let backend = BACKEND.get_or_init(|| {
            let mut b = LlamaBackend::init().expect("[LLM] Failed to initialize global llama.cpp backend");
            b.void_logs();
            b
        });

        let model_params = LlamaModelParams::default()
            .with_n_gpu_layers(0); // CPU-only per architecture constraint

        log::info!("[LLM] Loading GGUF: {:?}", resolved);
        let model = LlamaModel::load_from_file(&backend, &resolved, &model_params)
            .map_err(|e| anyhow!("[LLM] Failed to load model: {}", e))?;

        log::info!("[LLM] Model loaded. ctx_size={} n_threads={}", ctx_size, n_threads);
        Ok(Self { 
            model, 
            backend, 
            ctx_size, 
            n_threads,
        })
    }

    /// Persistent loop running on a dedicated OS thread.
    /// Listens for `LlmCommand` and executes them.
    pub fn run_loop(
        &self,
        rx: std::sync::mpsc::Receiver<LlmCommand>,
        tx: std::sync::mpsc::Sender<VoxEvent>,
    ) {
        log::info!("[LLM Worker] Persistent loop started.");
        
        while let Ok(cmd) = rx.recv() {
            match cmd {
                LlmCommand::Generate { text, system_prompt, turn_id, cancel_flag } => {
                    if let Err(e) = self.generate(&text, &system_prompt, turn_id, &cancel_flag, &tx) {
                        log::error!("[LLM Worker] Generation error (turn {}): {}", turn_id, e);
                        let _ = tx.send(VoxEvent::Error { 
                            turn_id, 
                            message: e.to_string() 
                        });
                    }
                }
                LlmCommand::Shutdown => {
                    log::info!("[LLM Worker] Shutdown command received. Exiting loop.");
                    break;
                }
            }
        }
        
        log::info!("[LLM Worker] Loop exited. Model will be dropped.");
    }

    /// Generate a response to `user_text`, streaming tokens via `tx`.
    ///
    /// Checks `cancel_flag` on every token — aborts cleanly if set.
    /// Returns when generation finishes, EOS is reached, or cancellation fires.
    pub fn generate(
        &self,
        user_text: &str,
        system_prompt: &str,
        turn_id: u32,
        cancel_flag: &Arc<AtomicBool>,
        tx: &std::sync::mpsc::Sender<VoxEvent>,
    ) -> Result<()> {
        let start_time = std::time::Instant::now();
        let mut ttft: Option<std::time::Duration> = None;
        let mut tokens_generated = 0;

        // Gemma 2/4 Instruct Format:
        // <start_of_turn>user\n{system}\n\n{user}<end_of_turn>\n<start_of_turn>model\n
        let prompt = self.format_prompt(user_text, system_prompt);

        // Tokenize
        let tokens = self.model
            .str_to_token(&prompt, AddBos::Always)
            .map_err(|e| anyhow!("[LLM] Tokenize failed: {}", e))?;

        if tokens.is_empty() {
            return Err(anyhow!("[LLM] Empty token list for prompt"));
        }

        // Build context (Directive: Optimized 8GB Memory Baseline)
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(NonZeroU32::new(self.ctx_size).unwrap()))
            .with_n_threads(self.n_threads as i32)
            .with_n_threads_batch(self.n_threads as i32);

        let mut ctx = self.model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| anyhow!("[LLM] Context creation failed: {}", e))?;

        // Explicitly clear KV cache to prevent memory leaks from accumulation
        ctx.clear_kv_cache();

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

        log::info!("[LLM] >>> Generating (turn: {})...", turn_id);

        // ── Decode loop ───────────────────────────────────────────────────────
        loop {
            // Atomic cancellation check
            if cancel_flag.load(Ordering::Relaxed) {
                log::info!("[LLM] Cancelled at token {} (turn: {})", n_cur, turn_id);
                ctx.clear_kv_cache();
                let _ = tx.send(VoxEvent::Cancelled { turn_id });
                return Ok(());
            }

            // Sample next token
            let candidates = ctx.candidates_ith(batch.n_tokens() as i32 - 1);
            let mut candidates_p = LlamaTokenDataArray::from_iter(candidates, false);
            let token = candidates_p.sample_token_greedy();

            // End of generation
            if self.model.is_eog_token(token) {
                log::info!("[LLM] EOS reached (turn: {})", turn_id);
                break;
            }

            // Record TTFT
            if ttft.is_none() {
                ttft = Some(start_time.elapsed());
            }

            // Decode token to string
            let token_str = self.model
                .token_to_piece(token, &mut decoder, false, None)
                .unwrap_or_default();

            let cleaned = Self::strip_tags(&token_str);
            if !cleaned.is_empty() {
                let _ = tx.send(VoxEvent::LlmToken {
                    turn_id,
                    token: cleaned,
                });
            }
            tokens_generated += 1;

            // Advance
            if n_cur >= self.ctx_size as i32 {
                log::warn!("[LLM] Context limit reached ({} tokens).", self.ctx_size);
                break;
            }

            batch.clear();
            batch.add(token, n_cur, &[0], true)
                .map_err(|e| anyhow!("[LLM] Batch add (gen) failed: {}", e))?;
            n_cur += 1;

            if n_cur > (tokens.len() as i32 + 512) {
                log::warn!("[LLM] Safety limit reached (512 tokens).");
                break;
            }

            ctx.decode(&mut batch)
                .map_err(|e| anyhow!("[LLM] Decode step failed: {}", e))?;
        }

        ctx.clear_kv_cache();

        let elapsed = start_time.elapsed().as_secs_f32();
        let tps = tokens_generated as f32 / elapsed;
        
        log::info!(
            "[LLM] Generation complete (turn: {}). Tokens: {}, TTFT: {:?}, TPS: {:.2}",
            turn_id, tokens_generated, ttft.unwrap_or_default(), tps
        );

        let _ = tx.send(VoxEvent::LlmFinished { turn_id });
        Ok(())
    }

    fn format_prompt(&self, text: &str, system_prompt: &str) -> String {
        // Gemma 4 E2B-it (March 2026) uses <|turn>role<turn|> format.
        // It supports a native system role.
        format!(
            "<|turn>system {}<turn|>\n<|turn>user {}<turn|>\n<|turn>model\n",
            system_prompt, text
        )
    }

    fn strip_tags(text: &str) -> String {
        // Gemma 4 uses <|turn>role, <turn|>, and <|channel>thought blocks.
        // We do NOT strip <|channel> here because pipeline.rs uses it to detect "Thinking" vs "Speaking" state.
        let mut cleaned = text.to_string();
        
        // 1. Remove markers and common role labels (except <|channel>)
        let re_tags = regex::Regex::new(r"<\|turn>|<turn\|>|<channel\|>|system\n|user\n|model\n").unwrap();
        cleaned = re_tags.replace_all(&cleaned, "").to_string();
        
        // DO NOT drop tokens just because they have `<|` or `<start` because `<|channel>` is a valid token we need!
        // We only strip `<end>` or `<eos>` if they somehow leak
        if cleaned.contains("<end") || cleaned.contains("<eos>") {
             log::warn!("[LLM] Possible leaked eos tag detected: {:?}", cleaned);
             return "".to_string();
        }
        
        cleaned
    }
}
