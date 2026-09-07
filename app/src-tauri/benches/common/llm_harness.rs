//! ============================================================================
//! benches/common/llm_harness.rs — LLM Evaluation & Inference Benchmark Harness
//! ============================================================================
//! Category     : Benchmark Harness
//! Component    : services::llm (EmbeddedProvider, LlmWorker)
//! Prerequisites: Local GGUF models in ~/.vox/models/llm/
//! Execution    : Used by benches/llm_bench.rs
//! Metrics      : TTFT (ms), Total Generation Time (ms), Tokens/sec, Memory RSS (MB)
//! ============================================================================

use std::{
    path::Path,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use vox_lib::services::{
    harness::{ChatMessage, Role},
    llm::{
        ConversationInput, GenerationOptions, GenerationPurpose, GenerationRequest, LlmProvider,
        LlmStreamEvent, OutputConstraint,
    },
};

use super::reporting::{get_process_memory_mb, BenchmarkSystemInfo};

/// Benchmark tuning parameters passed from CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmBenchmarkParams {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub max_output_tokens: u32,
    pub warmup: bool,
    pub turns: usize,
    pub seed: u64,
}

/// Measurement result for a single conversational turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTurnResult {
    pub turn_id: u32,
    pub prompt: String,
    pub response: String,
    pub generated_tokens: usize,
    pub ttft_ms: f64,
    pub generation_time_ms: f64,
    pub total_time_ms: f64,
    pub tokens_per_sec: f64,
}

/// Execution summary for an LLM model benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmBenchmarkRunResult {
    pub model_name: String,
    pub model_path: String,
    pub memory_rss_mb: u64,
    pub warmup_performed: bool,
    pub warmup_duration_ms: f64,
    pub turns: Vec<LlmTurnResult>,
    pub avg_ttft_ms: f64,
    pub avg_tokens_per_sec: f64,
}

/// Complete benchmark report artifact structure for LLM evaluations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmBenchmarkReport {
    pub run_id: String,
    pub timestamp_utc: String,
    pub benchmark_name: String,
    pub system_info: BenchmarkSystemInfo,
    pub params: LlmBenchmarkParams,
    pub runs: Vec<LlmBenchmarkRunResult>,
}

/// Canonical multi-turn conversational benchmark script derived from Vox audio test-clips.
pub const CANONICAL_CONVERSATION_TURNS: &[&str] = &[
    "Hey Vox, good morning! Can you check my calendar and give me a quick briefing on today's scheduled meetings?",
    "Vox, what's the weather like outside right now? Is it going to rain later this afternoon?",
    "Can you help me refactor this Rust async function to reduce mutex contention across our background threads?",
    "Hey Vox, summarize the key action items from my design review notes and draft a quick email to the team.",
    "Set a timer for twenty-five minutes for a focused Pomodoro session, and minimize background notifications.",
];

