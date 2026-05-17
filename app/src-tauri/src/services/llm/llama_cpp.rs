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
use crate::services::traits;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFamily {
    Gemma,
    Qwen,
    Llama3,
    Nemotron,
    Unknown,
}

impl ModelFamily {
    pub fn detect(path: &Path) -> Self {
        let filename = path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        if filename.contains("gemma") {
            ModelFamily::Gemma
        } else if filename.contains("qwen") {
            ModelFamily::Qwen
        } else if filename.contains("llama") {
            ModelFamily::Llama3
        } else if filename.contains("nemotron") {
            ModelFamily::Nemotron
        } else {
            ModelFamily::Unknown
        }
    }

    pub fn format_prompt(&self, text: &str, system_prompt: &str) -> String {
        match self {
            ModelFamily::Gemma => {
                // Gemma-4 specific fine-tuned chat template
                format!(
                    "<|turn>system {}<turn|>\n<|turn>user {}<turn|>\n<|turn>model\n",
                    system_prompt, text
                )
            }
            ModelFamily::Qwen => {
                // Qwen 2.5 / 3 / 3.5 ChatML template
                format!(
                    "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
                    system_prompt, text
                )
            }
            ModelFamily::Llama3 => {
                // Llama 3 / 3.2 Instruct official template
                format!(
                    "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
                    system_prompt, text
                )
            }
            ModelFamily::Nemotron => {
                // NVIDIA Nemotron3 Nano template
                format!(
                    "<extra_id_0>System\n{}\n<extra_id_1>User\n{}\n<extra_id_1>Assistant\n",
                    system_prompt, text
                )
            }
            ModelFamily::Unknown => {
                // Direct fallback
                format!(
                    "System: {}\nUser: {}\nAssistant: ",
                    system_prompt, text
                )
            }
        }
    }

    pub fn stop_sequences(&self) -> &'static [&'static str] {
        match self {
            ModelFamily::Gemma => &[
                "<end",
                "<eos>",
                "<|turn>",
                "turn|>"
            ],
            ModelFamily::Qwen => &[
                "<|im_end|>",
                "<|im_start|>",
                "</think>",
                "<|turn|>"
            ],
            ModelFamily::Llama3 => &[
                "<|eot_id|>",
                "<|end_of_text|>"
            ],
            ModelFamily::Nemotron => &[
                "<extra_id_1>",
                "<extra_id_0>"
            ],
            ModelFamily::Unknown => &[
                "\nUser:",
                "\nSystem:"
            ],
        }
    }

    pub fn strip_tags(&self, text: &str) -> String {
        let mut cleaned = text.to_string();
        
        match self {
            ModelFamily::Gemma => {
                let re_tags = regex::Regex::new(r"<\|turn>|<turn\|>|<channel\|>|system\s*\n|user\s*\n|model\s*\n").unwrap();
                cleaned = re_tags.replace_all(&cleaned, "").to_string();
                if cleaned.contains("<end") || cleaned.contains("<eos>") {
                    log::warn!("[LLM] Possible leaked eos tag detected: {:?}", cleaned);
                    return "".to_string();
                }
            }
            ModelFamily::Qwen => {
                // Strip entire <think>...</think> block including inner text
                let re_think = regex::Regex::new(r"(?s)<think>.*?</think>").unwrap();
                cleaned = re_think.replace_all(&cleaned, "").to_string();

                let tags = vec![
                    "<|im_start|>", "<|im_end|>", "<|turn|>",
                    "user\n", "assistant\n", "system\n", "thought\n"
                ];
                for tag in tags {
                    cleaned = cleaned.replace(tag, "");
                }
                
                // Backup cleanup of open/close tags
                cleaned = cleaned.replace("<think>", "").replace("</think>", "");
            }
            ModelFamily::Llama3 => {
                let tags = vec![
                    "<|begin_of_text|>", "<|start_header_id|>", "<|end_header_id|>", "<|eot_id|>",
                    "user\n", "assistant\n", "system\n"
                ];
                for tag in tags {
                    cleaned = cleaned.replace(tag, "");
                }
            }
            ModelFamily::Nemotron => {
                let tags = vec![
                    "<extra_id_0>", "<extra_id_1>",
                    "User\n", "Assistant\n", "System\n"
                ];
                for tag in tags {
                    cleaned = cleaned.replace(tag, "");
                }
            }
            ModelFamily::Unknown => {}
        }

        cleaned.trim().to_string()
    }
}

pub struct LlmWorker {
    model: LlamaModel,
    backend: &'static LlamaBackend,
    ctx_size: u32,
    n_threads: u32,
    family: ModelFamily,
}

