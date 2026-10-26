import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, cleanup, act, fireEvent } from "@testing-library/react";
import { QrScannerModal } from "@/shared/components/remote/QrScannerModal";
import { useRemoteStore } from "@/store/remoteStore";

describe("QrScannerModal DOM suite", () => {
  let mockPairDevice: ReturnType<typeof vi.fn>;
  let detectedPayload: string | null = null;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.restoreAllMocks();

    mockPairDevice = vi.fn().mockImplementation(() => new Promise(() => {})); // pending promise
    detectedPayload = null;

    useRemoteStore.setState({
      linkState: "Unpaired",
      failureReason: null,
      pairDevice: mockPairDevice as unknown as (params: import("@/services/remoteService").PairDeviceParams) => Promise<void>,
    });

    // Mock HTMLMediaElement prototype for jsdom
    Object.defineProperty(HTMLMediaElement.prototype, "readyState", {
      value: 4,
      configurable: true,
      writable: true,
    });
    HTMLMediaElement.prototype.play = vi.fn().mockResolvedValue(undefined);

    // Mock navigator.mediaDevices.getUserMedia
    Object.defineProperty(navigator, "mediaDevices", {
      value: {
        getUserMedia: vi.fn().mockResolvedValue({
          getVideoTracks: () => [
            {
              getCapabilities: () => ({ torch: true }),
              stop: vi.fn(),
              applyConstraints: vi.fn(),
            },
          ],
          getTracks: () => [{ stop: vi.fn() }],
        }),
      },
      configurable: true,
      writable: true,
    });

    // Mock BarcodeDetector
    class MockBarcodeDetector {
      detect = vi.fn().mockImplementation(async () => {
        if (detectedPayload) {
          const val = detectedPayload;
          detectedPayload = null;
          return [{ rawValue: val }];
        }
        return [];
      });
    }
    (window as unknown as { BarcodeDetector: typeof MockBarcodeDetector }).BarcodeDetector =
      MockBarcodeDetector;
  });

  afterEach(() => {
    cleanup();
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it("calls pairDevice with full payload on QR scan, shows connecting state, and does NOT close synchronously", async () => {
    const onClose = vi.fn();
    render(<QrScannerModal open={true} onClose={onClose} />);

    // Fast-forward past camera init delay (150ms)
    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });

    // Modal renders in portal; query video from document
    const video = document.querySelector("video");
    expect(video).toBeTruthy();
    if (video) {
      await act(async () => {
        fireEvent.playing(video);
      });
    }

    // Supply QR barcode payload
    detectedPayload = JSON.stringify({
      hint_addr: "192.168.1.100",
      hint_port: 7700,
      token: "tok_test_456",
      device_id: "host_id_123",
      public_key: "aabbccddeeff0011",
    });

    // Advance timer to trigger scan interval (120ms) and flush promises
    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });

    // 1. Assert pairDevice is called with full 4-field payload
    expect(mockPairDevice).toHaveBeenCalledWith({
      address: "192.168.1.100:7700",
      deviceId: "host_id_123",
      token: "tok_test_456",
      publicKey: "aabbccddeeff0011",
    });

    // 2. Assert modal is in connecting state
    expect(screen.getByText("Establishing Connection...")).toBeTruthy();

    // 3. Assert onClose is NOT called synchronously
    expect(onClose).not.toHaveBeenCalled();
  });

  it("closes modal after delay when linkState transitions to Paired", async () => {
    const onClose = vi.fn();
    render(<QrScannerModal open={true} onClose={onClose} />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });

    const video = document.querySelector("video");
    expect(video).toBeTruthy();
    if (video) {
      await act(async () => {
        fireEvent.playing(video);
      });
    }

    detectedPayload = JSON.stringify({
      hint_addr: "192.168.1.100",
      hint_port: 7700,
      token: "tok_test_456",
      device_id: "host_id_123",
      public_key: "aabbccddeeff0011",
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });

    expect(screen.getByText("Establishing Connection...")).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();

    // Simulate linkState transition to Paired (Tier 1 completes)
    act(() => {
      useRemoteStore.setState({ linkState: "Paired" });
    });

    // Immediately after transition, celebration shows and waits 800ms
    expect(screen.getAllByText("Host Paired Successfully").length).toBeGreaterThan(0);
    // Verify accent token classes per Ticket 13
    const celebrationWrapper = document.querySelector(".bg-\\[rgba\\(var\\(--accent\\)\\,0\\.12\\)\\]");
    expect(celebrationWrapper).toBeTruthy();
    expect(document.querySelector(".text-\\[rgb\\(var\\(--success\\)\\]")).toBeNull();
    expect(onClose).not.toHaveBeenCalled();

    // Advance timers by 800ms
    act(() => {
      vi.advanceTimersByTime(800);
    });

    // Assert onClose is now called
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("displays error message and does NOT auto-close when linkState transitions to Failed", async () => {
    const onClose = vi.fn();
    render(<QrScannerModal open={true} onClose={onClose} />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });

    const video = document.querySelector("video");
    expect(video).toBeTruthy();
    if (video) {
      await act(async () => {
        fireEvent.playing(video);
      });
    }

    detectedPayload = JSON.stringify({
      hint_addr: "192.168.1.100",
      hint_port: 7700,
      token: "tok_test_456",
      device_id: "host_id_123",
      public_key: "aabbccddeeff0011",
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });

    expect(screen.getByText("Establishing Connection...")).toBeTruthy();

    // Simulate linkState transition to Failed with reason
    act(() => {
      useRemoteStore.setState({
        linkState: "Failed",
        failureReason: "Pairing denied by host: rejected by user",
      });
    });

    // Assert error message and explicit retry button are displayed in DOM
    expect(screen.getByText("Pairing denied by host: rejected by user")).toBeTruthy();
    const retryBtn = screen.getByRole("button", { name: /Scan Again \/ Retry/i });
    expect(retryBtn).toBeTruthy();

    // Clicking retry resets scan error and restarts camera
    await act(async () => {
      fireEvent.click(retryBtn);
      await vi.advanceTimersByTimeAsync(200);
    });

    // Error alert is cleared
    expect(screen.queryByText("Pairing denied by host: rejected by user")).toBeNull();

    // Advance timers — modal must NOT auto-close on error
    act(() => {
      vi.advanceTimersByTime(3000);
    });

    expect(onClose).not.toHaveBeenCalled();
  });

  it("supports manual IP entry and sends address payload without closing immediately", async () => {
    const onClose = vi.fn();
    render(<QrScannerModal open={true} onClose={onClose} defaultManual={true} />);

    const input = screen.getByPlaceholderText("192.168.1.42:7700");
    fireEvent.change(input, { target: { value: "10.0.0.42:8800" } });

    const connectButton = screen.getByRole("button", { name: "Connect to Host" });
    await act(async () => {
      fireEvent.click(connectButton);
      await Promise.resolve();
    });

    expect(mockPairDevice).toHaveBeenCalledWith({
      address: "10.0.0.42:8800",
      deviceId: "",
      token: "",
      publicKey: "",
    });

    expect(screen.getByText("Establishing Connection...")).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });
});
