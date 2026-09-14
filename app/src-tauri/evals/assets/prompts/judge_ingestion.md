# Rung 2 judge — deduplication correctness

You are grading a two-stage fact-deduplication run. The system compared incoming
facts against stored facts and either merged them (deactivated the older copy,
kept the incoming one) or kept both as distinct.

Input sections in the user message:
- MERGED_PAIRS: each pair the system treated as duplicates (kept text,
  deactivated text, and which stage decided: exact or semantic).
- SURVIVORS: all facts still active, which the system claims are pairwise distinct.

Write a fluid markdown report covering these three dimensions in your own words
(use these as sections; quote the fact texts you discuss):

## False merges
For each merged pair, are the two texts really saying the same thing? Same
wording with different punctuation/casing counts as duplicate. Two facts sharing
a topic but carrying different information must NOT be merged. List every wrong
merge and why the pair is distinct.

## Missed duplicates
Scan SURVIVORS for pairs that really are duplicates but were kept separate.
Quote both texts for each missed pair.

## Winner direction
For each merged pair, was keeping the incoming text sensible, or was the
deactivated text clearly better (more specific, more current, better phrased)?

## Scores
Give two scores 0-100 with one line of justification each: merge precision
(were merges correct), recall (were dupes caught).

End your report with exactly one line in this format (it is machine-read):
VERDICT: PASS
or
VERDICT: FAIL

Pass bar: PASS only if zero false merges that destroy distinct information and
at most 1 missed duplicate pair. Otherwise FAIL.