impl LlmWorker {
    pub fn new(model_path: &Path, ctx_size: u32, n_threads: u32) -> Result<Self> {
        log::info!("[LLM] >>> Initializing llama.cpp backend...");

        let resolved = model_path.canonicalize()
            .unwrap_or_else(|_| model_path.to_path_buf());

        if !resolved.exists() {
            return Err(anyhow!("[LLM] GGUF not found: {:?}", resolved));
        }

        let family = ModelFamily::detect(&resolved);
        log::info!("[LLM] Detected model family: {:?}", family);

        static BACKEND: std::sync::OnceLock<LlamaBackend> = std::sync::OnceLock::new();
        let backend = BACKEND.get_or_init(|| {
            let mut b = LlamaBackend::init().expect("[LLM] Failed to initialize global llama.cpp backend");
            b.void_logs();
            b
        });

        let model_params = LlamaModelParams::default()
            .with_n_gpu_layers(0)
            .with_use_mlock(true);

        log::info!("[LLM] Loading GGUF: {:?}", resolved);
        let model = LlamaModel::load_from_file(backend, &resolved, &model_params)
            .map_err(|e| anyhow!("[LLM] Failed to load model: {}", e))?;

        log::info!("[LLM] Model loaded. family={:?} ctx_size={} n_threads={}", family, ctx_size, n_threads);
        Ok(Self { 
            model, 
            backend, 
            ctx_size, 
            n_threads,
            family,
        })
    }

    pub fn run_loop(
        &self,
        rx: std::sync::mpsc::Receiver<super::actor::LlmCommand>,
        tx: std::sync::mpsc::Sender<VoxEvent>,
    ) {
        log::info!("[LLM Worker] Persistent loop started.");
        
        while let Ok(cmd) = rx.recv() {
            match cmd {
                super::actor::LlmCommand::Generate { text, system_prompt, turn_id, cancel_flag } => {
                    use crate::services::traits::LlmEngine as _;
                    if let Err(e) = self.generate(&text, &system_prompt, turn_id, &cancel_flag, &tx) {
                        log::error!("[LLM Worker] Generation error (turn {}): {}", turn_id, e);
                        let _ = tx.send(VoxEvent::Error { 
                            turn_id, 
                            message: e.to_string() 
                        });
                    }
                }
                super::actor::LlmCommand::Shutdown => {
                    log::info!("[LLM Worker] Shutdown command received. Exiting loop.");
                    break;
                }
            }
        }
        
        log::info!("[LLM Worker] Loop exited. Model will be dropped.");
    }
}

impl traits::LlmEngine for LlmWorker {
    fn generate(
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

        let prompt = self.family.format_prompt(user_text, system_prompt);
        log::info!("[LLM] Formatted Prompt:\n{}", prompt);

        let tokens = self.model
            .str_to_token(&prompt, AddBos::Always)
            .map_err(|e| anyhow!("[LLM] Tokenize failed: {}", e))?;

        if tokens.is_empty() {
            return Err(anyhow!("[LLM] Empty token list for prompt"));
        }

        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(NonZeroU32::new(self.ctx_size).unwrap()))
            .with_n_threads(self.n_threads as i32)
            .with_n_threads_batch(self.n_threads as i32)
            .with_n_batch(512)
            .with_n_ubatch(512);

        let mut ctx = self.model
            .new_context(self.backend, ctx_params)
            .map_err(|e| anyhow!("[LLM] Context creation failed: {}", e))?;

        ctx.clear_kv_cache();

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

        let mut raw_gen_buf = String::new();
        let stop_seqs = self.family.stop_sequences();

        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                log::info!("[LLM] Cancelled at token {} (turn: {})", n_cur, turn_id);
                ctx.clear_kv_cache();
                let _ = tx.send(VoxEvent::Cancelled { turn_id });
                return Ok(());
            }

            let candidates = ctx.candidates_ith(batch.n_tokens() - 1);
            let mut candidates_p = LlamaTokenDataArray::from_iter(candidates, false);
            let token = candidates_p.sample_token_greedy();

            if self.model.is_eog_token(token) {
                log::info!("[LLM] EOS reached (turn: {})", turn_id);
                break;
            }

            let token_str = self.model
                .token_to_piece(token, &mut decoder, false, None)
                .unwrap_or_default();

            raw_gen_buf.push_str(&token_str);

            // Bulletproof stop sequence detection
            let mut stop_triggered = false;
            for stop in stop_seqs {
                if let Some(pos) = raw_gen_buf.find(stop) {
                    log::info!("[LLM] Stop sequence {:?} triggered! Terminating generation.", stop);
                    // Truncate the buffer at the stop sequence and flush anything remaining
                    let clean_remaining = &raw_gen_buf[..pos];
                    let cleaned = self.family.strip_tags(clean_remaining);
                    if !cleaned.is_empty() {
                        let _ = tx.send(VoxEvent::LlmToken {
                            turn_id,
                            token: cleaned,
                        });
                    }
                    stop_triggered = true;
                    break;
                }
            }

            if stop_triggered {
                break;
            }

            if ttft.is_none() {
                ttft = Some(start_time.elapsed());
            }

            let cleaned = self.family.strip_tags(&token_str);
            if !cleaned.is_empty() {
                let _ = tx.send(VoxEvent::LlmToken {
                    turn_id,
                    token: cleaned,
                });
            }
            tokens_generated += 1;

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
}
