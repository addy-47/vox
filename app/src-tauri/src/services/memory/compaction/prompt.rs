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
You are a structured memory extraction engine for an intelligent assistant.
Your task is to analyze conversation turns and extract complete, self-contained declarative facts while preserving full semantic context.
</role>

<objective>
Extract explicit, durable, high-confidence declarative facts into the unified memory schema defined below.
</objective>

<output_schema>
{
  "context_summary": "<rolling conversational summary string>",
  "personal": ["<unstructured fact about user>"],
  "objective": ["<unstructured goal/intent>"],
  "workdone": ["<unstructured completed task/milestone>"],
  "blocker": ["<unstructured error/blocker>"],
  "next_step": ["<unstructured upcoming step>"],
  "pitfall": ["<unstructured edge case/lesson learned>"]
}
</output_schema>

<field_definitions>
context_summary:
A single, concise, chronological narrative summary describing the session's overall progression, key decisions, and conversational context so far.

personal:
Stable facts, preferences, habits, personal characteristics, or attributes about the user.

objective:
Active operational goals, intentions, assigned work, or project scope statements.

workdone:
Completed tasks, milestones reached, code changes authored, or verified solutions.

blocker:
Technical issues, unresolved errors, missing dependencies, or blocked execution paths.

next_step:
Concrete planned follow-ups, upcoming actions, or pending tasks.

pitfall:
Edge cases discovered, lessons learned, antipatterns, or architectural constraints to respect.
</field_definitions>

<extraction_principles>
1. COMPLETE DECLARATIVE SENTENCES ONLY:
   - Every extracted statement MUST be a complete, self-contained declarative sentence.
   - NEVER extract single-word labels, bare entity names, or incomplete fragments.

2. CONTEXT & PRECISION PRESERVATION:
   - Preserve all crucial details: numbers, file paths, tool names, exact error messages, and parameters.
   - Keep each extracted statement atomic: state exactly one durable fact per sentence.
</extraction_principles>

<output_requirements>
- Output exactly ONE JSON object strictly adhering to <output_schema>.
- context_summary is a single string. All other keys are JSON arrays of complete declarative sentence strings.
- Do not output markdown codeblock formatting or surrounding commentary outside the JSON object.
</output_requirements>"#;

/// Calculates dynamic max compaction output tokens based on context window size.
pub fn calculate_compaction_max_tokens(ctx_size: u32) -> u32 {
    let ctx = ctx_size as f32;
    let ratio = if ctx <= 8192.0 {
        let t = ((ctx - 2048.0) / (8192.0 - 2048.0)).clamp(0.0, 1.0);
        0.30 - t * (0.30 - 0.15)
    } else {
        let t = ((ctx - 8192.0) / (1_000_000.0 - 8192.0)).clamp(0.0, 1.0);
        0.15 - t * (0.15 - 0.10)
    };

    let raw = (ctx * ratio) as u32;
    raw.clamp(256, 16_384)
}

/// Builds the provider-neutral GenerationRequest for compaction.
pub fn build_compaction_request(
    history_messages: &[ChatMessage],
    settings: Option<&LlmSettings>,
) -> GenerationRequest {
    let mut history_text = String::new();
    for msg in history_messages {
        history_text.push_str(&format!("{}: {}\n\n", msg.role, msg.content));
    }

    let user_content = format!(
        "<conversation_history>\n{}\n</conversation_history>\n\n\
         <task>\n\
         Analyze the <conversation_history> above and extract all stated facts into the unified schema from <output_schema>.\n\
         Output ONLY the JSON object starting with {{ and ending with }}.\n\
         </task>",
        history_text
    );

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let default_settings = LlmSettings::default();
    let effective_settings = settings.unwrap_or(&default_settings);
    let eff_ctx = effective_settings.effective_ctx_size();
    let compaction_max_tokens = calculate_compaction_max_tokens(eff_ctx);
    let policy = GenerationPolicy::from_settings(effective_settings, Some(compaction_max_tokens));

    policy.build_request(
        GenerationPurpose::MemoryCompaction,
        ConversationInput {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: COMPACTION_SYSTEM_PROMPT.to_string(),
                    timestamp_ms: now_ms,
                },
                ChatMessage {
                    role: Role::User,
                    content: user_content,
                    timestamp_ms: now_ms,
                },
            ],
        },
    )
}
