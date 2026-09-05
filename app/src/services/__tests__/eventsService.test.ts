import { describe, it, expect, vi, beforeEach } from "vitest";

const mockListen = vi.fn();

vi.mock("@tauri-apps/api/event", () => ({
  listen: (eventName: string, handler: (event: unknown) => void) => mockListen(eventName, handler),
}));

import {
  on,
  onModelProgress,
  onStateChanged,
  onTranscriptPartial,
  onTranscriptFinal,
  onLlmToken,
  onTelemetry,
  onSystemStats,
} from "../eventsService";

describe("eventsService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Listener Registration & Unlisten Teardowns", () => {
    it("should register listener and invoke unlisten function upon teardown", async () => {
      const mockUnlisten = vi.fn();
      mockListen.mockResolvedValueOnce(mockUnlisten);

      const handler = vi.fn();
      const unlistenFn = onModelProgress(handler);

      expect(mockListen).toHaveBeenCalledWith("model_progress", expect.any(Function));

      // Wait for promise resolution inside on()
      await new Promise((r) => setTimeout(r, 10));

      unlistenFn();
      expect(mockUnlisten).toHaveBeenCalledTimes(1);
    });

    it("should handle early cancellation before listen promise resolves without memory leak", async () => {
      const mockUnlisten = vi.fn();
      let resolveListen: (value: unknown) => void = () => {};
      const listenPromise = new Promise((resolve) => {
        resolveListen = resolve;
      });
      mockListen.mockReturnValueOnce(listenPromise);

      const handler = vi.fn();
      const unlistenFn = on("toggle_tray", handler);

      // Synchronously cancel immediately before promise resolves (simulates fast React unmount)
      unlistenFn();

      // Resolve the Tauri listen promise
      resolveListen(mockUnlisten);
      await listenPromise;

      // Verify mockUnlisten was automatically called upon promise resolution because cancelled == true
      expect(mockUnlisten).toHaveBeenCalledTimes(1);
    });
  });

  describe("Event Payload Propagation", () => {
    it("should correctly handle model_progress events", async () => {
      let registeredCallback: ((event: { payload: unknown }) => void) | null = null;
      mockListen.mockImplementationOnce((_evt: string, callback: (event: { payload: unknown }) => void) => {
        registeredCallback = callback;
        return Promise.resolve(vi.fn());
      });

      const handler = vi.fn();
      onModelProgress(handler);

      expect(mockListen).toHaveBeenCalledWith("model_progress", expect.any(Function));
      registeredCallback!({
        payload: {
          model_id: "qwen2.5-0.5b",
          step: "Downloading",
          progress: 50,
          bytes_downloaded: 500,
          total_bytes: 1000,
          error: null,
        },
      });
      expect(handler).toHaveBeenCalledWith({
        model_id: "qwen2.5-0.5b",
        step: "Downloading",
        progress: 50,
        bytes_downloaded: 500,
        total_bytes: 1000,
        error: null,
      });
    });

    it("should correctly handle state_changed events", async () => {
      let registeredCallback: ((event: { payload: unknown }) => void) | null = null;
      mockListen.mockImplementationOnce((_evt: string, callback: (event: { payload: unknown }) => void) => {
        registeredCallback = callback;
        return Promise.resolve(vi.fn());
      });

      const handler = vi.fn();
      onStateChanged(handler);

      expect(mockListen).toHaveBeenCalledWith("state_changed", expect.any(Function));
      registeredCallback!({
        payload: {
          owner: "Assistant",
          state: "Listening",
          turn_id: 42,
        },
      });
      expect(handler).toHaveBeenCalledWith({
        owner: "Assistant",
        state: "Listening",
        turn_id: 42,
      });
    });

    it("should correctly handle streaming transcript, llm_token, and Error state_changed events", async () => {
      let registeredCallback: ((event: { payload: unknown }) => void) | null = null;
      mockListen.mockImplementation((_evt: string, callback: (event: { payload: unknown }) => void) => {
        registeredCallback = callback;
        return Promise.resolve(vi.fn());
      });

      const tokenHandler = vi.fn();
      onLlmToken(tokenHandler);
      expect(mockListen).toHaveBeenCalledWith("llm_token", expect.any(Function));
      registeredCallback!({ payload: { turn_id: 1, token: "Hello" } });
      expect(tokenHandler).toHaveBeenCalledWith({ turn_id: 1, token: "Hello" });

      const transcriptHandler = vi.fn();
      onTranscriptPartial(transcriptHandler);
      expect(mockListen).toHaveBeenCalledWith("transcript_partial", expect.any(Function));
      registeredCallback!({ payload: { turn_id: 1, text: "Hey vox", owner: "Assistant" } });
      expect(transcriptHandler).toHaveBeenCalledWith({ turn_id: 1, text: "Hey vox", owner: "Assistant" });

      const finalHandler = vi.fn();
      onTranscriptFinal(finalHandler);
      expect(mockListen).toHaveBeenCalledWith("transcript_final", expect.any(Function));
      registeredCallback!({ payload: { turn_id: 1, text: "Hey vox", owner: "Assistant" } });
      expect(finalHandler).toHaveBeenCalledWith({ turn_id: 1, text: "Hey vox", owner: "Assistant" });

      // Pipeline failures surface as canonical Error state, not a bespoke event.
      const errorStateHandler = vi.fn();
      onStateChanged(errorStateHandler);
      expect(mockListen).toHaveBeenCalledWith("state_changed", expect.any(Function));
      registeredCallback!({ payload: { owner: "Assistant", state: "Error", turn_id: 7 } });
      expect(errorStateHandler).toHaveBeenCalledWith({ owner: "Assistant", state: "Error", turn_id: 7 });

      const telemetryHandler = vi.fn();
      onTelemetry(telemetryHandler);
      expect(mockListen).toHaveBeenCalledWith("telemetry", expect.any(Function));
      registeredCallback!({ payload: { energy: 0.5, vad_prob: 0.8, low: 0.1, mid: 0.2, high: 0.3 } });
      expect(telemetryHandler).toHaveBeenCalledWith({ energy: 0.5, vad_prob: 0.8, low: 0.1, mid: 0.2, high: 0.3 });

      const statsHandler = vi.fn();
      onSystemStats(statsHandler);
      expect(mockListen).toHaveBeenCalledWith("system_stats", expect.any(Function));
      registeredCallback!({
        payload: {
          system_cpu: 10,
          system_ram_pct: 40,
          vox_cpu: 2,
          vox_ram_mb: 256,
          threads: 8,
          total_memory_gb: 16,
          cpu_count: 8,
        },
      });
      expect(statsHandler).toHaveBeenCalledWith({
        system_cpu: 10,
        system_ram_pct: 40,
        vox_cpu: 2,
        vox_ram_mb: 256,
        threads: 8,
        total_memory_gb: 16,
        cpu_count: 8,
      });
    });
  });
});
