import { describe, it, expect } from "vitest";
import {
  buildReviewDocument,
  type ParsedSectionItem,
} from "@/shared/components/memory/staging/reviewModel";
import type { MemoryRevisionView } from "@/services/memoryService";

/**
 * reviewModel regression suite — Memory staging review document.
 *
 * Phase 1 — Production Path Trace
 * -------------------------------
 * SUT (System Under Test):
 *   Pending semantic revisions land in exactly one visible review entry,
 *   including new-section string blocks and orphans.
 *
 * Production Entry Seam:
 *   buildReviewDocument({ baseSections, revisions, parseOpPayload }) —
 *   called by SuggestionsReviewView (staging/SuggestionsReviewView.tsx) and
 *   PersonalMemoryStagingCard on every render of the review list.
 *
 * Direction Check:
 *   The entry seam is the upstream TRIGGER-side aggregator: production feeds
 *   backend revision rows + the parsed base document in, and reads
 *   ReviewDocument.sections out. The test invokes the trigger, not the
 *   downstream renderer. A test that rendered SuggestionCard directly would
 *   test the sink, not this pipeline.
 *
 * Production Path:
 *   backend get_memory_revisions rows (MemoryRevisionView[])
 *   → parseOpPayload (JSON.parse with raw-text fallback, production copy in
 *      PersonalMemoryStagingCard.tsx:70 — not exported, see gap note below)
 *   → buildReviewDocument bucketing (updates/deletes/creates/new-sections)
 *   → ReviewDocument.sections (rendered by SuggestionsReviewView)
 *   → observable exit: entry text / kind / revisionId the user decides on
 *
 * Observable Exit:
 *   sections[].entries[].text + kind + placedRevisionIds + orphanedRevisions.
 *
 * Production functions the test calls:
 *   setup:  none needed (pure function, no constructors)
 *   entry:  buildReviewDocument (real production function)
 *   observe: returned ReviewDocument (real downstream output)
 *
 * Phase 2a — Testability: yes (4/4). Pure function, real signature invokable,
 * state assembled from wire-shape literals (backend JSON rows ARE the
 * production-owned state format), output directly observable, no mock of the
 * boundary under test. parseOpPayload is injected by production callers, so a
 * faithful local copy of the 10-line production parser is a valid dependency
 * stub, not a reimplementation of the SUT.
 *
 * Phase 2b — False-Green table (each row is a one-line production mutation):
 *   | producer silent (revisions=[]) → no "add" entries           | must fail |
 *   | string-block branch removed → new-section text ""           | must fail |
 *   | orphan section push removed → unanchored revision invisible | must fail |
 *   | placedRevisionIds.add removed → orphanedRevisions non-empty | must fail |
 *   | update routing dropped → base text shown instead of newText | must fail |
 *
 * Phase 4 — Governing sentence:
 *   If the create_section string-block handoff silently broke, this test would
 *   fail because the "renders string blocks" assertion would receive "" while
 *   expecting the backend-provided fact text.
 */

// Faithful copy of the production parser (PersonalMemoryStagingCard.tsx:70).
// Testability gap note: the production function is module-local and not
// exported, so it cannot be imported. If it drifts, this stub drifts with
// nothing catching it — the durable fix is to export parseOpPayload from a
// shared module and import it here (spec update first per AGENTS.md §4.3).
function parseOpPayload(content?: string): {
  text?: string;
  section_id?: string;
  block_id?: string;
  title?: string;
  blocks?: Array<string | { id?: string; text?: string }>;
} {
  if (!content) return {};
  try {
    return JSON.parse(content);
  } catch {
    return { text: content };
  }
}

let revSeq = 0;
function makeRevision(
  partial: Partial<MemoryRevisionView> & { op: string; content: string }
): MemoryRevisionView {
  revSeq += 1;
  return {
    id: `rev_${revSeq}`,
    target_id: "",
    status: "pending",
    created_at: 1_700_000_000 + revSeq,
    preview: "preview",
    old_text: null,
    ...partial,
  };
}

const BASE: ParsedSectionItem[] = [
  { id: "sec_a", title: "Overview", blocks: [{ id: "blk_1", text: "likes hiking" }] },
];

