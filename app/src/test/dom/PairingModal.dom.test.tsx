import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, cleanup, act } from "@testing-library/react";
import { PairingModal, _resetCachedQrDetails } from "@/shared/components/remote/PairingModal";
import { useRemoteStore } from "@/store/remoteStore";

describe("PairingModal DOM suite", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    _resetCachedQrDetails();
  });

  afterEach(() => {
    cleanup();
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it("renders SVG QR code and network hint upon opening", async () => {
    const mockExpiresAt = Date.now() + 120_000;
    const mockPairNewDevice = vi.fn().mockResolvedValue({
      payload: JSON.stringify({ hint_addr: "192.168.1.42", hint_port: 7700 }),
      qrDataUrl: "data:image/svg+xml;base64,PHN2Zz50ZXN0PC9zdmc+",
      expiresAt: mockExpiresAt,
    });

    useRemoteStore.setState({
      pairNewDevice: mockPairNewDevice,
      linkState: "Disconnected",
    });

    const onClose = vi.fn();
    render(<PairingModal open={true} onClose={onClose} />);

    // Wait for the async generatePairing call to settle
    await act(async () => {
      await Promise.resolve();
    });

    const qrImage = screen.getByAltText<HTMLImageElement>("Remote Pairing QR Code");
    expect(qrImage).toBeTruthy();
    expect(qrImage.src).toBe("data:image/svg+xml;base64,PHN2Zz50ZXN0PC9zdmc+");

    expect(screen.getByText(/Expires in/)).toBeTruthy();
    expect(screen.getByText("2:00")).toBeTruthy();
    expect(screen.getByText("192.168.1.42:7700")).toBeTruthy();
  });

  it("updates countdown timer every second and displays expiration overlay at 0", async () => {
    const mockExpiresAt = Math.floor(Date.now() / 1000) + 120;
    const mockPairNewDevice = vi.fn().mockResolvedValue({
      payload: JSON.stringify({ hint_addr: "127.0.0.1", hint_port: 7700 }),
      qrDataUrl: "data:image/svg+xml;base64,PHN2Zz50ZXN0PC9zdmc+",
      expiresAt: mockExpiresAt,
    });

    useRemoteStore.setState({
      pairNewDevice: mockPairNewDevice,
      linkState: "Disconnected",
    });

    render(<PairingModal open={true} onClose={vi.fn()} />);

    await act(async () => {
      await Promise.resolve();
    });

    expect(screen.getByText("2:00")).toBeTruthy();

    // Advance 1 second
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(screen.getByText("1:59")).toBeTruthy();

    // Advance remaining 119 seconds
    act(() => {
      vi.advanceTimersByTime(119_000);
    });

    expect(screen.getByText("QR Code Expired")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Regenerate" })).toBeTruthy();
  });

  it("automatically closes when linkState transitions to Connected", async () => {
    const mockPairNewDevice = vi.fn().mockResolvedValue({
      payload: "{}",
      qrDataUrl: "data:image/svg+xml;base64,PHN2Zz50ZXN0PC9zdmc+",
      expiresAt: Date.now() + 120_000,
    });

    useRemoteStore.setState({
      pairNewDevice: mockPairNewDevice,
      linkState: "Disconnected",
    });

    const onClose = vi.fn();
    render(<PairingModal open={true} onClose={onClose} />);

    await act(async () => {
      await Promise.resolve();
    });

    expect(onClose).not.toHaveBeenCalled();

    // Host establishes connection while modal is open
    act(() => {
      useRemoteStore.setState({ linkState: "Connected" });
    });

    // Should not close immediately (celebration state is visible)
    expect(onClose).not.toHaveBeenCalled();

    // Advance celebration timer by 800ms
    act(() => {
      vi.advanceTimersByTime(800);
    });

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("cleans up active timers when closed or unmounted", async () => {
    const clearIntervalSpy = vi.spyOn(globalThis, "clearInterval");
    const mockPairNewDevice = vi.fn().mockResolvedValue({
      payload: "{}",
      qrDataUrl: "data:image/svg+xml;base64,PHN2Zz50ZXN0PC9zdmc+",
      expiresAt: Date.now() + 120_000,
    });

    useRemoteStore.setState({
      pairNewDevice: mockPairNewDevice,
      linkState: "Disconnected",
    });

    const { unmount } = render(<PairingModal open={true} onClose={vi.fn()} />);

    await act(async () => {
      await Promise.resolve();
    });

    unmount();
    expect(clearIntervalSpy).toHaveBeenCalled();
    clearIntervalSpy.mockRestore();
  });

  it("transitions in-place to AuthorizeDeviceContent when pendingPairingPrompt is received", async () => {
    const mockPairNewDevice = vi.fn().mockResolvedValue({
      payload: "{}",
      qrDataUrl: "data:image/svg+xml;base64,PHN2Zz50ZXN0PC9zdmc+",
      expiresAt: Date.now() + 120_000,
    });
    const mockApprovePairing = vi.fn().mockResolvedValue(undefined);

    useRemoteStore.setState({
      pairNewDevice: mockPairNewDevice,
      linkState: "Disconnected",
      pendingPairingPrompt: null,
      approvePairing: mockApprovePairing,
    });

    const onClose = vi.fn();
    render(<PairingModal open={true} onClose={onClose} />);

    await act(async () => {
      await Promise.resolve();
    });

    // Initially in QR view
    expect(screen.getByText("Pair Mobile Controller")).toBeTruthy();
    expect(screen.getByAltText("Remote Pairing QR Code")).toBeTruthy();

    // Host receives remote_pairing_requested
    act(() => {
      useRemoteStore.setState({
        pendingPairingPrompt: {
          deviceId: "phone_123",
          displayName: "Ada's Pixel",
        },
      });
      vi.advanceTimersByTime(250);
    });

    // Transitions in-place to Authorize Remote Device view
    expect(screen.getByText("Authorize Remote Device")).toBeTruthy();
    expect(screen.getByText("THIS DEVICE CAN")).toBeTruthy();
    expect(screen.getByText("Stream microphone audio")).toBeTruthy();
    expect(
      screen.getByText("Ada's Pixel is requesting permission to control Vox over your local Wi-Fi network.")
    ).toBeTruthy();

    // Approves from within the same modal
    const approveBtn = screen.getByRole("button", { name: /Approve Connection/i });
    await act(async () => {
      approveBtn.click();
    });

    expect(mockApprovePairing).toHaveBeenCalledWith("Approve");
    expect(onClose).toHaveBeenCalled();
  });
});
