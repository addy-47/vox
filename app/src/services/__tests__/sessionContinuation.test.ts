import { describe, it, expect, vi, beforeEach } from "vitest";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  selectSession,
  startNewConversation,
  resolveSessionTitle,
  sessionLastActivity,
  sortSessionsNewestFirst,
  formatSessionRecency,
  type SessionRow,
} from "../historyService";
import { SESSION_COPY } from "@/data/sessionCopy";

function row(partial: Partial<SessionRow> & { id: number }): SessionRow {
  return {
    started_at: 1000,
    ended_at: null,
    turn_count: 0,
    first_message: null,
    ...partial,
  };
}

describe("sessionContinuation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("resolveSessionTitle", () => {
    it("prefers the generated title once available", () => {
      expect(
        resolveSessionTitle(row({ id: 1, title: "Trip planning", first_message: "Hello" }))
      ).toBe("Trip planning");
    });

    it("falls back to the first message before a title exists", () => {
      expect(resolveSessionTitle(row({ id: 1, first_message: "Hello there" }))).toBe(
        "Hello there"
      );
    });

    it("uses the untitled placeholder when nothing is stored", () => {
      expect(resolveSessionTitle(row({ id: 1 }))).toBe(SESSION_COPY.untitledSession);
    });

    it("treats blank titles as absent (never stores placeholder echoes)", () => {
      expect(
        resolveSessionTitle(row({ id: 1, title: "   ", first_message: "Hi" }))
      ).toBe("Hi");
      expect(resolveSessionTitle(row({ id: 1, title: "  " }))).toBe(
        SESSION_COPY.untitledSession
      );
    });
  });

  describe("ordering", () => {
    it("sorts newest-first by last activity, falling back to started_at", () => {
      const sessions = [
        row({ id: 1, started_at: 1000 }),
        row({ id: 2, started_at: 3000 }),
        row({ id: 3, started_at: 2000, last_activity: 9000 }),
      ];
      const sorted = sortSessionsNewestFirst(sessions);
      expect(sorted.map((s) => s.id)).toEqual([3, 2, 1]);
      // input not mutated
      expect(sessions.map((s) => s.id)).toEqual([1, 2, 3]);
    });

    it("sessionLastActivity prefers last_activity over started_at", () => {
      expect(sessionLastActivity(row({ id: 1, started_at: 1000 }))).toBe(1000);
      expect(
        sessionLastActivity(row({ id: 1, started_at: 1000, last_activity: 5000 }))
      ).toBe(5000);
    });
  });

  describe("formatSessionRecency", () => {
    const now = new Date("2026-09-05T12:00:00Z").getTime();

    it("buckets sub-minute, minutes, hours, yesterday, and dates", () => {
      expect(formatSessionRecency(now - 30_000, now)).toBe(SESSION_COPY.recency.justNow);
      expect(formatSessionRecency(now - 5 * 60_000, now)).toBe(
        SESSION_COPY.recency.minutesAgo.replace("{n}", "5")
      );
      expect(formatSessionRecency(now - 3 * 3_600_000, now)).toBe(
        SESSION_COPY.recency.hoursAgo.replace("{n}", "3")
      );
      expect(formatSessionRecency(now - 30 * 3_600_000, now)).toBe(
        SESSION_COPY.recency.yesterday
      );
      expect(typeof formatSessionRecency(now - 10 * 86_400_000, now)).toBe("string");
    });
  });

  describe("selectSession / startNewConversation", () => {
    it("selects via backend then loads turns in order", async () => {
      const turns = [
        {
          id: 1,
          session_id: 7,
          turn_id: 1,
          user_text: "Hi",
          assistant_text: "Hello",
          stt_latency_ms: null,
          ttft_ms: null,
          created_at: 100,
        },
      ];
      mockInvoke
        .mockResolvedValueOnce(undefined) // select_session
        .mockResolvedValueOnce(turns); // get_turns
      const res = await selectSession(7);
      expect(mockInvoke).toHaveBeenCalledWith("select_session", { sessionId: 7 });
      expect(mockInvoke).toHaveBeenCalledWith("get_turns", { sessionId: 7 });
      expect(res).toEqual(turns);
    });

    it("falls back to local turns when the backend command is missing", async () => {
      const turns: unknown[] = [];
      mockInvoke
        .mockRejectedValueOnce(new Error("Command select_session not found"))
        .mockResolvedValueOnce(turns);
      const res = await selectSession(7);
      expect(mockInvoke).toHaveBeenCalledWith("get_turns", { sessionId: 7 });
      expect(res).toEqual(turns);
    });

    it("rethrows real backend errors instead of masking them", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("Database locked"));
      await expect(selectSession(7)).rejects.toThrow("Database locked");
    });

    it("starts a new conversation via backend", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      await startNewConversation();
      expect(mockInvoke).toHaveBeenCalledWith("start_new_conversation");
    });

    it("falls back locally when start command is missing", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("Unknown command"));
      await expect(startNewConversation()).resolves.toBeUndefined();
    });
  });
});