describe("reviewModel — create_section dual-shape regression (2026-10-05 empty-row bug)", () => {
  it("renders backend string blocks as visible new-fact text (primary gate)", () => {
    const revisions = [
      makeRevision({
        op: "create_section",
        content: JSON.stringify({
          title: "Work",
          blocks: ["owns a bakery", "opens at 6am"],
        }),
      }),
    ];

    const doc = buildReviewDocument({
      baseSections: BASE,
      revisions,
      parseOpPayload,
    });

    const created = doc.sections.find((s) => s.isNew && s.title === "Work");
    expect(created, "new section 'Work' missing from review document").toBeTruthy();
    expect(created!.entries).toHaveLength(2);
    expect(created!.entries.map((e) => e.text)).toEqual([
      "owns a bakery",
      "opens at 6am",
    ]);
    for (const entry of created!.entries) {
      expect(entry.kind).toBe("add");
      expect(entry.text.length).toBeGreaterThan(0);
    }
    expect(doc.orphanedRevisions).toEqual([]);
  });

  it("renders object blocks and falls back to empty string only when text is absent", () => {
    const revisions = [
      makeRevision({
        op: "create_section",
        content: JSON.stringify({
          title: "Prefs",
          blocks: [{ id: "b1", text: "dark mode" }, { id: "b2" }],
        }),
      }),
    ];

    const doc = buildReviewDocument({
      baseSections: BASE,
      revisions,
      parseOpPayload,
    });

    const created = doc.sections.find((s) => s.title === "Prefs");
    expect(created!.entries.map((e) => e.text)).toEqual(["dark mode", ""]);
  });

  it("routes an update_block to its base block with diffed new text", () => {
    const revisions = [
      makeRevision({
        op: "update_block",
        target_id: "blk_1",
        old_text: "likes hiking",
        content: JSON.stringify({ text: "loves hiking" }),
      }),
    ];

    const doc = buildReviewDocument({
      baseSections: BASE,
      revisions,
      parseOpPayload,
    });

    const entry = doc.sections[0].entries[0];
    expect(entry.kind).toBe("update");
    expect(entry.text).toBe("loves hiking");
    expect(doc.orphanedRevisions).toEqual([]);
  });

  it("NEGATIVE: an unmatchable revision is never silently dropped — it lands in Unanchored suggestions", () => {
    const revisions = [
      makeRevision({
        op: "update_block",
        target_id: "blk_does_not_exist",
        old_text: "ghost text nobody has",
        content: JSON.stringify({ text: "new ghost text" }),
      }),
    ];

    const doc = buildReviewDocument({
      baseSections: BASE,
      revisions,
      parseOpPayload,
    });

    // Deterministic absence check on the base section: the ghost must NOT be
    // rendered as an update of the unrelated base block…
    expect(doc.sections[0].entries[0].kind).toBe("unchanged");
    // …and must appear exactly once in the orphan section instead.
    const orphan = doc.sections.find((s) => s.key === "sec_orphaned");
    expect(orphan, "orphan section missing — revision was silently dropped").toBeTruthy();
    expect(orphan!.entries).toHaveLength(1);
    const orphanEntry = orphan!.entries[0];
    expect(orphanEntry.kind).toBe("update");
    if (orphanEntry.kind === "update") {
      expect(orphanEntry.revisionId).toBe(revisions[0].id);
    }
  });

  it("every revision lands in exactly one visible entry (no loss, no duplication)", () => {
    const revisions = [
      makeRevision({
        op: "create_section",
        content: JSON.stringify({ title: "S1", blocks: ["fact one"] }),
      }),
      makeRevision({
        op: "create_block",
        target_id: "sec_a",
        content: JSON.stringify({ text: "appended fact" }),
      }),
      makeRevision({
        op: "delete_block",
        target_id: "blk_1",
        old_text: "likes hiking",
        content: JSON.stringify({}),
      }),
    ];

    const doc = buildReviewDocument({
      baseSections: BASE,
      revisions,
      parseOpPayload,
    });

    const visibleIds = doc.sections.flatMap((s) =>
      s.entries.map((e) => ("revisionId" in e ? e.revisionId : null))
    );
    for (const rev of revisions) {
      expect(
        visibleIds.filter((id) => id === rev.id),
        `revision ${rev.id} must appear exactly once`
      ).toHaveLength(1);
    }
  });
});
