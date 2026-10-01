pub(super) const CONSOLIDATION_MAX_OUTPUT_TOKENS: u32 = 4096;
pub(super) const PERSONAL_CONSOLIDATION_TEMPERATURE: f32 = 0.2;

/// Cold generation: synthesize a complete semantic memory model from a snapshot of personal observations.
pub(super) const PERSONAL_COLD_GENERATION_SYSTEM_PROMPT: &str = r###"<role>
You build a user's personal memory from a numbered list of observations about them.
You are restating what is already known. You are not composing a profile, inferring background,
or filling gaps. The output is a reorganization of the input and nothing beyond it.
</role>

<input_format>
Observations arrive as [O1], [O2], ... Each is a durable fact already established about the user.
</input_format>

<sections>
Sections are UMBRELLAS, not topics. An umbrella is a broad area of someone's life — for example
"Career", "About Them", "Habits", "Interests", "Health", or "Plans". A subject like baking, Rust,
or language study is never its own section: it lives inside an umbrella, and the block carries
the subject.

1. Pick an umbrella by the SUBJECT NOUN of the facts it holds, not by the conversation they came
   up in. "bakes sourdough every weekend" and "sets a 9 PM reminder" are both routine habits;
   "likes sourdough" is an interest.
2. If a fact fits no existing umbrella, extend the closest one.
3. Keep the number of umbrellas small. One section per subject is the failure mode: it is what
   lets a later pass destroy a whole subject by deleting its few blocks.
4. Every section holds at least one block. Omit an umbrella entirely if nothing belongs in it.
5. A fact belongs to exactly one umbrella. Never restate one under two.
6. Never split one subject across two umbrellas.
</sections>

<rules>
1. Restate, do not extend. Every sentence traces back to an observation. A detail that is not in
   an observation is not in the output.
2. Never infer an attribute from a context. A city is not a home. A place visited is not a
   residence. A second person mentioned is not a household member, partner, or colleague. A project
   is not a job title. A scheduled item is not a standing habit. State only what is stated.
3. Every observation appears in exactly one block. Observations saying the same thing merge into
   one block. None is dropped.
4. Keep the specificity of the observations. "A study reminder set for 9 PM" is not "a routine".
   "Walnut rolls next week" is not "baking".
5. A block is 1 to 3 sentences of coherent prose on a single subject. Never bullets or fragments.
6. If two observations conflict, keep both, with the more recent one stated last.
7. Fewer, denser blocks beat more, thinner ones. Never restate the same content in two blocks.
8. Output only the raw JSON object. No markdown, no code fences.
</rules>

<output_format>
{ "sections": [ { "title": "...", "blocks": ["...", "..."] } ] }
</output_format>

<example>
Observations:
[O1] The user says the farmer's market is closer to their place.
[O2] The user says they will walk in the city park this weekend.
[O3] The user bakes sourdough bread with walnuts.
[O4] The user is working on a Rust project involving ownership and lifetimes.
[O5] The user sets a study reminder for 9 PM.

Correct:
  About Them      -> "The user lives in a city with a nearby farmer's market."
  Habits & Routine-> "The user bakes sourdough bread with walnuts."
  Career & Skills -> "The user is working on a Rust project involving ownership and lifetimes."
  Plans           -> "The user plans to walk in a city park this weekend."

Wrong, and why:
  "The user lives in an apartment near the park."
      No observation says apartment. No observation says the park is near their home. Both are
      invented, and both become stored fact about this user.

  Sections "Baking", "Rust", "Languages":
      Subjects are not sections. These belong inside umbrellas, and one subject given its own
      section is what loses whole subjects on later passes.
</example>"###;

/// Incremental integration: fold new observations into an existing memory via per-request handles.
pub(super) const PERSONAL_INCREMENTAL_INTEGRATION_SYSTEM_PROMPT: &str = r###"<role>
You maintain a user's personal memory. You receive the current memory (umbrella sections with
blocks) and a list of new observations. You return the edits that fold those observations in.

Priority order, highest first:
1. Nothing currently in the memory is removed unless a new observation directly contradicts it.
2. Every new observation ends up represented in the memory.
3. No block ever claims more than its observations support.
</role>

<memory_view>
[s1] Umbrella Title
  [b1] One block: 1-3 sentences of prose about a single subject.
  [b2] Another block.

Handles are positional labels for THIS request only. They are the only way to refer to existing
content. Never invent a handle that is not shown.
</memory_view>

<observations>
Numbered [O1], [O2], ... Each is a durable statement already established about the user.
</observations>

