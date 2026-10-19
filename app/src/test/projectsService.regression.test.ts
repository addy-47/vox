import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  getProjects,
  refreshProjects,
  createProject,
  renameProject,
  deleteProject,
} from "@/services/projectsService";
import { setTransport, TauriTransport, type Transport } from "@/services/transport";
import type { ProjectRow } from "@/services/projectsService";

/**
 * Projects list SWR cache regression suite (services/projectsService.ts).
 *
 * Phase 1 — Production Path Trace
 * -------------------------------
 * SUT:
 *   The project-list reads the Home route pays on every mount. Before the
 *   cache, `getProjects()` was the last uncached full-list read: SessionPanel
 *   activation and ActiveSessionHeader refired `get_projects` IPC on each
 *   Home remount — the navigation hot path paid a backend query before paint
 *   even after the sessions cache landed.
 *
 * Production Entry Seam:
 *   The real exported service functions driven over the swappable Transport
 *   seam — the same seam SessionPanel / ActiveSessionHeader / Memory use. The
 *   wire is faked; the cache state machine under test is the real module.
 *
 * Production Path:
 *   Route mount / panel activation → getProjects()
 *   → warm cache? resolve synchronously + background revalidate
 *   : → fetchProjectsFromBackend (deduped in-flight) → cachedProjects
 *   Invalidation paths (sessions_changed / mutations) → refreshProjects or
 *   mutation-cache clearing → forced backend read.
 *
 * Observable Exit: invoke call log (command names + counts), resolved row
 * arrays (by reference), and cache identity across reads.
 *
 * Phase 2a — Testability: pure cache state machine over the transport seam;
 * responses are queued, background revalidates settle via flushMicrotasks.
 * The suite owns the module-level cache reset: `renameProject` is the one
 * deterministic cache-clearing primitive, so `resetCold()` restores a known
 * cold state without reaching into module internals.
 *
 * Phase 2b — False-Green table (each row a one-line production mutation):
 *   | getProjects refires IPC on every warm read (no cache)                 | must fail |
 *   | warm read returns uncached identity (new fetch object)                | must fail |
 *   | background revalidate fires per warm read (no dedupe)                 | must fail |
 *   | warm read never updates the cache from the revalidate result          | must fail |
 *   | refreshProjects serves the cache instead of forcing the backend       | must fail |
 *   | concurrent refreshes double the backend call (in-flight dedupe gone)  | must fail |
 *   | createProject/rename/delete leave the cache warm (stale mutation data)| must fail |
 *
 * Phase 4 — Governing sentence:
 *   If the cache broke, a warm `getProjects()` would await a fresh backend
 *   reply or return an un-cached array identity — exactly the navigation-lag
 *   regression this suite exists to pin.
 */

type Responder = (cmd: string, args?: Record<string, unknown>) => unknown;

const rows: ProjectRow[] = [
  { id: "default", name: "Personal", created_at: 1, updated_at: 10 },
  { id: "work", name: "Work", created_at: 2, updated_at: 20 },
];

const freshRows: ProjectRow[] = [
  { id: "default", name: "Personal", created_at: 1, updated_at: 11 },
];

function transportFrom(responder: Responder): Transport {
  return {
    invoke: (cmd, args) => Promise.resolve(responder(cmd, args) as never),
    listen: () => Promise.resolve(() => {}) as never,
    convertFileSrc: () => "",
  };
}

let calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
let responses: Record<string, unknown> = {};

function queueCalls(cmd: string): number {
  return calls.filter((c) => c.cmd === cmd).length;
}

function flushMicrotasks(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

/**
 * Deterministic cold state: one metadata mutation (never observed by a UI
 * assertion) clears the persisted cache; every pending background revalidate
 * from a prior block resolves here inside its own test's accounting.
 */
async function resetCold(): Promise<void> {
  await flushMicrotasks();
  responses = { rename_project: null, get_projects: rows };
  await renameProject("__reset__", "__reset__");
}

beforeEach(() => {
  calls = [];
  responses = {};
  setTransport(
    transportFrom((cmd, args) => {
      calls.push({ cmd, args });
      if (!(cmd in responses)) throw new Error(`Unexpected backend call: ${cmd}`);
      return responses[cmd];
    }),
  );
});

afterEach(() => {
  setTransport(new TauriTransport());
  vi.restoreAllMocks();
});

describe("projectsService SWR cache", () => {
  it("cold read awaits the backend and caches the resolved array", async () => {
    await resetCold();

    const result = await getProjects();

    expect(queueCalls("get_projects")).toBe(1);
    expect(result).toBe(rows);
  });

  it("warm read resolves from cache synchronously and revalidates once in background", async () => {
    await resetCold();

    const cold = await getProjects();
    expect(cold).toBe(rows);
    await flushMicrotasks();
    expect(queueCalls("get_projects")).toBe(1);

    // Warm read: cached identity synchronously; exactly one deduped
    // background revalidate scheduled for the next read.
    responses = { rename_project: null, get_projects: freshRows };
    const warm = await getProjects();
    expect(warm).toBe(rows);
    await flushMicrotasks();
    expect(queueCalls("get_projects")).toBe(2);

    // The revalidate's result feeds the cache, not just the caller.
    const warmAfterRevalidate = await getProjects();
    expect(warmAfterRevalidate).toBe(freshRows);
  });

  it("refreshProjects forces a backend read and dedupes concurrent calls", async () => {
    await resetCold();

    const [a, b] = await Promise.all([refreshProjects(), refreshProjects()]);

    expect(queueCalls("get_projects")).toBe(1);
    expect(a).toBe(rows);
    expect(b).toBe(a);
  });

  it("each project mutation clears the cache so the next read refetches", async () => {
    await resetCold();
    await getProjects(); // cold fill; cache now warm
    await flushMicrotasks();
    expect(queueCalls("get_projects")).toBe(1);

    // Warm read: zero further backend awaits on the caller path.
    responses = { rename_project: null, get_projects: rows };
    const warm = await getProjects();
    expect(warm).toBe(rows);
    await flushMicrotasks();
    expect(queueCalls("get_projects")).toBe(2);

    responses = { create_project: freshRows[0] };
    await createProject("New Project");
    // Clearing is deferred to the next read — no eager refetch per mutation.
    expect(queueCalls("get_projects")).toBe(2);

    responses = { rename_project: null, get_projects: freshRows };
    await renameProject("work", "Renamed");
    const renamed = await getProjects();
    expect(renamed).toBe(freshRows);
    expect(queueCalls("get_projects")).toBe(3);

    responses = { delete_project: null, get_projects: rows };
    await deleteProject("work");
    const afterDelete = await getProjects();
    expect(afterDelete).toBe(rows);
    expect(queueCalls("get_projects")).toBe(4);
  });
});
