import { describe, it, expect, vi, beforeEach } from "vitest";

const mockListen = vi.fn();

vi.mock("@tauri-apps/api/event", () => ({
  listen: (eventName: string, handler: (event: unknown) => void) => mockListen(eventName, handler),
}));

import {
  on,
  onStateChanged,
  onTranscriptFinal,
  onPttStatus,
  onModeChanged,
  onTelemetry,
  onPipelineError,
  onModelSetupStatus,
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
      const unlistenFn = onStateChanged(handler);

      expect(mockListen).toHaveBeenCalledWith("state_changed", expect.any(Function));

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
      const unlistenFn = on("test_event", handler);

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
    it("should invoke handler with event payload when Tauri emits event", async () => {
      let registeredCallback: ((event: { payload: unknown }) => void) | null = null;
      mockListen.mockImplementationOnce((_evt: string, callback: (event: { payload: unknown }) => void) => {
        registeredCallback = callback;
        return Promise.resolve(vi.fn());
      });

      const handler = vi.fn();
      onTranscriptFinal(handler);

      expect(registeredCallback).not.toBeNull();
      registeredCallback!({ payload: { text: "Hello Vox", turn_id: 1, owner: "MainWindow" } });

      expect(handler).toHaveBeenCalledWith({
        text: "Hello Vox",
        turn_id: 1,
        owner: "MainWindow",
      });
    });

    it("should correctly route ptt_status event payload", async () => {
      let registeredCallback: ((event: { payload: unknown }) => void) | null = null;
      mockListen.mockImplementationOnce((_evt: string, callback: (event: { payload: unknown }) => void) => {
        registeredCallback = callback;
        return Promise.resolve(vi.fn());
      });

      const handler = vi.fn();
      onPttStatus(handler);

      expect(mockListen).toHaveBeenCalledWith("ptt_status", expect.any(Function));
      registeredCallback!({ payload: { state: "RECORDING", session_id: 42 } });
      expect(handler).toHaveBeenCalledWith({ state: "RECORDING", session_id: 42 });
    });

    it("should correctly format target-specific mode_changed event names", async () => {
      mockListen.mockResolvedValue(vi.fn());
      const handler = vi.fn();

      onModeChanged("main", handler);
      expect(mockListen).toHaveBeenCalledWith("mode_changed_main", expect.any(Function));

      onModeChanged("tray", handler);
      expect(mockListen).toHaveBeenCalledWith("mode_changed_tray", expect.any(Function));
    });

    it("should correctly handle telemetry, pipeline_error, and model_setup_status events", async () => {
      mockListen.mockResolvedValue(vi.fn());

      onTelemetry(vi.fn());
      expect(mockListen).toHaveBeenCalledWith("telemetry", expect.any(Function));

      onPipelineError(vi.fn());
      expect(mockListen).toHaveBeenCalledWith("pipeline_error", expect.any(Function));

      onModelSetupStatus(vi.fn());
      expect(mockListen).toHaveBeenCalledWith("model_setup_status", expect.any(Function));
    });
  });
});
