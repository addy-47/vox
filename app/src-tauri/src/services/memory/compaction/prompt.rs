use crate::{
    core::settings::LlmSettings,
    services::{
        harness::buffer::{ChatMessage, Role},
        llm::{ConversationInput, GenerationPolicy, GenerationPurpose, GenerationRequest},
    },
};

/// Prompt instructions instructing the LLM to extract durable facts into the 6 memory collections.
pub const COMPACTION_SYSTEM_PROMPT: &str = r#"<role>
You are a structured memory extraction engine for an intelligent assistant.
Your task is to analyze conversation turns and extract complete, self-contained declarative facts while preserving full semantic context.
</role>

<objective>
Extract explicit, durable, high-confidence declarative facts into the six memory collections defined below.
</objective>

<output_schema>
{
  "Identity": [],
  "Directives": [],
  "Narrative": "",
  "Profile": [],
  "Entities": [],
  "Constraints": []
}
</output_schema>

<collection_definitions>
Identity:
Stable foundational facts that uniquely identify the user, such as their full name, core primary role, or enduring self-identification.

Directives:
Active operational goals, pending tasks, assigned work, commitments, standing instructions, scheduled events, and progress updates.

Narrative:
A single, concise, chronological narrative summary describing the session's overall progression and key milestones.

Profile:
Stable personal characteristics, preferences, skills, habits, experiences, interests, and behavioral tendencies.

Entities:
Declarative facts about named external subjects (people, organizations, tools, services) and their specific relationship or relevance to the user.

Constraints:
Hard, non-negotiable limits, safety boundaries, security rules, health/dietary restrictions, budget limits, or strict technical requirements.
</collection_definitions>

<extraction_principles>
1. COMPLETE DECLARATIVE SENTENCES ONLY:
   - Every extracted statement MUST be a complete, self-contained declarative sentence.
   - NEVER extract single-word labels, bare entity names, or incomplete fragments.

2. CONTEXT & PRECISION PRESERVATION:
   - Preserve all crucial details in each sentence: numbers, dollar amounts, temporal deadlines, exact model names, and specific constraints.
   - Keep each extracted statement atomic: state exactly one durable fact per sentence.

3. DISAMBIGUATION & CLASSIFICATION RULES:
   - Identity vs Profile: Reserve Identity strictly for core foundational user identity. If uncertain, ALWAYS classify under Profile.
   - Constraints vs Profile: Reserve Constraints strictly for non-negotiable hard limits, safety boundaries, allergies, or strict technical prohibitions. Place soft preferences under Profile.
   - Directives vs Profile: Directives describe active work, open tasks, and scheduled commitments. General experience or past skills belong under Profile.
   - Entities: Describe named external entities and the user's explicit relationship or context with them.
</extraction_principles>

<output_requirements>
- Output exactly ONE JSON object strictly adhering to <output_schema>.
- All collections except Narrative are JSON arrays of strings. Narrative is a single string.
- Do not output any markdown codeblock formatting or surrounding commentary outside the JSON object.
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
         Analyze the <conversation_history> above and extract all stated facts into the 6 collections from the <output_schema>.\n\
         Output ONLY the JSON object starting with {{ and ending with }}.\n\
         </task>",
        history_text
    );

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
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
