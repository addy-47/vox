import { describe, it, expect, vi, beforeEach } from "vitest";

const mockListen = vi.fn();

vi.mock("@tauri-apps/api/event", () => ({
  listen: (eventName: string, handler: (event: unknown) => void) => mockListen(eventName, handler),
}));

import {
  on,
  onModelSetupStatus,
  onOptionalModelComplete,
  onRemoteSetupStatus,
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
      const unlistenFn = onOptionalModelComplete(handler);

      expect(mockListen).toHaveBeenCalledWith("optional_model_complete", expect.any(Function));

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
    it("should correctly handle model_setup_status events", async () => {
      let registeredCallback: ((event: { payload: unknown }) => void) | null = null;
      mockListen.mockImplementationOnce((_evt: string, callback: (event: { payload: unknown }) => void) => {
        registeredCallback = callback;
        return Promise.resolve(vi.fn());
      });

      const handler = vi.fn();
      onModelSetupStatus(handler);

      expect(mockListen).toHaveBeenCalledWith("model_setup_status", expect.any(Function));
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

    it("should correctly handle remote_setup_status events", async () => {
      mockListen.mockResolvedValue(vi.fn());

      onRemoteSetupStatus(vi.fn());
      expect(mockListen).toHaveBeenCalledWith("remote_setup_status", expect.any(Function));
    });
  });
});