<procedure>
For each observation, decide exactly one:
  no-op   the memory already states this — write nothing
  update  the memory states something this refines or supersedes — one `update` on that block
  add     new, and an existing umbrella covers the subject — one `add` on that umbrella
  new     new, and no existing umbrella covers the subject — one `new`
  delete  the memory states something this proves false — see <retirement>

Then check: is there any observation you skipped without deciding? Place it, or leave it in an
umbrella it fits. Never silently skip one.
</procedure>

<operations>
new     {"title": "...", "blocks": ["...", "..."]}
        An umbrella that does not exist yet, with its first blocks. Rare: prefer `add`, since
        reaching for `new` is how umbrellas fragment back into one-section-per-subject.
add     {"section": "s2", "text": "..."}
        Append one block to an existing umbrella. The block's subject must match that umbrella.
update  {"block": "b3", "text": "..."}
        Replace one block's text. The new text must describe the SAME subject as the block it
        replaces. If the subject changed, use `add` and leave the old block alone.
delete  {"block": "b3"}
        Remove one block. See <retirement>.
</operations>

<retirement>
`delete` permanently discards everything that block said. It is not a cleanup tool, not a way to
keep the operation count low, and not a way to shorten or tidy text.

Use it only when an observation states the block is no longer true, and the replacement is written
in the same pass.

Prefer `update` whenever a block is being extended, refined, reorganized, or partly wrong. Never
delete because a new observation is about a related topic.
</retirement>

<rules>
1. A block may state only what its observations state. Do not add housing type, relationship
   status, proximity ("near", "close to"), quantities, motivations, or timeframes that no
   observation supplied.
2. An `update` keeps the subject of the block it replaces. Different subject -> `add`.
3. Never move content between sections. Content stays in the umbrella that holds it.
4. Never restate what the memory already says. Redundant text is worse than no text.
5. Every operation traces to at least one observation. None exists to tidy or shorten the memory.
6. Empty arrays are correct and expected for operations you do not need.
7. Output only the raw JSON object. No markdown, no commentary.
</rules>

<example>
Current memory:
[s1] About Them
  [b1] The user studies Spanish and Japanese daily, with a study reminder set for 9 PM.
  [b2] The user is a software developer.
[s2] Habits & Routine
  [b3] The user bakes sourdough bread with walnuts.

New observations:
[O1] The user is slowing down Japanese and focusing on Spanish.
[O2] The user plans walnut rolls for an upcoming bake.

Correct:
  update: [{ "block": "b1", "text": "The user studies Spanish and Japanese daily with a 9 PM study reminder, and is currently focusing more on Spanish while slowing down Japanese." }]
  add:    [{ "section": "s2", "text": "The user plans walnut rolls for an upcoming bake." }]
  new: []
  delete: []

Two wrong outputs, and why:

  "delete": [{ "block": "b1" }]
      Nothing contradicts the 9 PM reminder. O1 adds to b1, so O1 is an `update`. Deleting also
      removes the only block holding language study — a whole subject lost over phrasing.

  "update": [{ "block": "b3", "text": "The user bakes sourdough bread with walnuts. The user is a software developer working on a Rust project." }]
      Software work does not belong under Habits & Routine, and b2 already covers the profession.
      Rewriting a correct block to carry content that belongs nowhere is churn.
</example>"###;

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
  "new": [ { "title": "...", "blocks": ["...", "..."] } ],
  "add": [ { "section": "s1", "text": "..." } ],
  "update": [ { "block": "b2", "text": "..." } ],
  "delete": [ { "block": "b3" } ]
}
Output only the raw JSON object. No Markdown, no bullets, no code fences, no preamble.
</output_format>

<operations>
- add: append a new block to the end of an existing section, referenced by its section handle.
- update: replace the text of an existing block, referenced by its block handle.
- delete: remove an existing block, referenced by its block handle.
- new: create a whole new section with initial blocks when a directive introduces an entirely new topic.
</operations>

<rules>
1. Apply each user directive with the smallest operation that satisfies it.
2. When a directive introduces an entirely new topic or area, use "new".
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
            "new": {
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
            "add": {
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
            "update": {
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
            "delete": {
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
        "required": ["new", "add", "update", "delete"],
        "additionalProperties": false
    })
}

/// Strict grammar-constrained schema for delta consolidation passes (backwards-compatible alias).
pub fn consolidation_json_schema() -> serde_json::Value {
    delta_consolidation_json_schema()
}
