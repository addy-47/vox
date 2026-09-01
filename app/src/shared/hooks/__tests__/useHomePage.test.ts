// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mocked pipeline service primitives (the IPC command contract under test).
// ---------------------------------------------------------------------------
const pipelineMocks = vi.hoisted(() => ({
  startSession: vi.fn().mockResolvedValue(undefined),
  endSession: vi.fn().mockResolvedValue(undefined),
  pauseSession: vi.fn().mockResolvedValue(undefined),
  resumeSession: vi.fn().mockResolvedValue(undefined),
  pttStart: vi.fn().mockResolvedValue(undefined),
  pttStop: vi.fn().mockResolvedValue(undefined),
  pttCancel: vi.fn().mockResolvedValue(undefined),
  testClip: vi.fn().mockResolvedValue(undefined),
  testClipCancel: vi.fn().mockResolvedValue(undefined),
  getRuntimeSnapshot: vi.fn().mockResolvedValue({
    is_engaged: false,
    is_sleeping: false,
    conversation_id: 0,
    cpu_governor_optimal: true,
  }),
}));

vi.mock("@/services/pipelineService", () => ({
  startSession: pipelineMocks.startSession,
  endSession: pipelineMocks.endSession,
  pauseSession: pipelineMocks.pauseSession,
  resumeSession: pipelineMocks.resumeSession,
  pttStart: pipelineMocks.pttStart,
  pttStop: pipelineMocks.pttStop,
  pttCancel: pipelineMocks.pttCancel,
  testClip: pipelineMocks.testClip,
  testClipCancel: pipelineMocks.testClipCancel,
  getRuntimeSnapshot: pipelineMocks.getRuntimeSnapshot,
}));

// ---------------------------------------------------------------------------
// Mocked collaborating services so the hook can mount under jsdom without
// touching real Tauri runtime state.
// ---------------------------------------------------------------------------
const settingsState = vi.hoisted(() => ({
  value: { interaction: { mode: "passive", pipeline_mode: "modular" } },
}));

vi.mock("@/services/settingsService", () => ({
  getSettings: vi.fn().mockImplementation(() => Promise.resolve(settingsState.value)),
}));

vi.mock("@/services/windowService", () => ({
  showMainWindow: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@/services/historyService", () => ({
  getTurns: vi.fn().mockResolvedValue([]),
}));

const telemetryRef = vi.hoisted(() => ({
  current: { energy: 0, vad_prob: 0, low: 0, mid: 0, high: 0 },
}));

vi.mock("@/shared/hooks/useTelemetry", () => ({
  useTelemetry: vi.fn(() => telemetryRef),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    listen: vi.fn().mockResolvedValue(() => {}),
  })),
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
const flush = async () => {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
};

let activeWrapper: ReturnType<typeof renderHook> | null = null;

beforeEach(() => {
  vi.clearAllMocks();
  settingsState.value = {
    interaction: { mode: "passive", pipeline_mode: "modular" },
  };
});

afterEach(() => {
  if (activeWrapper) {
    act(() => activeWrapper!.unmount());
    activeWrapper = null;
  }
});

import { VoiceSessionProvider } from "@/shared/context/VoiceSessionContext";
import { useHomePage } from "@/shared/hooks/useHomePage";

