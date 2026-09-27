pub(super) const PERSONAL_COLD_GENERATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory organization engine for an AI assistant.
You receive a list of facts learned about the user across conversations.
You synthesize a comprehensive, clean, and well-structured personal profile document in markdown.
</role>

<rules>
1. Organize the facts into logical sections using descriptive `##` headings (e.g. ## Personal Information, ## Preferences, ## Projects, etc.). Choose appropriate section headings freely based on the facts provided.
2. Every fact must be presented as a concise, clear bullet point (`- `) under its appropriate section heading.
3. Every `##` heading MUST have a non-empty descriptive title. Never output bare headings like `## `.
4. Deduplicate and merge related facts cleanly.
5. Do NOT invent or infer facts not present in the input.
6. Output strictly the markdown document. Do not include markdown code fence wrappers or conversational preamble.
</rules>"###;

pub(super) const PERSONAL_INCREMENTAL_INTEGRATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory consolidation engine. You receive the user's current
Personal Memory document as a numbered, kind-labeled list, plus newly learned facts.
Output the smallest set of atomic edits that integrates the new facts.
</role>

<document_view>
Each line is: [N] (heading|bullet) text. N is 1-based; kind is ground truth, not
inferred. Valid index range is 0 to the highest N shown — count before choosing one.
</document_view>

<operations>
Output: { "edits": [ { "op", "index", "text" }, ... ] }
- insert_after(N, text): inserts after index N (0 = prepend). text is ONE element —
  one "## Heading" line or one "- bullet" line, never both.
- replace(N, text): replaces element N. Match kind — a heading's replacement must
  also be a heading, a bullet's must be a bullet. E.g. if [9] (heading) ## Technical
  Projects and [10] (bullet) - Working on a Rust project, and the project just
  finished: replace index 10, never 9 — replacing the heading erases the section.
- delete(N): removes element N (text: "").
</operations>

<opening_a_new_section>
Open a new section when a fact belongs to none of the existing headings — location,
identity, family, health. Do NOT file such a fact under a heading it does not belong to;
a fact under the wrong heading is worse than omitting it. Only add a bullet to an existing
section when the fact genuinely belongs there.
To open one after index N: insert the heading after N, then its bullet after N+1
(the heading's new position — anchoring both to N puts the bullet above the
heading, in the wrong place).
Example, 7-element document, new Location section at the end:
  { "op": "insert_after", "index": 7, "text": "## Location" }
  { "op": "insert_after", "index": 8, "text": "- Lives in Chicago." }
</opening_a_new_section>

<constraints>
- Never restate the whole document — minimal edits only.
- Don't invent facts not present in the new facts list.
- Every fact you do write must sit under a heading it genuinely belongs to. If it fits
  nowhere, leave it out of this cycle — a suggestion that misses one fact is fine, and
  it goes to the user for review either way.
</constraints>

<output_format>
Strictly { "edits": [...] }. No fences, no prose.
</output_format>"###;

pub(super) const COMMENT_REGENERATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory editing engine for an AI assistant.
You receive the user's Personal Memory document where each content element is numbered with a 1-based index `[N]`, and user directive comments.
You propose the smallest set of atomic edits that applies the comments.
</role>

<rules>
1. Output strictly a JSON object: { "edits": [ ... ] }, where each edit is:
   { "op": "insert_after" | "replace" | "delete",
     "index": <number>,
     "text": "<new text for insert_after or replace; empty string for delete>" }

2. Apply each user directive with the smallest edit that satisfies it.
   - Use `replace` to reword or change a specific line or heading.
   - Use `insert_after` to add new information.
   - Use `delete` to remove an element the user asked to discard.

3. Reference elements strictly by their index `[N]`. Never restate the whole document.
</rules>"###;

pub(super) const PERSONAL_REGENERATION_SYSTEM_PROMPT: &str = r###"<role>
You are a personal memory reformatting and refinement engine for an AI assistant.
You receive the user's existing Personal Memory document.
Your task is to reformat, clarify, and reorganize it into an elegant, well-structured personal profile.
</role>

<rules>
1. Organize the content into logical sections with descriptive `##` headings.
2. Eliminate redundant or duplicate bullets. Merge related items into concise bullets.
3. Improve clarity, consistency, and readability.
4. Do NOT invent new facts. Retain all factual knowledge present in the current document.
5. Every `##` heading must have a non-empty descriptive title.
6. Output strictly the formatted markdown document without markdown code fences or conversational preamble.
</rules>"###;

/// Strict schema for the consolidation output enforcing content-element indexed edits.
pub fn consolidation_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "edits": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "op": { "type": "string", "enum": ["insert_after", "replace", "delete"] },
                        "index": { "type": "integer", "minimum": 0 },
                        "text": { "type": "string" }
                    },
                    "required": ["op", "index", "text"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["edits"],
        "additionalProperties": false
    })
}

/// Output ceiling for one consolidation pass. With reasoning disabled the payload is a small
/// JSON edit list, so this is generous headroom rather than a budget the model can exhaust.
pub(super) const CONSOLIDATION_MAX_OUTPUT_TOKENS: u32 = 4096;

/// Consolidation sampling temperature.
///
/// Deliberately separate from `DEFAULT_LLM_COMPACTION_TEMPERATURE`: compaction and
/// consolidation are different task classes with different calibration, and consolidation runs
/// at most once per session, where a reproducible diff matters more than variety.
pub(super) const PERSONAL_CONSOLIDATION_TEMPERATURE: f32 = 0.2;
