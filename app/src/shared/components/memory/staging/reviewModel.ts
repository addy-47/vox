import type { MemoryRevisionView } from "@/services/memoryService";
import { diffWords, type DiffToken } from "@/shared/lib/diff";

export interface ParsedBlockItem {
  id?: string;
  text: string;
}

export interface ParsedSectionItem {
  id?: string;
  title: string;
  blocks: ParsedBlockItem[];
}

/** Every pending revision lands in exactly one entry, including orphans, so a
 *  revision the user cannot see is never one they cannot decide on. */
export type ReviewEntry =
  | { kind: "unchanged"; key: string; text: string }
  | { kind: "update"; key: string; revisionId: string; tokens: DiffToken[]; text: string }
  | { kind: "delete"; key: string; revisionId: string; text: string }
  | { kind: "add"; key: string; revisionId: string; text: string };

export interface ReviewSection {
  key: string;
  title: string;
  isNew: boolean;
  revisionId?: string;
  entries: ReviewEntry[];
}

export interface ReviewDocument {
  sections: ReviewSection[];
  placedRevisionIds: Set<string>;
  orphanedRevisions: MemoryRevisionView[];
}

export interface BuildReviewDocumentInput {
  baseSections: ParsedSectionItem[];
  revisions: MemoryRevisionView[];
  parseOpPayload: (content?: string) => {
    text?: string;
    section_id?: string;
    block_id?: string;
    title?: string;
    blocks?: Array<string | { id?: string; text?: string }>;
  };
}

function targetKeysFor(rev: MemoryRevisionView, payload: { block_id?: string; section_id?: string }) {
  const keys: string[] = [];
  const push = (v?: string | null) => {
    if (v && !keys.includes(v)) keys.push(v);
  };
  push(rev.target_id);
  push(payload.block_id);
  push(payload.section_id);
  push(rev.old_text?.trim());
  return keys;
}

export function buildReviewDocument({
  baseSections,
  revisions,
  parseOpPayload,
}: BuildReviewDocumentInput): ReviewDocument {
  const placedRevisionIds = new Set<string>();

  const updatesByKey = new Map<string, MemoryRevisionView>();
  const deletesByKey = new Map<string, MemoryRevisionView>();
  const createsBySection = new Map<string, MemoryRevisionView[]>();
  const newSectionRevisions: MemoryRevisionView[] = [];

  for (const rev of revisions) {
    const payload = parseOpPayload(rev.content);
    if (rev.op === "create_section") {
      newSectionRevisions.push(rev);
    } else if (rev.op === "create_block") {
      const secKey = rev.target_id || payload.section_id || "";
      const list = createsBySection.get(secKey) ?? [];
      list.push(rev);
      createsBySection.set(secKey, list);
    } else {
      const map = rev.op === "update_block" ? updatesByKey : deletesByKey;
      for (const key of targetKeysFor(rev, payload)) {
        if (!map.has(key)) map.set(key, rev);
      }
    }
  }

  const keysForBlock = (blk: ParsedBlockItem) => {
    const keys: string[] = [];
    const push = (v?: string | null) => {
      if (v && !keys.includes(v)) keys.push(v);
    };
    push(blk.id);
    push(blk.text.trim());
    return keys;
  };

  const takeFirst = <T,>(map: Map<string, T>, keys: string[]): T | undefined => {
    for (const key of keys) {
      const hit = map.get(key);
      if (hit) return hit;
    }
    return undefined;
  };

  const sections: ReviewSection[] = baseSections.map((sec, secIdx) => {
    const secKey = sec.id || sec.title;
    const entries: ReviewEntry[] = sec.blocks.map((blk, blkIdx) => {
      const key = blk.id || `blk_${secIdx}_${blkIdx}`;
      const lookupKeys = keysForBlock(blk);

      const update = takeFirst(updatesByKey, lookupKeys);
      if (update) {
        placedRevisionIds.add(update.id);
        const payload = parseOpPayload(update.content);
        const newText = payload.text || update.preview || "";
        return {
          kind: "update" as const,
          key,
          revisionId: update.id,
          tokens: diffWords(update.old_text || blk.text, newText),
          text: newText,
        };
      }

      const del = takeFirst(deletesByKey, lookupKeys);
      if (del) {
        placedRevisionIds.add(del.id);
        return { kind: "delete" as const, key, revisionId: del.id, text: blk.text };
      }

      return { kind: "unchanged" as const, key, text: blk.text };
    });

    const appended = createsBySection.get(secKey) ?? createsBySection.get(sec.title) ?? [];
    for (const create of appended) {
      placedRevisionIds.add(create.id);
      const payload = parseOpPayload(create.content);
      entries.push({
        kind: "add" as const,
        key: create.id,
        revisionId: create.id,
        text: payload.text || create.preview || "",
      });
    }

    return { key: `sec_${secIdx}`, title: sec.title, isNew: false, entries };
  });

  for (const secRev of newSectionRevisions) {
    placedRevisionIds.add(secRev.id);
    const payload = parseOpPayload(secRev.content);
    sections.push({
      key: secRev.id,
      title: payload.title || "New Section",
      isNew: true,
      revisionId: secRev.id,
      entries: (payload.blocks ?? []).map((b, i) => ({
        kind: "add" as const,
        key: `${secRev.id}_blk_${typeof b === "object" ? b.id ?? i : i}`,
        revisionId: secRev.id,
        text: typeof b === "string" ? b : b.text || "",
      })),
    });
  }

  const orphanedRevisions = revisions.filter((r) => !placedRevisionIds.has(r.id));

  if (orphanedRevisions.length > 0) {
    sections.push({
      key: "sec_orphaned",
      title: "Unanchored suggestions",
      isNew: true,
      entries: orphanedRevisions.map((rev): ReviewEntry => {
        placedRevisionIds.add(rev.id);
        const payload = parseOpPayload(rev.content);
        const newText = payload.text || rev.preview || "";
        const key = rev.id;

        if (rev.op === "update_block") {
          return {
            kind: "update",
            key,
            revisionId: rev.id,
            tokens: diffWords(rev.old_text || "", newText),
            text: newText,
          };
        }
        if (rev.op === "delete_block") {
          return {
            kind: "delete",
            key,
            revisionId: rev.id,
            text: rev.old_text || newText,
          };
        }
        return { kind: "add", key, revisionId: rev.id, text: newText };
      }),
    });
  }

  return { sections, placedRevisionIds, orphanedRevisions };
}