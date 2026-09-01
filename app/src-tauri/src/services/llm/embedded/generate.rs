use super::family::{partial_tag_len, ModelFamily};
use super::worker::{CacheState, LlmWorker};
use crate::core::events::VoxEvent;
use crate::services::harness::ConversationContext;
use crate::services::llm::LlmEngine;
use anyhow::{anyhow, Result};
use llama_cpp_4::{
    context::LlamaContext,
    llama_batch::LlamaBatch,
    model::AddBos,
    sampling::LlamaSampler,
    token::data_array::LlamaTokenDataArray,
    token::LlamaToken,
};

/// Soft-cap limits and state for stream generation.
struct GenerationLimits {
    pub total_input_tokens: usize,
    pub max_ctx_size: u32,
    pub max_safety_tokens: usize,
}

impl GenerationLimits {
    pub fn new(total_input_tokens: usize, max_ctx_size: u32) -> Self {
        Self {
            total_input_tokens,
            max_ctx_size,
            max_safety_tokens: crate::services::llm::DEFAULT_MAX_GENERATION_SAFETY_TOKENS,
        }
    }

    /// Evaluates if generation has exceeded either context limit or safety limit soft caps.
    pub fn is_soft_cap_exceeded(&self, n_cur: i32) -> bool {
        if n_cur >= self.max_ctx_size as i32 {
            log::warn!("[LLM] Context limit reached ({} tokens).", self.max_ctx_size);
            return true;
        }
        if n_cur > (self.total_input_tokens + self.max_safety_tokens) as i32 {
            log::warn!(
                "[LLM] Safety limit reached ({} tokens beyond input).",
                self.max_safety_tokens
            );
            return true;
        }
        false
    }
}

/// Helper struct managing incremental streaming buffer, tag stripping, and partial emission.
struct StreamingEmitter<'a> {
    family: &'a ModelFamily,
    turn_id: u32,
    tx: &'a std::sync::mpsc::Sender<VoxEvent>,
    raw_gen_buf: String,
    emitted_clean_len: usize,
    byte_buf: Vec<u8>,
}

impl<'a> StreamingEmitter<'a> {
    pub fn new(
        family: &'a ModelFamily,
        turn_id: u32,
        tx: &'a std::sync::mpsc::Sender<VoxEvent>,
    ) -> Self {
        Self {
            family,
            turn_id,
            tx,
            raw_gen_buf: String::new(),
            emitted_clean_len: 0,
            byte_buf: Vec::new(),
        }
    }

    /// Appends raw token bytes, decodes UTF-8, and returns true if a stop sequence was triggered.
    pub fn process_token_bytes(
        &mut self,
        token_bytes: &[u8],
        ttft: &mut Option<std::time::Duration>,
        start_time: &std::time::Instant,
        tokens_generated: &mut usize,
    ) -> bool {
        self.byte_buf.extend_from_slice(token_bytes);

        if let Ok(s) = std::str::from_utf8(&self.byte_buf) {
            let token_str = s.to_string();
            self.byte_buf.clear();

            if !token_str.is_empty() {
                self.raw_gen_buf.push_str(&token_str);

                // Check stop sequences
                for stop in self.family.stop_sequences() {
                    if let Some(pos) = self.raw_gen_buf.find(stop) {
                        log::info!(
                            "[LLM] Stop sequence {:?} triggered! Terminating generation.",
                            stop
                        );
                        self.emit_stopped_remainder(pos);
                        return true;
                    }
                }

                if ttft.is_none() {
                    *ttft = Some(start_time.elapsed());
                }

                self.emit_partial_clean_delta();
                *tokens_generated += 1;
            }
        }
        false
    }

