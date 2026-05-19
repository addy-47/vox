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
    sampling::LlamaSampler,
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

    pub fn format_system_prompt(&self, system_prompt: &str) -> String {
        match self {
            ModelFamily::Gemma => {
                format!("<|turn>system {}<turn|>\n", system_prompt)
            }
            ModelFamily::Qwen => {
                format!("<|im_start|>system\n{}<|im_end|>\n", system_prompt)
            }
            ModelFamily::Llama3 => {
                format!(
                    "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n{}<|eot_id|>",
                    system_prompt
                )
            }
            ModelFamily::Nemotron => {
                format!("<extra_id_0>System\n{}\n", system_prompt)
            }
            ModelFamily::Unknown => {
                format!("System: {}\n", system_prompt)
            }
        }
    }

    pub fn format_user_prompt(&self, text: &str) -> String {
        match self {
            ModelFamily::Gemma => {
                format!("<|turn>user {}<turn|>\n<|turn>model\n", text)
            }
            ModelFamily::Qwen => {
                format!("<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n", text)
            }
            ModelFamily::Llama3 => {
                format!(
                    "<|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
                    text
                )
            }
            ModelFamily::Nemotron => {
                format!("<extra_id_1>User\n{}\n<extra_id_1>Assistant\n", text)
            }
            ModelFamily::Unknown => {
                format!("User: {}\nAssistant: ", text)
            }
        }
    }

    pub fn format_prompt(&self, text: &str, system_prompt: &str) -> String {
        format!("{}{}", self.format_system_prompt(system_prompt), self.format_user_prompt(text))
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
                "<|turn|>",
                "<|endoftext|>",
                "<|end|>"
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
                static RE_GEMMA_TAGS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
                let re_tags = RE_GEMMA_TAGS.get_or_init(|| regex::Regex::new(r"<\|turn>|<turn\|>|<channel\|>|system\s*\n|user\s*\n|model\s*\n").unwrap());
                cleaned = re_tags.replace_all(&cleaned, "").to_string();
                if cleaned.contains("<end") || cleaned.contains("<eos>") {
                    log::warn!("[LLM] Possible leaked eos tag detected: {:?}", cleaned);
                    return "".to_string();
                }
            }
            ModelFamily::Qwen => {
                static RE_QWEN_THINK: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
                let re_think = RE_QWEN_THINK.get_or_init(|| regex::Regex::new(r"(?s)<think>.*?</think>").unwrap());
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

        cleaned
    }
}

struct CacheState {
    system_prompt: String,
    system_tokens_len: usize,
}

pub struct LlmWorker {
    model: LlamaModel,
    _backend: &'static LlamaBackend,
    ctx_size: u32,
    _n_threads: u32,
    family: ModelFamily,
    ctx: std::sync::Mutex<Option<llama_cpp_2::context::LlamaContext<'static>>>,
    cache_state: std::sync::Mutex<Option<CacheState>>,
}

