import { describe, it, expect } from "vitest";
import {
  reconcileSessions,
  reconcileProjects,
} from "@/shared/hooks/useSessionPanel";
import type { SessionRow } from "@/services/persistenceService";
import type { ProjectRow } from "@/services/projectsService";

/**
 * Session-panel refresh reconciliation — the invariant that stops the
 * "Session Panel lags heavily" re-render storm.
 *
 * Phase 1 — Production Path Trace
 * -------------------------------
 * SUT (System Under Test):
 *   `refresh()` (useSessionPanel) runs on every `sessions_changed` event, which
 *   fires per conversation turn. `getSessions()` / `getProjects()` deserialize
 *   fresh objects over IPC every time, so a naive `setSessions(fresh)` replaces
 *   every row object and the array itself. That invalidates `SessionPanel`'s
 *   memoized lists and defeats `React.memo` on every `SessionRowItem` /
 *   `ProjectRowItem`, re-rendering the whole panel subtree (42 rows for an
 *   expanded project) even when nothing observable changed.
 *
 * Production Entry Seam:
 *   reconcileSessions / reconcileProjects (shared/hooks/useSessionPanel.ts) —
 *   the real pure functions `refresh()` calls inside `setSessions`/`setProjects`
 *   functional updaters. No mock sits between them and `refresh()`.
 *
 * Direction Check:
 *   This is the write side of the data pipeline (IPC rows → React state
 *   identity → consumer memoization). Testing the rendered DOM would test the
 *   sink; the identity decision is made here.
 *
 * Production Path:
 *   sessions_changed → refresh() → getSessions/getProjects (fresh objects)
 *   → reconcileSessions/reconcileProjects → setState → memoized rows
 *   → render work (or bail-out).
 *
 * Observable Exit:
 *   Array identity (===), element identity (===), and returned values.
 *
 * Phase 2a — Testability: yes. Both functions are pure, exported, synchronous,
 * and their outputs (references) are directly observable. No IPC, no timers.
 *
 * Phase 2b — False-Green table (each row = one-line production mutation):
 *   | reconcileSessions always returns `out` (drop `changed ? out : prev`) | must fail |
 *   | reuse ignores `sameSession` and always returns the fresh row          | must fail |
 *   | `changed` never set on reorder (drop `prev[i] !== old`)              | must fail |
 *   | `changed` never set on length change (drop the init)                 | must fail |
 *   | reuse keyed by index instead of id                                   | must fail |
 *   | reconcileProjects inherits the same defects                          | must fail |
 *
 * Phase 4 — Governing sentence:
 *   If the no-op bail-out broke, this test would fail because a refresh whose
 *   rows are field-identical would return a new array (identity !== prev),
 *   re-rendering the whole panel for nothing.
 */

function session(id: number, overrides: Partial<SessionRow> = {}): SessionRow {
  return {
    id,
    project_id: "default",
    title: `session-${id}`,
    is_pinned: false,
    deleted_at: null,
    created_at: 1_000 + id,
    updated_at: 2_000 + id,
    turn_count: 1,
    first_message: null,
    uncompacted_turns: 0,
    ...overrides,
  };
}

function project(id: string, overrides: Partial<ProjectRow> = {}): ProjectRow {
  return {
    id,
    name: `project-${id}`,
    created_at: 100,
    updated_at: 200,
    ...overrides,
  };
}

describe("reconcileSessions — no-op refresh must preserve identity", () => {
  it("returns the previous array itself when every row is field-identical", () => {
    const prev = [session(1), session(2), session(3)];
    // Fresh deserialized objects with identical fields (what IPC hands back).
    const fresh = [session(1), session(2), session(3)];

    const next = reconcileSessions(prev, fresh);

    expect(next).toBe(prev);
    // StrictMode/React bail-out depends on element identity too.
    next.forEach((row, i) => expect(row).toBe(prev[i]));
  });

  it("keeps every unchanged row instance when only one row changed", () => {
    const prev = [session(1), session(2), session(3)];
    const fresh = [
      session(1),
      session(2, { turn_count: 7, updated_at: 9_999 }),
      session(3),
    ];

    const next = reconcileSessions(prev, fresh);

    expect(next).not.toBe(prev);
    expect(next[0]).toBe(prev[0]);
    expect(next[2]).toBe(prev[2]);
    // The changed row is the fresh object so the UI actually updates.
    expect(next[1]).toBe(fresh[1]);
    expect(next[1].turn_count).toBe(7);
  });

  it("NEGATIVE: a new session id forces a new array and keeps old instances", () => {
    const prev = [session(1), session(2)];
    const fresh = [session(1), session(2), session(3)];

    const next = reconcileSessions(prev, fresh);

    expect(next).not.toBe(prev);
    expect(next).toHaveLength(3);
    expect(next[0]).toBe(prev[0]);
    expect(next[1]).toBe(prev[1]);
    expect(next[2]).toBe(fresh[2]);
  });

  it("NEGATIVE: a reorder produces a new array even when rows are equal", () => {
    const prev = [session(1), session(2)];
    const fresh = [session(2), session(1)];

    const next = reconcileSessions(prev, fresh);

    expect(next).not.toBe(prev);
    // Rows are still reused by id, but order changed so consumers must re-render.
    expect(next[0]).toBe(prev[1]);
    expect(next[1]).toBe(prev[0]);
  });

  it("every observable field breaks reuse", () => {
    const base = session(1);
    const mutations: Array<[string, SessionRow]> = [
      ["project_id", session(1, { project_id: "other" })],
      ["title", session(1, { title: "renamed" })],
      ["is_pinned", session(1, { is_pinned: true })],
      ["deleted_at", session(1, { deleted_at: 5 })],
      ["created_at", session(1, { created_at: 0 })],
      ["updated_at", session(1, { updated_at: 0 })],
      ["turn_count", session(1, { turn_count: 2 })],
      ["first_message", session(1, { first_message: "hi" })],
      ["uncompacted_turns", session(1, { uncompacted_turns: 3 })],
    ];

    for (const [field, mutated] of mutations) {
      const next = reconcileSessions([base], [mutated]);
      expect(next[0], `field ${field} must not be reused`).not.toBe(base);
      expect(next[0][field as keyof SessionRow]).toBe(
        mutated[field as keyof SessionRow]
      );
    }
  });

  it("an empty previous list adopts the fresh list as-is", () => {
    const fresh = [session(1)];
    expect(reconcileSessions([], fresh)).toBe(fresh);
  });
});

describe("reconcileProjects — same identity contract", () => {
  it("returns the previous array when the projects are field-identical", () => {
    const prev = [project("a"), project("b")];
    const fresh = [project("a"), project("b")];

    expect(reconcileProjects(prev, fresh)).toBe(prev);
  });

  it("reuses unchanged projects and adopts the changed one", () => {
    const prev = [project("a"), project("b")];
    const fresh = [project("a"), project("b", { name: "renamed" })];

    const next = reconcileProjects(prev, fresh);

    expect(next).not.toBe(prev);
    expect(next[0]).toBe(prev[0]);
    expect(next[1]).toBe(fresh[1]);
  });

  it("NEGATIVE: a deleted project forces a new array", () => {
    const prev = [project("a"), project("b")];
    const fresh = [project("a")];

    const next = reconcileProjects(prev, fresh);

    expect(next).not.toBe(prev);
    expect(next[0]).toBe(prev[0]);
  });
});