    fn emit_stopped_remainder(&mut self, pos: usize) {
        let clean_remaining = &self.raw_gen_buf[..pos];
        let mut cleaned = self.family.strip_tags(clean_remaining);
        if let Some(think_pos) = cleaned.find("<think>") {
            cleaned.truncate(think_pos);
        }
        let cleaned_trimmed = cleaned.trim().to_string();
        if cleaned_trimmed.len() > self.emitted_clean_len {
            let delta = &cleaned_trimmed[self.emitted_clean_len..];
            if !delta.is_empty() {
                let _ = self.tx.send(VoxEvent::LlmToken {
                    turn_id: self.turn_id,
                    token: delta.to_string(),
                });
            }
            self.emitted_clean_len = cleaned_trimmed.len();
        }
    }

    fn emit_partial_clean_delta(&mut self) {
        let cleaned = self.family.strip_tags_raw(&self.raw_gen_buf);
        let tags = self.family.tags_to_strip();
        let holdback = partial_tag_len(&cleaned, tags);
        let clean_len = cleaned.len().saturating_sub(holdback);

        if clean_len > self.emitted_clean_len {
            let delta = &cleaned[self.emitted_clean_len..clean_len];
            if !delta.is_empty() {
                let _ = self.tx.send(VoxEvent::LlmToken {
                    turn_id: self.turn_id,
                    token: delta.to_string(),
                });
            }
            self.emitted_clean_len = clean_len;
        }
    }

    /// Emits any remaining cleaned text upon end of generation.
    pub fn emit_final_flush(self) {
        let mut final_cleaned = self.family.strip_tags_raw(&self.raw_gen_buf);
        if let Some(think_pos) = final_cleaned.find("<think>") {
            final_cleaned.truncate(think_pos);
        }
        let final_trimmed = final_cleaned.trim_end().to_string();
        if final_trimmed.len() > self.emitted_clean_len {
            let delta = &final_trimmed[self.emitted_clean_len..];
            if !delta.is_empty() {
                let _ = self.tx.send(VoxEvent::LlmToken {
                    turn_id: self.turn_id,
                    token: delta.to_string(),
                });
            }
        }
    }
}

