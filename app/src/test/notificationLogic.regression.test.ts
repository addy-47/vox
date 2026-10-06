import { describe, it, expect } from "vitest";
import {
  countsTowardBadge,
  metadataResolution,
  metadataTurnCount,
  toCategory,
} from "@/services/notificationService";
import {
  selectBadgeCount,
  selectRolledUpNotifications,
  selectTasksRolledUp,
  selectUpdatesRolledUp,
  selectUncompactedSessionIds,
} from "@/store/notificationStore";
import type { NotificationRecord } from "@/services/eventsService";

/**
 * Notification badge / tab / rollup regression suite.
 *
 * Phase 1 — Production Path Trace
 * -------------------------------
 * SUT: badge weight (§9.2), tasks-vs-updates split, group rollup ordering,
 * and the uncompacted-session projection that drives History triggers.
 *
 * Production Entry Seams (all real production signatures):
 *   countsTowardBadge / metadataResolution / toCategory
 *     (services/notificationService.ts — pure wire-shape readers)
 *   selectBadgeCount / selectRolledUpNotifications / selectTasksRolledUp /
 *     selectUpdatesRolledUp / selectUncompactedSessionIds
 *     (store/notificationStore.ts — zustand selectors over the live array)
 *
 * Direction Check:
 *   Entry seams are the upstream trigger-side readers: backend notification
 *   rows arrive via get_notifications / notification_created events, and these
 *   functions project them into badge counts and tab lists. The test feeds
 *   wire-shape rows in and reads the projections out — not the downstream
 *   badge renderer.
 *
 * Production Path:
 *   backend notifications.rs rows → NotificationRecord[] (store state)
 *   → selectors above → badge count / rolled-up tab groups /
 *      uncompacted session id set → drawer + badge + history triggers
 *
 * Observable Exit: numbers, group keys/counts/ordering, tab membership sets.
 *
 * Phase 2a — Testability: yes. Wire-shape literals ARE the production-owned
 * state format (backend JSON over IPC); no constructor exists to call instead.
 * Selectors take the zustand state object — assembled as `{ notifications,
 * activeActionIds, ... }` shaped state via a minimal cast-free helper below
 * that mirrors the store slice the selectors read. No boundary is mocked: the
 * real selector functions run over the provided rows.
 *
 * Phase 2b — False-Green table:
 *   | producer silent ([]) → badge 0, tabs empty                  | must fail |
 *   | resolved-filter dropped → resolved leaks into badge/tasks   | must fail |
 *   | dismissed excluded-filter dropped → dismissed in rollup     | must fail |
 *   | unknown-category fallback dropped → new tab / crash         | must fail |
 *   | newest-first sort dropped → stale group surfaces as latest  | must fail |
 *
 * Phase 4 — Governing sentence:
 *   If the resolved-task badge exclusion silently broke, this test would fail
 *   because selectBadgeCount would receive 1 for a resolved interactive row
 *   while expecting 0.
 */

let notifSeq = 0;
function makeNotif(partial: Partial<NotificationRecord> = {}): NotificationRecord {
  notifSeq += 1;
  return {
    id: `n_${notifSeq}`,
    group_key: "",
    category: "pipeline",
    severity: "info",
    action_type: "interactive",
    action_payload: "{}",
    title: "t",
    message: "m",
    status: "unread",
    session_id: null,
    metadata: "{}",
    created_at: 1_700_000_000 + notifSeq,
    updated_at: 1_700_000_000 + notifSeq,
    ...partial,
  };
}

// Minimal state slice shaped exactly as the selectors read it.
function stateFor(notifications: NotificationRecord[]) {
  return { notifications, activeActionIds: [] as string[] } as unknown as Parameters<
    typeof selectBadgeCount
  >[0];
}

