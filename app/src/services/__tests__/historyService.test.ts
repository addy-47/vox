import { describe, it, expect, vi, beforeEach } from "vitest";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  getTranscriptHistory,
  commitSessionToHistory,
  getSessions,
  getTurns,
  deleteSession,
  formatDateShort,
  formatDateTime,
} from "../historyService";

describe("historyService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Ephemeral Transcript Buffer IPC", () => {
    it("should fetch transcript history", async () => {
      const mockHistory = ["Line 1", "Line 2"];
      mockInvoke.mockResolvedValueOnce(mockHistory);
      const res = await getTranscriptHistory();
      expect(mockInvoke).toHaveBeenCalledWith("get_transcript_history");
      expect(res).toEqual(mockHistory);
    });

    it("should commit session to history", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      await commitSessionToHistory("Session text");
      expect(mockInvoke).toHaveBeenCalledWith("commit_session_to_history", { text: "Session text" });
    });
  });

  describe("Persisted Sessions & Turns Database IPC", () => {
    it("should fetch sessions", async () => {
      const mockSessions = [
        { id: 1, started_at: 100000, ended_at: 200000, turn_count: 5, first_message: "Hello" },
      ];
      mockInvoke.mockResolvedValueOnce(mockSessions);
      const res = await getSessions();
      expect(mockInvoke).toHaveBeenCalledWith("get_sessions");
      expect(res).toEqual(mockSessions);
    });

    it("should fetch turns for a specific session ID", async () => {
      const mockTurns = [
        {
          id: 10,
          session_id: 1,
          turn_id: 1,
          user_text: "Hi",
          assistant_text: "Hello",
          stt_latency_ms: 50,
          ttft_ms: 120,
          created_at: 100050,
        },
      ];
      mockInvoke.mockResolvedValueOnce(mockTurns);
      const res = await getTurns(1);
      expect(mockInvoke).toHaveBeenCalledWith("get_turns", { session_id: 1 });
      expect(res).toEqual(mockTurns);
    });

    it("should delete session by ID", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      await deleteSession(1);
      expect(mockInvoke).toHaveBeenCalledWith("delete_session", { id: 1 });
    });
  });

  describe("Date Formatting Helpers", () => {
    it("should format timestamps correctly", () => {
      const timestamp = new Date("2026-01-15T12:30:00Z").getTime();
      expect(typeof formatDateShort(timestamp)).toBe("string");
      expect(typeof formatDateTime(timestamp)).toBe("string");
    });
  });
});
