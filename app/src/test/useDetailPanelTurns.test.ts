import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { useDetailPanelTurns } from "@/shared/components/home/recall/useDetailPanelTurns";
import * as persistenceService from "@/services/persistenceService";
import type { SessionRow, TurnRow } from "@/services/persistenceService";

const MOCK_SESSION: SessionRow = {
  id: 42,
  project_id: "test-proj",
  title: "Test Session",
  created_at: 1700000000000,
  updated_at: 1700000100000,
  is_pinned: false,
  deleted_at: null,
  turn_count: 1,
  first_message: "Hi",
  uncompacted_turns: 1,
};

const MOCK_TURNS: TurnRow[] = [
  {
    id: 1,
    session_id: 42,
    turn_id: 1,
    user_text: "Hi",
    assistant_text: "Hello",
    created_at: 1700000010000,
  },
];

describe("useDetailPanelTurns", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("returns empty state when session is null without calling getTurns", () => {
    const getTurnsSpy = vi.spyOn(persistenceService, "getTurns");
    const { result } = renderHook(() => useDetailPanelTurns(null));

    expect(result.current.turns).toEqual([]);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    expect(getTurnsSpy).not.toHaveBeenCalled();
  });

  it("fetches turns when session is provided and updates state", async () => {
    const getTurnsSpy = vi.spyOn(persistenceService, "getTurns").mockResolvedValue(MOCK_TURNS);
    const { result } = renderHook(() => useDetailPanelTurns(MOCK_SESSION));

    expect(getTurnsSpy).toHaveBeenCalledWith(42);

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.turns).toEqual(MOCK_TURNS);
    expect(result.current.error).toBeNull();
  });

  it("handles fetch failure gracefully and sets error string", async () => {
    vi.spyOn(persistenceService, "getTurns").mockRejectedValue(new Error("Database disconnected"));
    const { result } = renderHook(() => useDetailPanelTurns(MOCK_SESSION));

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.turns).toEqual([]);
    expect(result.current.error).toBe("Database disconnected");
  });

  it("retries fetching turns when retry() is called", async () => {
    const getTurnsSpy = vi
      .spyOn(persistenceService, "getTurns")
      .mockRejectedValueOnce(new Error("Network glitch"))
      .mockResolvedValueOnce(MOCK_TURNS);

    const { result } = renderHook(() => useDetailPanelTurns(MOCK_SESSION));

    await waitFor(() => {
      expect(result.current.error).toBe("Network glitch");
    });

    act(() => {
      result.current.retry();
    });

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.turns).toEqual(MOCK_TURNS);
    expect(result.current.error).toBeNull();
    expect(getTurnsSpy).toHaveBeenCalledTimes(2);
  });

  it("does not update state if unmounted while fetch is in-flight", async () => {
    let resolvePromise: (turns: TurnRow[]) => void = () => {};
    const deferred = new Promise<TurnRow[]>((resolve) => {
      resolvePromise = resolve;
    });

    vi.spyOn(persistenceService, "getTurns").mockReturnValue(deferred);

    const { result, unmount } = renderHook(() => useDetailPanelTurns(MOCK_SESSION));

    expect(result.current.loading).toBe(true);

    unmount();

    // Resolve after unmount
    act(() => {
      resolvePromise(MOCK_TURNS);
    });

    // No warning or error thrown; state remains untouched
  });
});