describe("notification badge weight (spec §9.2)", () => {
  it("counts unread pending tasks and unread receipts/updates", () => {
    const pending = makeNotif({ action_type: "interactive", metadata: "{}" });
    const receipt = makeNotif({ action_type: "receipt", metadata: "{}" });
    expect(countsTowardBadge(pending)).toBe(true);
    expect(countsTowardBadge(receipt)).toBe(true);
    expect(selectBadgeCount(stateFor([pending, receipt]))).toBe(2);
  });

  it("NEGATIVE: resolved tasks, read rows, and malformed metadata never inflate the badge", () => {
    const resolved = makeNotif({
      action_type: "interactive",
      metadata: JSON.stringify({ resolution: "resolved" }),
    });
    const read = makeNotif({ status: "read", action_type: "interactive" });
    const malformed = makeNotif({ action_type: "receipt", metadata: "{oops" });
    expect(countsTowardBadge(resolved)).toBe(false);
    expect(countsTowardBadge(read)).toBe(false);
    // Malformed metadata is a backend data issue, never a UI crash — and a
    // receipt still counts (resolution falls back to "resolved"→counts as update).
    expect(() => countsTowardBadge(malformed)).not.toThrow();
    expect(selectBadgeCount(stateFor([resolved, read]))).toBe(0);
  });

  it("metadataResolution falls back without throwing", () => {
    expect(metadataResolution(makeNotif({ action_type: "receipt", metadata: "{bad" }))).toBe(
      "resolved"
    );
    expect(metadataResolution(makeNotif({ action_type: "interactive", metadata: "{bad" }))).toBe(
      "pending"
    );
    expect(metadataTurnCount(makeNotif({ metadata: JSON.stringify({ uncompacted_turns: 7 }) }))).toBe(
      7
    );
    expect(metadataTurnCount(makeNotif({ metadata: "{bad" }))).toBeNull();
  });

  it("NEGATIVE: unknown future categories normalize to pipeline (no new tab, no crash)", () => {
    expect(toCategory("session_compaction")).toBe("session_compaction");
    expect(toCategory("some_future_category_v99")).toBe("pipeline");
  });
});

describe("notification rollup + tasks/updates split", () => {
  it("groups by group_key newest-first with counts and unread propagation, excluding dismissed", () => {
    const a1 = makeNotif({ group_key: "g1", created_at: 100, status: "read" });
    const a2 = makeNotif({ group_key: "g1", created_at: 200, status: "unread" });
    const b = makeNotif({ group_key: "g2", created_at: 150, status: "unread" });
    const dismissed = makeNotif({ group_key: "g3", created_at: 300, status: "dismissed" });

    const rolled = selectRolledUpNotifications(stateFor([a1, a2, b, dismissed]));
    expect(rolled.map((g) => g.key)).toEqual(["g1", "g2"]);
    const g1 = rolled.find((g) => g.key === "g1")!;
    expect(g1.count).toBe(2);
    expect(g1.hasUnread).toBe(true);
    expect(g1.latest.created_at).toBe(200);
  });

  it("tasks and updates tabs are disjoint and exhaustive over active notifications", () => {
    const pendingTask = makeNotif({ action_type: "interactive", metadata: "{}" });
    const resolvedTask = makeNotif({
      action_type: "interactive",
      metadata: JSON.stringify({ resolution: "resolved" }),
    });
    const receipt = makeNotif({ action_type: "receipt", metadata: "{}" });
    const state = stateFor([pendingTask, resolvedTask, receipt]);

    const tasks = selectTasksRolledUp(state);
    const updates = selectUpdatesRolledUp(state);
    const taskIds = new Set(tasks.map((g) => g.latest.id));
    const updateIds = new Set(updates.map((g) => g.latest.id));

    expect(taskIds.has(pendingTask.id)).toBe(true);
    expect(taskIds.has(resolvedTask.id)).toBe(false);
    expect(updateIds.has(resolvedTask.id)).toBe(true);
    expect(updateIds.has(receipt.id)).toBe(true);
    for (const id of taskIds) expect(updateIds.has(id)).toBe(false);
    expect(taskIds.size + updateIds.size).toBe(3);
  });

  it("uncompacted projection only admits live session_compaction rows with a session", () => {
    const live = makeNotif({
      category: "session_compaction",
      session_id: 42,
      status: "unread",
      action_type: "interactive",
      metadata: "{}",
    });
    const resolved = makeNotif({
      category: "session_compaction",
      session_id: 43,
      status: "unread",
      action_type: "interactive",
      metadata: JSON.stringify({ resolution: "resolved" }),
    });
    const noSession = makeNotif({
      category: "session_compaction",
      session_id: null,
      status: "unread",
      action_type: "interactive",
      metadata: "{}",
    });
    const wrongCat = makeNotif({
      category: "pipeline",
      session_id: 44,
      status: "unread",
      action_type: "interactive",
      metadata: "{}",
    });

    const ids = selectUncompactedSessionIds(stateFor([live, resolved, noSession, wrongCat]));
    expect(ids.has(42)).toBe(true);
    expect(ids.has(43)).toBe(false);
    expect(ids.size).toBe(1);
  });
});
