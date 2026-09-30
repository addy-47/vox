/// Cold generation: synthesize a complete semantic model from a snapshot of personal observations.
///
/// The model determines the section taxonomy and the prose block boundaries. It emits `new_sections`
/// only; `creates`, `updates`, and `deletes` are structurally impossible because there is nothing to
/// modify yet.
pub(super) const PERSONAL_COLD_GENERATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory organization engine for an AI assistant.
You receive observations learned about the user across conversations.
You synthesize a structured semantic memory model: titled sections, each holding a small number of
prose blocks.
</role>

<output_format>
A single JSON object with four arrays, all four always present:
{
  "new_sections": [ { "title": "...", "blocks": ["...", "..."] } ],
  "creates": [],
  "updates": [],
  "deletes": []
}
Output only the raw JSON object. No Markdown, no bullets, no code fences, no preamble.
</output_format>

<rules>
1. Organize the observations into sections with descriptive titles. Ordinary names work best, for
   example: About, Career, Skills, Preferences, Hobbies, Health, Relationships, Plans. Give each
   observation a section that genuinely fits; do not lump unrelated observations together.
2. Each block is a semantic unit: 1 to 3 sentences expressing ONE coherent idea. Do not write bullets,
   do not write single fragments, and do not split one idea across several blocks. If an observation
   bundles two ideas, write two blocks.
3. Every title MUST be non-empty and distinctive. Never emit two sections with the same title.
4. Deduplicate and merge related observations. If two observations say the same thing, keep one block.
5. Retain every distinct piece of information present in the input.
6. Do NOT invent, infer, or embellish observations that are not present in the input.
7. Populate ONLY "new_sections". The other three arrays MUST be empty.
</rules>"###;

/// Incremental integration: fold new observations into an existing memory via per-request handles.
///
/// The memory arrives as `[s1] Title` / `[b1] text`. The model proposes the minimal set of grouped
/// operations that folds in the new observations.
pub(super) const PERSONAL_INCREMENTAL_INTEGRATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory consolidation engine. You receive the user's current Personal Memory as
handle-labelled sections and blocks, plus newly learned observations.
You output the smallest set of semantic operations that integrates the new observations.
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
- creates: append a new block to the end of an existing section, referenced by its section handle.
- updates: replace the text of an existing block, referenced by its block handle. The block keeps its
  identity; you are rewording, not replacing it with an unrelated idea.
- deletes: remove an existing block, referenced by its block handle.
- new_sections: create a whole new section, with its initial blocks inline. Use this when an
  observation has no section it genuinely belongs to. Never force it into an unrelated section.
</operations>

<rules>
1. Empty arrays are valid and preferred. If nothing needs creating, emit "creates": [].
2. Never restate the whole memory. Minimal changes only.
3. Each block should express one distinct idea in 1 to 3 sentences. A block is prose, never a bullet.
4. If one block must split into two independent ideas, emit a delete for the old block plus two
   creates into the same section. There is no way to split in place.
5. Do NOT invent observations that are not present in the new observations list.
6. Every fact you write must sit under a section it genuinely belongs to. If an observation fits
   nowhere, leave it out of this pass; a missing observation is fine because this output goes to the
   user for review.
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

<rules>
1. Apply each user directive with the smallest operation that satisfies it.
   - Use updates to reword or correct an existing block.
   - Use creates to add new information to an existing section.
   - Use deletes to remove a block the user asked to discard.
   - Use new_sections when the directive introduces a topic with no home in the current structure.
2. Never restate the whole memory. Minimal changes only.
3. Each block should express one distinct idea in 1 to 3 sentences. A block is prose, never a bullet.
4. Do not act on directives that are not in the user comment list.
5. Empty arrays are valid and preferred.
</rules>"###;

/// Regeneration: full reorganization of the existing semantic memory.
pub(super) const PERSONAL_REGENERATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory reorganization engine for an AI assistant.
You receive the user's existing Personal Memory as handle-labelled sections and blocks.
You produce a complete, better-organized replacement structure.
</role>

<output_format>
A single JSON object with four arrays, all four always present:
{
  "new_sections": [ { "title": "...", "blocks": ["...", "..."] } ],
  "creates": [],
  "updates": [],
  "deletes": []
}
Output only the raw JSON object. No Markdown, no bullets, no code fences, no preamble.
Populate ONLY "new_sections". This is a full replacement, so the other three arrays MUST be empty.
</output_format>

<rules>
1. Preserve every piece of information present in the current memory. This is a reorganization, not an
   edit: nothing may be dropped.
2. Do NOT invent new information.
3. Improve the section taxonomy. Merge sections that overlap, split sections that do not, and rename
   for clarity.
4. Eliminate redundancy. If two blocks say overlapping things, write one better block.
5. Each block is a semantic unit: 1 to 3 sentences expressing ONE coherent idea. Prose, never bullets.
6. Every title MUST be non-empty and distinctive. Never emit two sections with the same title.
7. There is no need to reference handles: every entity in the output is new.
</rules>"###;

/// Strict grammar-constrained schema for every consolidation pass.
///
/// Flat homogeneous arrays grouped by verb, which decode far more reliably than a discriminated
/// union under strict structured output
/// (`semantic-structured-personal-memory-architecture.md` §6.1, §16).
pub fn consolidation_json_schema() -> serde_json::Value {
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

/// Output ceiling for one consolidation pass.
///
/// The payload is a small grouped JSON edit list, never a copy of the document, so this is generous
/// headroom rather than a budget the model can exhaust.
pub(super) const CONSOLIDATION_MAX_OUTPUT_TOKENS: u32 = 4096;

/// Consolidation sampling temperature.
///
/// Deliberately separate from `DEFAULT_LLM_COMPACTION_TEMPERATURE`: compaction and consolidation are
/// different task classes with different calibration, and consolidation runs at most once per
/// session, where a reproducible diff matters more than variety.
pub(super) const PERSONAL_CONSOLIDATION_TEMPERATURE: f32 = 0.2;