const mountHook = () => {
  const wrapper = renderHook(() => useHomePage(), {
    wrapper: VoiceSessionProvider,
  });
  activeWrapper = wrapper;
  return wrapper;
};

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------
describe("useHomePage voice pipeline lifecycle", () => {
  describe("Engage (engage / handleEngage)", () => {
    it("invokes startSession() unconditionally regardless of pipeline mode", async () => {
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.engage();
      });

      expect(pipelineMocks.startSession).toHaveBeenCalledTimes(1);
      expect(result.current.isEngaged).toBe(true);
    });

    it("invokes startSession() when in realtime mode without mode branching", async () => {
      settingsState.value = {
        interaction: { mode: "passive", pipeline_mode: "realtime" },
      };
      const { result } = mountHook();
      await flush();

      expect(result.current.pipelineMode).toBe("realtime");

      await act(async () => {
        await result.current.engage();
      });

      expect(pipelineMocks.startSession).toHaveBeenCalledTimes(1);
      expect(result.current.isEngaged).toBe(true);
    });
  });

  describe("Disengage (disengage / handleEnd) — standard session", () => {
    it("invokes endSession() unconditionally and never pauseSession()", async () => {
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.engage();
      });
      pipelineMocks.startSession.mockClear();

      await act(async () => {
        await result.current.disengage();
      });

      expect(pipelineMocks.endSession).toHaveBeenCalledTimes(1);
      expect(pipelineMocks.testClipCancel).not.toHaveBeenCalled();
      expect(pipelineMocks.pauseSession).not.toHaveBeenCalled();
      expect(result.current.isEngaged).toBe(false);
    });

    it("clears active transcript and assistant text on disengage", async () => {
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.engage();
      });

      await act(async () => {
        await result.current.disengage();
      });

      expect(result.current.transcript).toBe("");
      expect(result.current.assistantText).toBe("");
      expect(result.current.isEngaged).toBe(false);
    });
  });

  describe("Disengage (disengage / handleEnd) — test clip playing", () => {
    it("invokes testClipCancel() and never endSession()/pauseSession() when a test clip is active", async () => {
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.handleTestClip("command.wav");
      });
      expect(pipelineMocks.testClip).toHaveBeenCalledWith("command.wav");
      expect(result.current.testingClip).toBe("command.wav");

      pipelineMocks.testClipCancel.mockClear();

      await act(async () => {
        await result.current.disengage();
      });

      expect(pipelineMocks.testClipCancel).toHaveBeenCalledTimes(1);
      expect(pipelineMocks.endSession).not.toHaveBeenCalled();
      expect(pipelineMocks.pauseSession).not.toHaveBeenCalled();
      expect(result.current.testingClip).toBeNull();
      expect(result.current.isEngaged).toBe(false);
    });
  });

  describe("Test clip (handleTestClip)", () => {
    it("sets testingClip state and engages the pipeline", async () => {
      const { result } = mountHook();
      await flush();

      expect(result.current.isEngaged).toBe(false);
      expect(result.current.testingClip).toBeNull();

      await act(async () => {
        await result.current.handleTestClip("alert.wav");
      });

      expect(pipelineMocks.testClip).toHaveBeenCalledWith("alert.wav");
      expect(result.current.testingClip).toBe("alert.wav");
      expect(result.current.isEngaged).toBe(true);
    });

    it("does nothing while already engaged (guards against double-engage)", async () => {
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.engage();
      });
      pipelineMocks.testClip.mockClear();

      await act(async () => {
        await result.current.handleTestClip("ignored.wav");
      });

      expect(pipelineMocks.testClip).not.toHaveBeenCalled();
      expect(result.current.testingClip).toBeNull();
    });
  });

  describe("Pause / Resume (pause / resume)", () => {
    it("invokes pauseSession() on pause and resumeSession() on resume", async () => {
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.engage();
      });

      await act(async () => {
        await result.current.pause();
      });
      expect(pipelineMocks.pauseSession).toHaveBeenCalledTimes(1);
      expect(result.current.isPaused).toBe(true);

      await act(async () => {
        await result.current.resume();
      });
      expect(pipelineMocks.resumeSession).toHaveBeenCalledTimes(1);
      expect(result.current.isPaused).toBe(false);
    });

    it("does not invoke pauseSession() when not engaged", async () => {
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.pause();
      });

      expect(pipelineMocks.pauseSession).not.toHaveBeenCalled();
    });
  });

  describe("Push-To-Talk (handlePttStart / handlePttStop / handlePttCancel)", () => {
    it("invokes pttStart(), pttStop(), and pttCancel()", async () => {
      settingsState.value = {
        interaction: { mode: "ptt", pipeline_mode: "modular" },
      };
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.engage();
      });

      await act(async () => {
        await result.current.handlePttStart();
      });
      expect(pipelineMocks.pttStart).toHaveBeenCalledTimes(1);

      await act(async () => {
        await result.current.handlePttStop();
      });
      expect(pipelineMocks.pttStop).toHaveBeenCalledTimes(1);

      await act(async () => {
        await result.current.handlePttCancel();
      });
      expect(pipelineMocks.pttCancel).toHaveBeenCalledTimes(1);
    });
  });
});
