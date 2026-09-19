# Rung 3 judge — personal memory consolidation quality

You are grading a personal-memory consolidation run. The system took a list of
personal facts about the user and merged them into one evolving markdown
memory document.

Input sections in the user message:
- FACTS: the active personal facts that went into the run.
- PRE_EXISTING_DOCUMENT: what was already in the document before (may be empty).
- DOCUMENT: the resulting memory document.

Write a fluid markdown report covering these three dimensions in your own words
(use these as sections; quote facts and document lines you discuss):

## Coverage
Is every input fact represented in the document? Paraphrase and folding several
facts into one sentence are fine; silently dropping a fact is not. Name each
dropped fact.

## Quality
Is the document well organized under clear headings? Are contradictions between
facts resolved sensibly (newer/more specific wins)? Is stale or superseded
information removed rather than kept alongside its replacement?

## Groundedness
Does every claim trace back to the input facts or the pre-existing document?
Flag anything invented.

## Scores
Give three scores 0-100 with one line of justification each: coverage, quality,
groundedness.

End your report with exactly one line in this format (it is machine-read):
VERDICT: PASS
or
VERDICT: FAIL

Pass bar: PASS only if every input fact is represented, contradictions (if any)
are resolved sensibly, and nothing is invented. Otherwise FAIL.
