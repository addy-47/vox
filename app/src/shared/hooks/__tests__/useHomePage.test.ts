// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mocked pipeline service primitives (the IPC command contract under test).
// ---------------------------------------------------------------------------
const pipelineMocks = vi.hoisted(() => ({
  engage: vi.fn().mockResolvedValue(undefined),
  startRealtimeSession: vi.fn().mockResolvedValue(undefined),
  stopRealtimeSession: vi.fn().mockResolvedValue(undefined),
  pausePipeline: vi.fn().mockResolvedValue(undefined),
  resumePipeline: vi.fn().mockResolvedValue(undefined),
  pttStart: vi.fn().mockResolvedValue(undefined),
  pttStop: vi.fn().mockResolvedValue(undefined),
  testClip: vi.fn().mockResolvedValue(undefined),
  testClipCancel: vi.fn().mockResolvedValue(undefined),
  getRealtimeSessionCache: vi.fn().mockResolvedValue({ has_session: false }),
  getRuntimeSnapshot: vi.fn().mockResolvedValue({
    is_engaged: false,
    is_sleeping: false,
    conversation_id: 0,
    cpu_governor_optimal: true,
  }),
}));

vi.mock("@/services/pipelineService", () => ({
  engage: pipelineMocks.engage,
  startRealtimeSession: pipelineMocks.startRealtimeSession,
  stopRealtimeSession: pipelineMocks.stopRealtimeSession,
  pausePipeline: pipelineMocks.pausePipeline,
  resumePipeline: pipelineMocks.resumePipeline,
  pttStart: pipelineMocks.pttStart,
  pttStop: pipelineMocks.pttStop,
  testClip: pipelineMocks.testClip,
  testClipCancel: pipelineMocks.testClipCancel,
  getRealtimeSessionCache: pipelineMocks.getRealtimeSessionCache,
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
  describe("Engage (handleEngage)", () => {
    it("invokes engage() when in modular mode", async () => {
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.handleEngage();
      });

      expect(pipelineMocks.engage).toHaveBeenCalledTimes(1);
      expect(pipelineMocks.startRealtimeSession).not.toHaveBeenCalled();
      expect(result.current.isEngaged).toBe(true);
    });

    it("invokes startRealtimeSession() when in realtime mode", async () => {
      settingsState.value = {
        interaction: { mode: "passive", pipeline_mode: "realtime" },
      };
      const { result } = mountHook();
      await flush();

      expect(result.current.pipelineMode).toBe("realtime");

      await act(async () => {
        await result.current.handleEngage();
      });

      expect(pipelineMocks.startRealtimeSession).toHaveBeenCalledTimes(1);
      expect(pipelineMocks.engage).not.toHaveBeenCalled();
    });
  });

  describe("Disengage (handleEnd) — standard session", () => {
    it("invokes engage() to toggle off and never pausePipeline() in modular mode", async () => {
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.handleEngage();
      });
      pipelineMocks.engage.mockClear();

      await act(async () => {
        await result.current.handleEnd();
      });

      expect(pipelineMocks.engage).toHaveBeenCalledTimes(1);
      expect(pipelineMocks.testClipCancel).not.toHaveBeenCalled();
      expect(pipelineMocks.pausePipeline).not.toHaveBeenCalled();
      expect(pipelineMocks.stopRealtimeSession).not.toHaveBeenCalled();
      expect(result.current.isEngaged).toBe(false);
    });

    it("invokes stopRealtimeSession() and never engage()/pausePipeline() in realtime mode", async () => {
      settingsState.value = {
        interaction: { mode: "passive", pipeline_mode: "realtime" },
      };
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.handleEngage();
      });
      pipelineMocks.stopRealtimeSession.mockClear();

      await act(async () => {
        await result.current.handleEnd();
      });

      expect(pipelineMocks.stopRealtimeSession).toHaveBeenCalledTimes(1);
      expect(pipelineMocks.engage).not.toHaveBeenCalled();
      expect(pipelineMocks.testClipCancel).not.toHaveBeenCalled();
      expect(pipelineMocks.pausePipeline).not.toHaveBeenCalled();
      expect(result.current.isEngaged).toBe(false);
    });

    it("clears active transcript and assistant text on disengage", async () => {
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.handleEngage();
      });

      await act(async () => {
        await result.current.handleEnd();
      });

      expect(result.current.transcript).toBe("");
      expect(result.current.assistantText).toBe("");
      expect(result.current.isEngaged).toBe(false);
    });
  });

  describe("Disengage (handleEnd) — test clip playing", () => {
    it("invokes testClipCancel() and never engage()/pausePipeline() when a test clip is active", async () => {
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.handleTestClip("command.wav");
      });
      expect(pipelineMocks.testClip).toHaveBeenCalledWith("command.wav");
      expect(result.current.testingClip).toBe("command.wav");

      pipelineMocks.testClipCancel.mockClear();

      await act(async () => {
        await result.current.handleEnd();
      });

      expect(pipelineMocks.testClipCancel).toHaveBeenCalledTimes(1);
      expect(pipelineMocks.engage).not.toHaveBeenCalled();
      expect(pipelineMocks.pausePipeline).not.toHaveBeenCalled();
      expect(pipelineMocks.stopRealtimeSession).not.toHaveBeenCalled();
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
        await result.current.handleEngage();
      });
      pipelineMocks.testClip.mockClear();

      await act(async () => {
        await result.current.handleTestClip("ignored.wav");
      });

      expect(pipelineMocks.testClip).not.toHaveBeenCalled();
      expect(result.current.testingClip).toBeNull();
    });
  });

  describe("Pause / Resume (handlePause / handleResume)", () => {
    it("invokes pausePipeline() on pause and resumePipeline() on resume", async () => {
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.handleEngage();
      });

      await act(async () => {
        await result.current.handlePause();
      });
      expect(pipelineMocks.pausePipeline).toHaveBeenCalledTimes(1);
      expect(result.current.isPaused).toBe(true);

      await act(async () => {
        await result.current.handleResume();
      });
      expect(pipelineMocks.resumePipeline).toHaveBeenCalledTimes(1);
      expect(result.current.isPaused).toBe(false);
    });

    it("does not invoke pausePipeline() when not engaged", async () => {
      const { result } = mountHook();
      await flush();

      await act(async () => {
        await result.current.handlePause();
      });

      expect(pipelineMocks.pausePipeline).not.toHaveBeenCalled();
    });
  });
});