impl LlmWorker {
    /// Prefills the full conversation or appends the latest user turn if KV-cache matches prefix.
    fn prefill_or_reuse_kv_cache(
        &self,
        ctx: &mut LlamaContext<'static>,
        conv_ctx: &ConversationContext,
        turn_id: u32,
        cancel: &tokio_util::sync::CancellationToken,
        tx: &std::sync::mpsc::Sender<VoxEvent>,
    ) -> Result<Option<(usize, i32)>> {
        let last_user_text = conv_ctx
            .messages
            .last()
            .map(|m| m.content.as_str())
            .unwrap_or("");

        let system_prompt = conv_ctx
            .messages
            .first()
            .map(|m| m.content.as_str())
            .unwrap_or("You are Vox.");

        let (system_tokens_len, initial_seq_len, cache_hit) = {
            let cache_lock = self.cache_state.lock();
            if let Some(state) = &*cache_lock {
                if state.system_prompt == system_prompt {
                    (state.system_tokens_len, state.current_seq_tokens_len, true)
                } else {
                    (0, 0, false)
                }
            } else {
                (0, 0, false)
            }
        };

        if cache_hit && initial_seq_len > 0 {
            log::info!(
                "[LLM] KV cache hit. Reusing {} sequence tokens (system prompt: {}).",
                initial_seq_len,
                system_tokens_len
            );

            if !last_user_text.is_empty() && last_user_text != "[WARMUP]" {
                let user_part = self.family.format_user_prompt(last_user_text);
                let user_tokens = self
                    .model
                    .str_to_token(&user_part, AddBos::Never)
                    .map_err(|e| anyhow!("[LLM] Tokenize user prompt failed: {}", e))?;

                if !user_tokens.is_empty() {
                    if cancel.is_cancelled() {
                        log::info!("[LLM] Generation cancelled during user prompt decode.");
                        *self.cache_state.lock() = None;
                        ctx.clear_kv_cache();
                        let _ = tx.send(VoxEvent::Cancelled { turn_id });
                        return Ok(None);
                    }

                    let total_input_tokens = initial_seq_len + user_tokens.len();
                    let mut batch = LlamaBatch::new(user_tokens.len(), 1);

                    for (i, &tok) in user_tokens.iter().enumerate() {
                        let pos = initial_seq_len as i32 + i as i32;
                        let is_last = i == user_tokens.len() - 1;
                        batch
                            .add(tok, pos, &[0], is_last)
                            .map_err(|e| anyhow!("[LLM] Batch add user failed: {}", e))?;
                    }

                    ctx.decode(&mut batch)
                        .map_err(|e| anyhow!("[LLM] User prompt decode failed: {}", e))?;

                    let last_sample_ith = user_tokens.len() as i32 - 1;
                    return Ok(Some((total_input_tokens, last_sample_ith)));
                }
            }
            Ok(Some((initial_seq_len, 0)))
        } else {
            log::info!("[LLM] KV cache miss. Prefilling full conversation context...");
            ctx.clear_kv_cache();

            let full_prompt = self.family.format_conversation(&conv_ctx.messages);
            let prompt_tokens = self
                .model
                .str_to_token(&full_prompt, AddBos::Always)
                .map_err(|e| anyhow!("[LLM] Tokenize full conversation failed: {}", e))?;

            let system_part = self.family.format_system_prompt(system_prompt);
            let sys_len = self
                .model
                .str_to_token(&system_part, AddBos::Always)
                .map(|t| t.len())
                .unwrap_or(0);

            let mut last_sample_ith = 0;
            if !prompt_tokens.is_empty() {
                let n_batch_chunk = crate::services::llm::DEFAULT_BATCH_CHUNK_SIZE;
                let total = prompt_tokens.len();
                let mut offset = 0;
                while offset < total {
                    if cancel.is_cancelled() {
                        log::info!("[LLM] Generation cancelled during prefill phase.");
                        *self.cache_state.lock() = None;
                        ctx.clear_kv_cache();
                        let _ = tx.send(VoxEvent::Cancelled { turn_id });
                        return Ok(None);
                    }
                    let end = (offset + n_batch_chunk).min(total);
                    let chunk = &prompt_tokens[offset..end];
                    let mut batch = LlamaBatch::new(chunk.len(), 1);
                    for (i, &tok) in chunk.iter().enumerate() {
                        let global_idx = offset + i;
                        let is_last = global_idx == total - 1;
                        batch
                            .add(tok, global_idx as i32, &[0], is_last)
                            .map_err(|e| anyhow!("[LLM] Batch add prompt chunk failed: {}", e))?;
                    }
                    ctx.decode(&mut batch)
                        .map_err(|e| anyhow!("[LLM] Full prompt decode failed: {}", e))?;
                    last_sample_ith = chunk.len() as i32 - 1;
                    offset = end;
                }
            }

            *self.cache_state.lock() = Some(CacheState {
                system_prompt: system_prompt.to_string(),
                system_tokens_len: sys_len,
                current_seq_tokens_len: prompt_tokens.len(),
            });

            Ok(Some((prompt_tokens.len(), last_sample_ith)))
        }
    }

    /// Selects and samples the next token from candidate logits.
    fn sample_token(
        ctx: &mut LlamaContext<'static>,
        sample_ith: i32,
        sampler: &mut Option<LlamaSampler>,
    ) -> LlamaToken {
        let candidates = ctx.candidates_ith(sample_ith);
        let mut candidates_p = LlamaTokenDataArray::from_iter(candidates, false);

        if let Some(s) = sampler.as_mut() {
            s.apply(&mut candidates_p);
            candidates_p
                .selected_token()
                .unwrap_or_else(|| candidates_p.sample_token_greedy())
        } else {
            candidates_p.sample_token_greedy()
        }
    }
}

