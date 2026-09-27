# Judge — indexed-patch consolidation: coverage, groundedness, anchor survival

You are grading one personal-memory consolidation cycle of an AI assistant.

The system no longer rewrites a memory document. It sends the LLM the current
document as a numbered list of content elements — each heading or bullet carries a
1-based index `[N]` — plus a set of newly learned facts. The LLM returns a list of
**atomic index-addressed operations** (`insert_after` / `replace` / `delete`), each
naming an element `index` and, for insert/replace, the replacement `text`. The
operations are staged for review, then a deterministic engine applies them from the
highest index down to produce the new document. Your job is to grade the
**resulting document**, not the intent of the operations.

Input sections in the user message:
- `NEW_FACTS`: the personal facts the cycle was asked to integrate.
- `BASE_DOCUMENT`: the document before the cycle. This is the accumulated
  memory from every prior cycle.
- `OPERATIONS`: the index-addressed operations the LLM proposed.
- `DOCUMENT`: the resulting document after the operations were applied.

Write a fluid markdown report in your own words, using these sections and
quoting the specific document lines you discuss.

## Coverage
Is every fact in `NEW_FACTS` represented in `DOCUMENT`? Paraphrasing, or folding
several facts into one bullet, is fine. Silently dropping a fact is not. Name
each dropped fact.

## Groundedness
Does every claim in `DOCUMENT` trace back to `NEW_FACTS` or to `BASE_DOCUMENT`?
Flag anything invented — a preference, a name, a project the sources never
mention.

## Anchor Survival
This is the dimension this system exists to get right. Anchors are facts the
user stated in *earlier* cycles that are already in `BASE_DOCUMENT`. The system
guarantees mechanically that no line is edited unless an operation targets it,
so a lost anchor means either an operation targeted a line it should not have,
or a fact that was previously learned has vanished.

For each claim in `BASE_DOCUMENT`: state whether it survives intact in
`DOCUMENT`, was contradicted by newer information (legitimate if `NEW_FACTS`
genuinely supersedes it), or was silently lost (a defect). Judge the claims, not
the formatting. A claim that was correctly replaced because a new fact
supersedes it is NOT a loss.

## Quality
This dimension exists because the document previously degenerated into an
append-only bullet list. Grade it strictly and name every instance you see:
- Is every `##` heading carrying a real descriptive title, or are there bare
  nameless headings? Nameless headings are a defect.
- Do heading titles repeat within the document? Repeated headings are a defect.
- Do any two bullets state the same thing in near-identical wording? Duplicated
  facts are a defect.
- Is superseded information removed rather than left sitting beside its
  replacement? A stale fact surviving next to its own update is a defect.
- Do bullets sit under the heading their content belongs to? A bullet filed
  under an unrelated heading is a defect.
- Does the document read as an organized profile of the user, or as an
  undifferentiated list?

## Scores
Give four integer scores, each 0-100, on their own line in this exact format:

SCORE_COVERAGE: <0-100>
SCORE_QUALITY: <0-100>
SCORE_GROUNDEDNESS: <0-100>
SCORE_ANCHOR_SURVIVAL: <0-100>

## Machine-read claims
After the prose, emit these two blocks. Use `NONE` when there is nothing to
report. One item per line, each prefixed with the tag.

ANCHOR_LOSS: <one base claim that was silently lost, quoted>
ANCHOR_LOSS: <another, or omit>
INVENTION: <one claim in DOCUMENT with no source in NEW_FACTS or BASE_DOCUMENT, quoted>
INVENTION: <another, or omit>

If both are empty, write exactly:

ANCHOR_LOSS: NONE
INVENTION: NONE

End your report with exactly one line in this format (it is machine-read):
VERDICT: PASS
or
VERDICT: FAIL

Pass bar: PASS only if every `NEW_FACTS` entry is represented, every base anchor
survives or was legitimately superseded, and nothing is invented. Otherwise
FAIL.

## Length budget

Keep the entire report under 350 words. Be terse — one or two lines per section.
The machine-read score and claim blocks at the end are mandatory and must always
be emitted, even when the report is long. Never omit them.
