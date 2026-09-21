use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    core::settings::LlmSettings,
    services::{
        harness::{ChatMessage, Role},
        llm::{ConversationInput, GenerationPolicy, GenerationPurpose, GenerationRequest},
    },
};

/// Prompt instructions instructing the LLM to extract durable facts into the unified memory schema.
pub const COMPACTION_SYSTEM_PROMPT: &str = r#"<role>
You are a session state and memory extraction engine for an AI assistant.
Analyze the dialogue to extract:
1. "personal": Durable facts, traits, and preferences about the user.
2. "objective", "workdone", "blocker", "next_step", "pitfall": The assistant's operational state for this thread.
The entire JSON output is injected into working memory (<session_context>) so the assistant retains full context after turns are pruned.
</role>

<schema>
{
  "personal": ["<durable user facts, traits, or preferences>"],
  "objective": ["<overarching goal, topic, or problem the assistant is addressing>"],
  "workdone": ["<concrete progress, deliverables, decisions, or answers completed>"],
  "blocker": ["<missing information, pending decisions, or hurdles blocking progress>"],
  "next_step": ["<immediate planned actions or agreed follow-ups for upcoming turns>"],
  "pitfall": ["<mistakes made, user corrections, dead-ends, or approaches to avoid>"]
}
</schema>

<category_definitions>
- personal: Facts about the user (identity, preferences, habits, traits, background).
- objective: The mission, question, or goal assigned to the assistant for this session.
- workdone: What the assistant has already investigated, completed, produced, or answered.
- blocker: What is currently unresolved, missing, or waiting on clarification before progress can resume.
- next_step: Concrete actions or next turns planned to advance the objective.
- pitfall: What failed, user corrections received, or constraints/methods explicitly ruled out.
</category_definitions>

<rules>
- Concise, self-contained declarative statements only. No conversational filler ("The user said...", "In this chat...").
- "personal" describes the user; the other 5 categories describe the assistant's operational task state.
- Completely ignore small-talk, greetings, and pleasantries. Output an empty list [] for categories with no substantive content.
- Dialogue often repeats the same topics: deduplicate, but capture each distinct durable fact once — repetition is emphasis, not noise.
- "personal" is mandatory whenever the dialogue states user identity, preferences, habits, background, or health constraints: put such facts in "personal" itself, never nested inside other buckets behind prefixes like "profile_recorded:" or "preference_recorded:".
- Extract every distinct durable fact across the whole slice; do not stop after the first few.
- System/hardware configuration, people and their organizational roles, health constraints, and locations are always durable: never omit them.
- If <prior_summary> is present, update the state incrementally; do not repeat unchanged facts.
- Output ONLY the raw JSON object adhering to <schema>. No markdown formatting, backticks, or commentary.
</rules>"#;

pub const COMPACTION_OUTPUT_RATIO: f32 = 0.15;
pub const COMPACTION_MIN_OUTPUT_TOKENS: u32 = 256;
pub const COMPACTION_MAX_OUTPUT_TOKENS: u32 = 16_384;
pub const DEFAULT_LLM_COMPACTION_TEMPERATURE: f32 = 0.2;

/// Canonical JSON Schema enforced on every compaction extraction.
/// Single source of truth shared by the prompt prose and the wire constraint.
pub fn compaction_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "personal": { "type": "array", "items": { "type": "string" } },
            "objective": { "type": "array", "items": { "type": "string" } },
            "workdone": { "type": "array", "items": { "type": "string" } },
            "blocker": { "type": "array", "items": { "type": "string" } },
            "next_step": { "type": "array", "items": { "type": "string" } },
            "pitfall": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["personal", "objective", "workdone", "blocker", "next_step", "pitfall"],
        "additionalProperties": false
    })
}

/// Calculates dynamic max compaction output tokens based on context window and probed ceiling.
/// Formula: min(slice, probed_max_output) where slice = (ctx_size * 0.15).
pub fn calculate_compaction_max_tokens(ctx_size: u32, probed_max_output: Option<u32>) -> u32 {
    let slice = (ctx_size as f32 * COMPACTION_OUTPUT_RATIO) as u32;
    let clamped_slice = slice.clamp(COMPACTION_MIN_OUTPUT_TOKENS, COMPACTION_MAX_OUTPUT_TOKENS);
    if let Some(ceiling) = probed_max_output {
        clamped_slice.min(ceiling).max(COMPACTION_MIN_OUTPUT_TOKENS)
    } else {
        clamped_slice
    }
}

