import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  getTransport,
  setTransport,
  invoke,
  listen,
  convertFileSrc,
  TauriTransport,
  type Transport,
  type UnlistenFn,
} from "../services/transport";

describe("Transport Carrier Layer", () => {
  beforeEach(() => {
    // Reset to default TauriTransport before each test
    setTransport(new TauriTransport());
  });

  it("initializes with default TauriTransport", () => {
    const transport = getTransport();
    expect(transport).toBeInstanceOf(TauriTransport);
  });

  it("allows swapping the active transport to a custom implementation", async () => {
    const invokedCommands: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
    const listeners: Array<{ event: string }> = [];

    const mockInvoke = vi.fn();
    const mockListen = vi.fn();
    const mockConvertFileSrc = vi.fn();

    const mockTransport: Transport = {
      invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
        mockInvoke(cmd, args);
        invokedCommands.push({ cmd, args });
        if (cmd === "mock_cmd") {
          return Promise.resolve({ success: true } as unknown as T);
        }
        return Promise.resolve(null as unknown as T);
      },
      listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<UnlistenFn> {
        mockListen(event, handler);
        listeners.push({ event });
        return Promise.resolve(() => {});
      },
      convertFileSrc(filePath: string, protocol?: string): string {
        mockConvertFileSrc(filePath, protocol);
        return `mock://${protocol ?? "asset"}/${filePath}`;
      },
    };

    setTransport(mockTransport);
    expect(getTransport()).toBe(mockTransport);

    // Test invoke
    const result = await invoke<{ success: boolean }>("mock_cmd", { foo: "bar" });
    expect(result).toEqual({ success: true });
    expect(invokedCommands).toEqual([{ cmd: "mock_cmd", args: { foo: "bar" } }]);
    expect(mockInvoke).toHaveBeenCalledWith("mock_cmd", { foo: "bar" });

    // Test listen
    const unlisten = await listen("test_event", () => {});
    expect(typeof unlisten).toBe("function");
    expect(listeners).toEqual([{ event: "test_event" }]);
    expect(mockListen).toHaveBeenCalled();

    // Test convertFileSrc
    const assetUrl = convertFileSrc("audio/preview.wav");
    expect(assetUrl).toBe("mock://asset/audio/preview.wav");
    expect(mockConvertFileSrc).toHaveBeenCalledWith("audio/preview.wav", undefined);
  });
});