unsafe impl Send for LlmWorker {}
unsafe impl Sync for LlmWorker {}

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
            _backend: backend, 
            ctx_size, 
            _n_threads: n_threads,
            family,
            ctx: std::sync::Mutex::new(None),
            cache_state: std::sync::Mutex::new(None),
        })
    }

    fn token_to_bytes(&self, token: llama_cpp_2::token::LlamaToken) -> Vec<u8> {
        match self.model.token_to_piece_bytes(token, 8, false, None) {
            Ok(bytes) => bytes,
            Err(llama_cpp_2::TokenToStringError::InsufficientBufferSpace(i)) => {
                self.model.token_to_piece_bytes(
                    token,
                    (-i) as usize,
                    false,
                    None
                ).unwrap_or_default()
            }
            Err(_) => Vec::new(),
        }
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

        let mut ctx_lock = self.ctx.lock().map_err(|_| anyhow!("Failed to lock context"))?;
        if ctx_lock.is_none() {
            log::info!("[LLM] Lazy initializing LlamaContext on stable execution address...");
            let ctx_params = LlamaContextParams::default()
                .with_n_ctx(Some(NonZeroU32::new(self.ctx_size).unwrap()))
                .with_n_threads(self._n_threads as i32)
                .with_n_threads_batch(self._n_threads as i32)
                .with_n_batch(512)
                .with_n_ubatch(512);

            let ctx = self.model.new_context(self._backend, ctx_params)
                .map_err(|e| anyhow!("[LLM] Lazy context creation failed: {}", e))?;

            let static_ctx: llama_cpp_2::context::LlamaContext<'static> = unsafe { std::mem::transmute(ctx) };
            *ctx_lock = Some(static_ctx);
        }
        let ctx = ctx_lock.as_mut().unwrap();

        let mut cache_lock = self.cache_state.lock().map_err(|_| anyhow!("Failed to lock cache state"))?;

        let mut system_tokens_len = 0;
        let mut cache_hit = false;

        if let Some(state) = &*cache_lock {
            if state.system_prompt == system_prompt {
                system_tokens_len = state.system_tokens_len;
                cache_hit = true;
            }
        }

        if cache_hit {
            log::info!("[LLM] System prompt KV cache hit. Reusing {} tokens.", system_tokens_len);
        } else {
            log::info!("[LLM] System prompt KV cache miss. Prefilling system prompt...");
            ctx.clear_kv_cache();

            let system_part = self.family.format_system_prompt(system_prompt);
            let sys_tokens = self.model
                .str_to_token(&system_part, AddBos::Always)
                .map_err(|e| anyhow!("[LLM] Tokenize system prompt failed: {}", e))?;

            if !sys_tokens.is_empty() {
                let mut batch = LlamaBatch::new(sys_tokens.len(), 1);
                for (i, &tok) in sys_tokens.iter().enumerate() {
                    let is_last = i == sys_tokens.len() - 1;
                    batch.add(tok, i as i32, &[0], is_last)
                        .map_err(|e| anyhow!("[LLM] Batch add system failed: {}", e))?;
                }
                ctx.decode(&mut batch)
                    .map_err(|e| anyhow!("[LLM] System prompt decode failed: {}", e))?;
                system_tokens_len = sys_tokens.len();
            }

            *cache_lock = Some(CacheState {
                system_prompt: system_prompt.to_string(),
                system_tokens_len,
            });
        }

        if user_text.is_empty() || user_text == "[WARMUP]" {
            log::info!("[LLM] Cache prefill warmup complete for system prompt.");
            return Ok(());
        }

        let user_part = self.family.format_user_prompt(user_text);
        let user_tokens = self.model
            .str_to_token(&user_part, AddBos::Never)
            .map_err(|e| anyhow!("[LLM] Tokenize user prompt failed: {}", e))?;

        if user_tokens.is_empty() {
            return Err(anyhow!("[LLM] Empty token list for user prompt"));
        }

        let total_input_tokens = system_tokens_len + user_tokens.len();
        let max_tokens = total_input_tokens + 512;
        let mut batch = LlamaBatch::new(max_tokens, 1);

        for (i, &tok) in user_tokens.iter().enumerate() {
            let pos = system_tokens_len as i32 + i as i32;
            let is_last = i == user_tokens.len() - 1;
            batch.add(tok, pos, &[0], is_last)
                .map_err(|e| anyhow!("[LLM] Batch add user failed: {}", e))?;
        }

        ctx.decode(&mut batch)
            .map_err(|e| anyhow!("[LLM] User prompt decode failed: {}", e))?;

        let mut n_cur = total_input_tokens as i32;

        log::info!("[LLM] >>> Generating (turn: {})...", turn_id);

        let mut raw_gen_buf = String::new();
        let stop_seqs = self.family.stop_sequences();
        let mut byte_buf = Vec::new();

        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                log::info!("[LLM] Cancelled at token {} (turn: {})", n_cur, turn_id);
                if n_cur > system_tokens_len as i32 {
                    let _ = ctx.clear_kv_cache_seq(Some(0), Some(system_tokens_len as u32), None);
                }
                let _ = tx.send(VoxEvent::Cancelled { turn_id });
                return Ok(());
            }

            let candidates = ctx.candidates_ith(batch.n_tokens() - 1);
            let mut candidates_p = LlamaTokenDataArray::from_iter(candidates, false);
            
            let token = if self.family == ModelFamily::Qwen {
                // Qwen temperature-based sampling with dist sampler to avoid infinite repeating loops
                let sampler = LlamaSampler::chain_simple([
                    LlamaSampler::top_k(20),
                    LlamaSampler::top_p(0.95, 1),
                    LlamaSampler::temp(0.6),
                    LlamaSampler::dist(42),
                ]);
                sampler.apply(&mut candidates_p);
                candidates_p.selected_token().unwrap_or_else(|| candidates_p.sample_token_greedy())
            } else {
                candidates_p.sample_token_greedy()
            };

            if self.model.is_eog_token(token) {
                log::info!("[LLM] EOS reached (turn: {})", turn_id);
                break;
            }

            // Stateful UTF-8 multibyte character boundary decoding
            let token_bytes = self.token_to_bytes(token);
            byte_buf.extend_from_slice(&token_bytes);

            let mut token_str = String::new();
            let mut decoded = false;

            match std::str::from_utf8(&byte_buf) {
                Ok(s) => {
                    token_str = s.to_string();
                    byte_buf.clear();
                    decoded = true;
                }
                Err(_) => {
                    // Incomplete multibyte sequence; wait for the next token to complete it
                }
            }

            if decoded && !token_str.is_empty() {
                raw_gen_buf.push_str(&token_str);

                // Stop sequence detection
                let mut stop_triggered = false;
                for stop in stop_seqs {
                    if let Some(pos) = raw_gen_buf.find(stop) {
                        log::info!("[LLM] Stop sequence {:?} triggered! Terminating generation.", stop);
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
            }

            if n_cur >= self.ctx_size as i32 {
                log::warn!("[LLM] Context limit reached ({} tokens).", self.ctx_size);
                break;
            }

            batch.clear();
            batch.add(token, n_cur, &[0], true)
                .map_err(|e| anyhow!("[LLM] Batch add (gen) failed: {}", e))?;
            n_cur += 1;

            if n_cur > (total_input_tokens as i32 + 512) {
                log::warn!("[LLM] Safety limit reached (512 tokens).");
                break;
            }

            ctx.decode(&mut batch)
                .map_err(|e| anyhow!("[LLM] Decode step failed: {}", e))?;
        }

        if n_cur > system_tokens_len as i32 {
            let _ = ctx.clear_kv_cache_seq(Some(0), Some(system_tokens_len as u32), None);
        }

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