impl LlmEngine for LlmWorker {
    /// Generates tokens for the conversation context and streams them via `tx`.
    fn generate(
        &self,
        conv_ctx: &ConversationContext,
        turn_id: u32,
        cancel: &tokio_util::sync::CancellationToken,
        tx: &std::sync::mpsc::Sender<VoxEvent>,
    ) -> Result<()> {
        self.init_context()?;

        let mut ctx_lock = self.ctx.lock();
        let ctx = ctx_lock.as_mut().unwrap();

        let start_time = std::time::Instant::now();
        let mut ttft: Option<std::time::Duration> = None;
        let mut tokens_generated = 0;

        let prefill_res = self.prefill_or_reuse_kv_cache(ctx, conv_ctx, turn_id, cancel, tx)?;
        let (total_input_tokens, mut sample_ith) = match prefill_res {
            Some(res) => res,
            None => return Ok(()), // Cancelled
        };

        let last_user_text = conv_ctx
            .messages
            .last()
            .map(|m| m.content.as_str())
            .unwrap_or("");
        if last_user_text.is_empty() || last_user_text == "[WARMUP]" {
            log::info!("[LLM] Cache prefill warmup complete.");
            return Ok(());
        }

        let mut n_cur = total_input_tokens as i32;
        let limits = GenerationLimits::new(total_input_tokens, self.ctx_size);
        let mut emitter = StreamingEmitter::new(&self.family, turn_id, tx);

        log::info!("[LLM] >>> Generating (turn: {})...", turn_id);

        let mut batch = LlamaBatch::new(
            total_input_tokens + crate::services::llm::DEFAULT_MAX_GENERATION_SAFETY_TOKENS,
            1,
        );

        let mut qwen_sampler = if self.family == ModelFamily::Qwen {
            Some(LlamaSampler::chain_simple([
                LlamaSampler::penalties(self.ctx_size as i32, 1.0, 0.0, 2.0),
                LlamaSampler::top_k(20),
                LlamaSampler::top_p(1.0, 1),
                LlamaSampler::min_p(0.0, 1),
                LlamaSampler::temp(1.0),
                LlamaSampler::dist(42),
            ]))
        } else {
            None
        };

        loop {
            if cancel.is_cancelled() {
                log::info!("[LLM] Cancelled at token {} (turn: {})", n_cur, turn_id);
                *self.cache_state.lock() = None;
                let _ = tx.send(VoxEvent::Cancelled { turn_id });
                return Ok(());
            }

            let token = Self::sample_token(ctx, sample_ith, &mut qwen_sampler);

            if self.model.is_eog_token(token) {
                log::info!("[LLM] EOS reached (turn: {})", turn_id);
                break;
            }

            let token_bytes = self.token_to_bytes(token);
            let stopped = emitter.process_token_bytes(
                &token_bytes,
                &mut ttft,
                &start_time,
                &mut tokens_generated,
            );
            if stopped {
                break;
            }

            if limits.is_soft_cap_exceeded(n_cur) {
                break;
            }

            batch.clear();
            batch
                .add(token, n_cur, &[0], true)
                .map_err(|e| anyhow!("[LLM] Batch add (gen) failed: {}", e))?;
            n_cur += 1;

            ctx.decode(&mut batch)
                .map_err(|e| anyhow!("[LLM] Decode step failed: {}", e))?;
            sample_ith = 0;
        }

        emitter.emit_final_flush();

        if let Some(state) = &mut *self.cache_state.lock() {
            state.current_seq_tokens_len = n_cur as usize;
        }

        let elapsed = start_time.elapsed().as_secs_f32();
        let tps = tokens_generated as f32 / elapsed;

        log::info!(
            "[LLM] Generation complete (turn: {}). Tokens: {}, TTFT: {:?}, TPS: {:.2}",
            turn_id,
            tokens_generated,
            ttft.unwrap_or_default(),
            tps
        );

        let _ = tx.send(VoxEvent::LlmFinished { turn_id });
        Ok(())
    }
}
