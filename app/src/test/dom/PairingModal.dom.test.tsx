import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, cleanup, act } from "@testing-library/react";
import { PairingModal } from "@/shared/components/remote/PairingModal";
import { useRemoteStore } from "@/store/remoteStore";

describe("PairingModal DOM suite", () => {
  beforeEach(() => {
    vi.useFakeTimers();
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

  it("renders redesigned security authorization dialog when pendingPairingPrompt is active", async () => {
    const mockApprovePairing = vi.fn().mockResolvedValue(undefined);
    useRemoteStore.setState({
      pendingPairingPrompt: {
        deviceId: "dev_phone_999",
        displayName: "Ada's Pixel",
      },
      approvePairing: mockApprovePairing,
    });

    render(<PairingModal open={true} onClose={vi.fn()} />);

    expect(screen.getByText("Authorize Remote Controller")).toBeTruthy();
    expect(screen.getByText("Ada's Pixel")).toBeTruthy();
    expect(screen.getByText("Fingerprint: dev_phone_999")).toBeTruthy();
    expect(screen.getByText("Granted Controller Privileges")).toBeTruthy();
    expect(screen.getByText("Audio Streaming")).toBeTruthy();
    expect(screen.getByText("Session Steering")).toBeTruthy();
    expect(screen.getByText("Push-to-Talk")).toBeTruthy();
    expect(screen.getByText("Telemetry Sync")).toBeTruthy();

    const authorizeBtn = screen.getByRole("button", { name: /Authorize Device/i });
    const declineBtn = screen.getByRole("button", { name: /Decline/i });
    expect(authorizeBtn).toBeTruthy();
    expect(declineBtn).toBeTruthy();

    // Click Authorize Device
    await act(async () => {
      authorizeBtn.click();
    });
    expect(mockApprovePairing).toHaveBeenCalledWith("Approve");
  });

  it("authorizes when Enter key is pressed during pending authorization", async () => {
    const mockApprovePairing = vi.fn().mockResolvedValue(undefined);
    useRemoteStore.setState({
      pendingPairingPrompt: {
        deviceId: "dev_phone_999",
        displayName: "Ada's Pixel",
      },
      approvePairing: mockApprovePairing,
    });

    render(<PairingModal open={true} onClose={vi.fn()} />);

    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
    });

    expect(mockApprovePairing).toHaveBeenCalledWith("Approve");
  });
});