/// Builds a complete `GenerationRequest` for memory compaction extraction from a slice of turns.
pub fn build_compaction_request(
    history_messages: &[ChatMessage],
    settings: Option<&LlmSettings>,
) -> GenerationRequest {
    let prior_summary = history_messages
        .iter()
        .find(|m| m.role == Role::System)
        .and_then(|m| crate::services::harness::PromptTag::SessionContext.extract(&m.content));

    let prior_summary_block = match prior_summary {
        Some(s) if !s.trim().is_empty() => {
            format!("<prior_summary>\n{}\n</prior_summary>\n\n", s.trim())
        }
        _ => String::new(),
    };

    let mut history_text = String::new();
    for msg in history_messages {
        if msg.role == Role::System {
            continue;
        }
        let speaker = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System | Role::Tool => continue,
        };
        history_text.push_str(&format!(
            r#"<turn speaker="{}">{}</turn>
"#,
            speaker,
            msg.content.trim()
        ));
    }

    let user_content = format!(
        "{}\
         <dialogue>\n{}</dialogue>\n\n\
         <task>\n\
         Analyze the <dialogue> turns above in light of <prior_summary> if present.\n\
         Extract the user's profile facts into \"personal\", and the assistant's operational session state across \"objective\", \"workdone\", \"blocker\", \"next_step\", and \"pitfall\".\n\
         Output ONLY the raw JSON object starting with {{ and ending with }}.\n\
         </task>",
        prior_summary_block, history_text
    );

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let default_settings = LlmSettings::default();
    let effective_settings = settings.unwrap_or(&default_settings);
    let eff_ctx = effective_settings.effective_ctx_size();
    let model = effective_settings.active_model();
    let baseline_spec = crate::services::llm::catalog::get_baseline_spec(model);
    let probed_max_output = baseline_spec.and_then(|s| s.max_output_tokens);
    let compaction_max_tokens = calculate_compaction_max_tokens(eff_ctx, probed_max_output);
    let policy = GenerationPolicy::from_settings(effective_settings, Some(compaction_max_tokens));

    let mut request = policy.build_request(
        GenerationPurpose::MemoryCompaction,
        ConversationInput {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: COMPACTION_SYSTEM_PROMPT.to_string(),
                    timestamp_ms: now_ms,
                    tool_call_id: None,
                    tool_calls: None,
                },
                ChatMessage {
                    role: Role::User,
                    content: user_content,
                    timestamp_ms: now_ms,
                    tool_call_id: None,
                    tool_calls: None,
                },
            ],
        },
    );
    request.options.temperature = Some(DEFAULT_LLM_COMPACTION_TEMPERATURE);
    // INVARIANT: compaction reasoning is always disabled, even if the user
    // later enables reasoning for agentic conversation. Voice-native default.
    request.options.reasoning = crate::services::llm::ReasoningMode::Disabled;
    // Universal compaction contract: strict schema where the backend supports
    // it, JSON-object baseline otherwise (transports negotiate down on 400s).
    request.output = compaction_output_constraint(model);

    if let (Some(sys), Some(usr)) = (
        request.input.messages.first(),
        request.input.messages.get(1),
    ) {
        log::info!(
            "[CompactionLLM::Input] (chars: {}, max_tokens: {:?})\n--- SYSTEM PROMPT ---\n{}\n--- USER PROMPT ---\n{}",
            sys.content.len() + usr.content.len(),
            request.options.max_output_tokens,
            sys.content,
            usr.content
        );
    }

    request
}

/// Selects the strict schema constraint, falling back to JSON-object when the
/// catalog baseline explicitly reports no structured-output support.
fn compaction_output_constraint(model: &str) -> crate::services::llm::OutputConstraint {
    let supported = crate::services::llm::catalog::get_baseline_spec(model)
        .map(|s| s.supports_structured)
        .unwrap_or(true);
    if supported {
        crate::services::llm::OutputConstraint::JsonSchema {
            name: "memory_compaction".to_string(),
            schema: compaction_json_schema(),
            strict: true,
        }
    } else {
        log::warn!(
            "[CompactionLLM::Output] Model {} lacks structured-output support; using JSON-object baseline.",
            model
        );
        crate::services::llm::OutputConstraint::JsonObject
    }
}
