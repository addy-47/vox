pub(super) const CONSOLIDATION_MAX_OUTPUT_TOKENS: u32 = 4096;
pub(super) const PERSONAL_CONSOLIDATION_TEMPERATURE: f32 = 0.2;

/// Cold generation: synthesize a complete semantic memory model from a snapshot of personal observations.
pub(super) const PERSONAL_COLD_GENERATION_SYSTEM_PROMPT: &str = r###"<role>
You are an expert personal memory synthesis engine for an AI assistant.
You receive a set of learned observations about the user gathered across conversations.
Your task is NOT shallow categorization into generic folders. Your task is holistic knowledge synthesis:
1. Understand how observations interconnect across domains (career, technical stack, preferences, habits, projects, personal constraints).
2. Synthesize related observations into cohesive, fluent prose blocks (1 to 3 sentences each) that capture complete concepts with context, rather than disjoint fragments.
3. Group the synthesized knowledge into natural, descriptive sections whose titles emerge directly from the user's profile and life context.
</role>

<output_format>
A single JSON object with a "sections" array:
{
  "sections": [
    {
      "title": "...",
      "blocks": [
        "...",
        "..."
      ]
    }
  ]
}
Output only the raw JSON object. No Markdown, no bullets, no code fences, no preamble.
</output_format>

<rules>
1. Synthesize, do not merely copy: Form a coherent mental model of the user. Where multiple observations touch on related themes synthesize them into unified, high-signal prose statements.
2. Each block is a semantic unit: 1 to 3 sentences expressing a coherent, complete idea with relevant context. Write natural prose; never write bullet points or fragmented phrases.
3. Emergent taxonomy: Choose section titles that genuinely reflect the user's specific context. Avoid generic or empty catch-all buckets.
4. Eliminate duplication and redundancy: If observations express overlapping or repeated information, synthesize them into one definitive block.
5. Absolute fidelity & Zero Extrapolation: Retain all factual information present in the input. NEVER invent, extrapolate, or embellish facts, motives, or rationale. Do NOT infer unstated goals, architecture motivations, or domain significance (e.g. if the input states the user works on ownership in Rust, state only that fact; do NOT extrapolate that it 'reflects a focus on robust architecture' or 'resource lifetimes'). Every detail in every block MUST be directly grounded in the text of the provided observations.
6. Non-empty and unique: Every section must contain at least one block, and every section title must be distinctive and non-empty. Never emit two sections with the same title.
</rules>"###;

/// Incremental integration: fold new observations into an existing memory via per-request handles.
pub(super) const PERSONAL_INCREMENTAL_INTEGRATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory consolidation engine. You receive the user's current Personal Memory as
handle-labelled sections and blocks, plus newly learned observations.
You output the smallest set of semantic operations that integrates the new observations into the memory.
</role>

<memory_view>
The current memory is shown as:
  [s1] Section Title
    [b1] One or two sentences forming a single block.
    [b2] Another block.

`s1` identifies a section, `b1` identifies a block. Block numbers run sequentially across ALL
sections. These handles are the ONLY way to reference existing content. Never invent a handle that is
not shown, and never use a persistent-looking ID.
</memory_view>

<output_format>
A single JSON object with four arrays, all four always present:
{
  "new_sections": [ { "title": "...", "blocks": ["...", "..."] } ],
  "creates": [ { "section": "s1", "text": "..." } ],
  "updates": [ { "block": "b2", "text": "..." } ],
  "deletes": [ { "block": "b3" } ]
}
Output only the raw JSON object. No Markdown, no bullets, no code fences, no preamble.
</output_format>

<operations>
- creates: append a new block to the end of an existing section, referenced by its section handle (e.g. {"section": "s1", "text": "..."}). Use this when an observation expands an existing section topic.
- updates: replace the text of an existing block, referenced by its block handle (e.g. {"block": "b2", "text": "..."}). Use this when an observation updates, refines, or supersedes an existing statement.
- deletes: remove an existing block, referenced by its block handle (e.g. {"block": "b3"}). Use this when an observation directly invalidates or replaces an older statement.
- new_sections: create a whole new section with its initial blocks inline (e.g. {"title": "...", "blocks": ["..."]}). Use this whenever observations introduce a new domain, life area, or distinct topic that does NOT belong to any existing section.
</operations>

