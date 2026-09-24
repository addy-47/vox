use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    core::settings::LlmSettings,
    services::{
        harness::{ChatMessage, PromptTag::SessionContext, Role},
        llm::{
            catalog, ConversationInput, GenerationPolicy, GenerationPurpose, GenerationRequest,
            OutputConstraint, ReasoningMode,
        },
    },
};

/// Prompt instructions instructing the LLM to extract durable facts into the unified memory schema.
pub const COMPACTION_SYSTEM_PROMPT: &str = r#"<role>
You extract durable memory and current task state from a conversation.
You are a conservative extractor: preserve what the speakers explicitly establish, and do not fill gaps with plausible inference.
The complete JSON object becomes the session's working context after the raw turns are pruned.
</role>

<schema>
{
  "personal": ["<explicit durable fact stated by the user>"],
  "objective": ["<active mission, question, project, or topic being addressed>"],
  "workdone": ["<completed progress or answer explicitly supported by the dialogue>"],
  "blocker": ["<unresolved obstacle, missing information, or pending decision>"],
  "next_step": ["<planned, promised, intended, or scheduled action not yet completed>"],
  "pitfall": ["<explicit correction, failed approach, dead end, or constraint to avoid>"]
}
</schema>

<attribution>
- Only the user's explicit statements establish facts in "personal".
- A question, search request, topic, or location mentioned by the user does not establish residence, identity, ownership, preference, or habit.
- Assistant suggestions, recommendations, examples, and hypothetical answers are not user facts or user preferences.
- A user asking for options is not evidence that the user likes, enjoys, prefers, or owns any option.
- Do not convert an assistant claim into a completed action unless the dialogue explicitly records successful execution or a successful tool/action result.
- Preserve modality: "will", "plans to", "intends to", "reminds me to", and "I'll" are next steps, not completed work.
</attribution>

<definitions>
- "personal": Explicit user identity, durable background, habits, goals, or preferences.
- "objective": The mission or topic being addressed, including a request for information or suggestions.
- "workdone": Only completed, explicitly supported progress, decisions, or answers.
- "blocker": Something unresolved, missing, waiting for clarification, or preventing progress.
- "next_step": A concrete future action, intention, commitment, or scheduled follow-up.
- "pitfall": A user correction, failed approach, or constraint explicitly recorded in the dialogue.
</definitions>

<precision_rules>
- Use concise third-person declarative statements without conversational filler.
- Deduplicate repeated statements while preserving distinct supported facts.
- Prefer an empty array over an uncertain or weakly supported claim.
- Do not invent details, completion status, preferences, locations, identities, or external side effects.
- If <prior_summary> is present, update it incrementally without duplicating unchanged facts.
- Output only the raw JSON object matching the schema. Do not add markdown, commentary, or provenance fields.
</precision_rules>"#;

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
        .and_then(|m| SessionContext.extract(&m.content));

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
    let baseline_spec = catalog::get_baseline_spec(model);
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
    request.options.reasoning = ReasoningMode::Disabled;
    request.options.context_window = Some(eff_ctx);
    // Universal compaction contract: strict schema where the backend supports
    // it, JSON-object baseline otherwise (transports negotiate down on 400s).
    request.output = compaction_output_constraint(model);

    log::info!(
        "[CompactionLLM::Request] model={} purpose={:?} output={:?} temperature={:?} max_output_tokens={:?} context_window={:?} reasoning={:?} top_p={:?} top_k={:?} seed={:?} stop_count={} tools_present={} messages={} system_chars={} user_chars={}",
        effective_settings.active_model(),
        request.purpose,
        request.output,
        request.options.temperature,
        request.options.max_output_tokens,
        request.options.context_window,
        request.options.reasoning,
        request.options.top_p,
        request.options.top_k,
        request.options.seed,
        request.options.stop.len(),
        request.tools.is_some(),
        request.input.messages.len(),
        request.input.messages.first().map_or(0, |message| message.content.len()),
        request.input.messages.get(1).map_or(0, |message| message.content.len()),
    );

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
fn compaction_output_constraint(model: &str) -> OutputConstraint {
    let supported = catalog::get_baseline_spec(model)
        .map(|s| s.supports_structured)
        .unwrap_or(true);
    if supported {
        OutputConstraint::JsonSchema {
            name: "memory_compaction".to_string(),
            schema: compaction_json_schema(),
            strict: true,
        }
    } else {
        log::warn!(
            "[CompactionLLM::Output] Model {} lacks structured-output support; using JSON-object baseline.",
            model
        );
        OutputConstraint::JsonObject
    }
}