/// Benchmarks an embedded LLM provider across multi-turn conversation steps.
pub fn benchmark_llm_provider(
    provider: Arc<dyn LlmProvider>,
    model_name: &str,
    model_path: &Path,
    system_prompt: &str,
    params: &LlmBenchmarkParams,
) -> Result<LlmBenchmarkRunResult, String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

    let mut warmup_duration_ms = 0.0;

    // 1. Execute Session Warmup if requested
    if params.warmup {
        println!("  [Warmup] Prefilling system prompt and initializing context...");
        let warmup_start = Instant::now();
        let (tx, _rx) = mpsc::channel();
        let cancel = CancellationToken::new();

        let warmup_req = GenerationRequest {
            input: ConversationInput {
                messages: vec![
                    ChatMessage {
                        role: Role::System,
                        content: system_prompt.to_string(),
                        timestamp_ms: 0,
                    },
                    ChatMessage {
                        role: Role::User,
                        content: "[WARMUP]".to_string(),
                        timestamp_ms: 0,
                    },
                ],
            },
            options: GenerationOptions::default(),
            output: OutputConstraint::Text,
            purpose: GenerationPurpose::Conversation,
        };

        runtime
            .block_on(provider.generate(warmup_req, 0, &cancel, &tx))
            .map_err(|e| format!("Warmup generation failed: {:?}", e))?;

        warmup_duration_ms = warmup_start.elapsed().as_secs_f64() * 1000.0;
        println!("  [Warmup] Complete in {:.2}ms", warmup_duration_ms);
    }

    let memory_rss_mb = get_process_memory_mb();
    let mut history = vec![ChatMessage {
        role: Role::System,
        content: system_prompt.to_string(),
        timestamp_ms: 0,
    }];

    let turns_to_run = params.turns.min(CANONICAL_CONVERSATION_TURNS.len());
    let mut turn_results = Vec::new();

    for (turn_idx, &prompt_text) in CANONICAL_CONVERSATION_TURNS
        .iter()
        .enumerate()
        .take(turns_to_run)
    {
        let turn_id = (turn_idx + 1) as u32;

        history.push(ChatMessage {
            role: Role::User,
            content: prompt_text.to_string(),
            timestamp_ms: 0,
        });

        let request = GenerationRequest {
            input: ConversationInput {
                messages: history.clone(),
            },
            options: GenerationOptions {
                temperature: Some(params.temperature),
                top_p: Some(params.top_p),
                top_k: Some(params.top_k),
                max_output_tokens: Some(params.max_output_tokens),
                seed: Some(params.seed),
                ..Default::default()
            },
            output: OutputConstraint::Text,
            purpose: GenerationPurpose::Conversation,
        };

        let (tx, rx) = mpsc::channel();
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        let provider_clone = Arc::clone(&provider);

        let start_time = Instant::now();
        let mut first_token_time: Option<Instant> = None;
        let mut full_response = String::new();
        let mut token_count = 0;

        let gen_handle = runtime.spawn(async move {
            provider_clone
                .generate(request, turn_id, &cancel_clone, &tx)
                .await
        });

        while let Ok(event) = rx.recv_timeout(Duration::from_secs(60)) {
            match event {
                LlmStreamEvent::Token(tok) => {
                    if first_token_time.is_none() {
                        first_token_time = Some(Instant::now());
                    }
                    full_response.push_str(&tok);
                    token_count += 1;
                }
                LlmStreamEvent::Finished => break,
            }
        }

        let gen_result = runtime
            .block_on(gen_handle)
            .map_err(|_| "LLM generation task panicked".to_string())?;
        gen_result.map_err(|e| format!("Turn {} generation failed: {:?}", turn_id, e))?;

        let total_elapsed = start_time.elapsed();
        let ttft = first_token_time
            .map(|t| t.duration_since(start_time))
            .unwrap_or(total_elapsed);
        let generation_time = total_elapsed.saturating_sub(ttft);

        let ttft_ms = ttft.as_secs_f64() * 1000.0;
        let gen_time_ms = generation_time.as_secs_f64() * 1000.0;
        let total_time_ms = total_elapsed.as_secs_f64() * 1000.0;
        let tokens_per_sec = if generation_time.as_secs_f64() > 0.0 {
            token_count as f64 / generation_time.as_secs_f64()
        } else {
            0.0
        };

        println!(
            "  Turn {}: TTFT: {:.1}ms | Gen: {:.1}ms | Tokens: {} | Speed: {:.1} tok/s",
            turn_id, ttft_ms, gen_time_ms, token_count, tokens_per_sec
        );

        history.push(ChatMessage {
            role: Role::Assistant,
            content: full_response.clone(),
            timestamp_ms: 0,
        });

        turn_results.push(LlmTurnResult {
            turn_id,
            prompt: prompt_text.to_string(),
            response: full_response,
            generated_tokens: token_count,
            ttft_ms,
            generation_time_ms: gen_time_ms,
            total_time_ms,
            tokens_per_sec,
        });
    }

    let avg_ttft_ms = if !turn_results.is_empty() {
        turn_results.iter().map(|r| r.ttft_ms).sum::<f64>() / (turn_results.len() as f64)
    } else {
        0.0
    };

    let avg_tokens_per_sec = if !turn_results.is_empty() {
        turn_results.iter().map(|r| r.tokens_per_sec).sum::<f64>() / (turn_results.len() as f64)
    } else {
        0.0
    };

    Ok(LlmBenchmarkRunResult {
        model_name: model_name.to_string(),
        model_path: model_path.to_string_lossy().to_string(),
        memory_rss_mb,
        warmup_performed: params.warmup,
        warmup_duration_ms,
        turns: turn_results,
        avg_ttft_ms,
        avg_tokens_per_sec,
    })
}