<rules>
1. Minimal edits: Only propose operations necessary to integrate the new observations. Do not rewrite or touch blocks that are unchanged.
2. Creating new sections: When new observations introduce a distinct domain or topic not covered by existing sections, ALWAYS create a new section via "new_sections". Never force unrelated observations into an existing section, and NEVER drop or ignore valid observations.
3. CRITICAL — Handle fidelity: The `creates` array accepts ONLY section handles that appear in the `<memory_view>` above (e.g. `s1`, `s2` … up to the last section listed). NEVER invent a handle like `s5` when only `s1`–`s4` exist — if the observation belongs to a new topic, use `new_sections` instead.
4. Synthesize into prose: Each block must express a coherent, complete idea in 1 to 3 sentences of natural prose. Never emit bullet points or fragmented notes.
5. Splitting blocks: If an existing block must split into two independent ideas, emit an update for the existing block plus a create for the new block into that section.
6. Absolute fidelity & Zero Extrapolation: Retain all factual information present in the input. NEVER invent, extrapolate, or embellish facts, motives, or rationale. Do NOT infer unstated goals, architecture motivations, or domain significance. Every statement proposed must be strictly grounded in the new observations.
7. Empty arrays are valid and preferred when an operation type is not needed (e.g. "deletes": []).
</rules>"###;

/// Comment-directed editing: apply the user's directive comments to the existing memory.
pub(super) const COMMENT_DIRECTED_EDIT_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory editing engine for an AI assistant.
You receive the user's current Personal Memory as handle-labelled sections and blocks, plus the user's
own directive comments about what to change.
You output the smallest set of semantic operations that applies those directives.
</role>

<memory_view>
The current memory is shown as:
  [s1] Section Title
    [b1] One or two sentences forming a single block.
    [b2] Another block.

`s1` identifies a section, `b1` identifies a block, numbered sequentially across ALL sections. These
handles are the ONLY way to reference existing content. Never invent a handle that is not shown.
</memory_view>

<output_format>
A single JSON object with four arrays, all four always present:
{
  "new_sections": [ { "title": "...", "blocks": ["...", "..."] } ],
  "creates": [ { "section": "s1", "text": "..." } ],
  "updates": [ { "block": "b2", "text": "..." } ],
  "deletes": [ { "block": "b3" } ]
}
Output only the raw JSON object. No Markdown, no bullets, no code fences, no preamble.
</output_format>

<operations>
- creates: append a new block to the end of an existing section, referenced by its section handle.
- updates: replace the text of an existing block, referenced by its block handle.
- deletes: remove an existing block, referenced by its block handle.
- new_sections: create a whole new section with initial blocks when a directive introduces an entirely new topic.
</operations>

<rules>
1. Apply each user directive with the smallest operation that satisfies it.
2. When a directive introduces an entirely new topic or area, use "new_sections".
3. Never restate the whole memory. Minimal changes only.
4. Each block should express one distinct idea in 1 to 3 sentences of natural prose.
5. Do not act on directives that are not in the user comment list.
6. Empty arrays are valid and preferred when an operation type is not needed.
</rules>"###;


/// Whole-memory schema for cold generation and full regeneration passes.
pub fn whole_memory_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "sections": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "blocks": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["title", "blocks"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["sections"],
        "additionalProperties": false
    })
}

/// Flat grouped delta schema for incremental integration and comment-directed edits.
pub fn delta_consolidation_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "new_sections": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "blocks": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["title", "blocks"],
                    "additionalProperties": false
                }
            },
            "creates": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "section": { "type": "string" },
                        "text": { "type": "string" }
                    },
                    "required": ["section", "text"],
                    "additionalProperties": false
                }
            },
            "updates": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "block": { "type": "string" },
                        "text": { "type": "string" }
                    },
                    "required": ["block", "text"],
                    "additionalProperties": false
                }
            },
            "deletes": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "block": { "type": "string" }
                    },
                    "required": ["block"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["new_sections", "creates", "updates", "deletes"],
        "additionalProperties": false
    })
}

/// Strict grammar-constrained schema for delta consolidation passes (backwards-compatible alias).
pub fn consolidation_json_schema() -> serde_json::Value {
    delta_consolidation_json_schema()
}
