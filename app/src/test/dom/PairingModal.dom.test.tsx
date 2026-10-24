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
});
