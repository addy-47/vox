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
      linkState: "Unpaired",
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
      linkState: "Unpaired",
    });

    render(<PairingModal open={true} onClose={vi.fn()} />);

    await act(async () => {
      await Promise.resolve();
    });

    expect(screen.getByText("2:00")).toBeTruthy();

    // Advance 1 second: 1:59 remaining (foreground color)
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    const timerText = screen.getByText("1:59");
    expect(timerText.className).toContain("text-[rgb(var(--foreground))]");

    // Advance to 20 seconds remaining (100 seconds advanced: 120 - 100 = 20)
    act(() => {
      vi.advanceTimersByTime(99_000);
    });
    const warningText = screen.getByText("0:20");
    expect(warningText.className).toContain("text-[rgb(var(--warning))]");

    // Advance to 5 seconds remaining (15 more seconds advanced: 20 - 15 = 5)
    act(() => {
      vi.advanceTimersByTime(15_000);
    });
    const errorText = screen.getByText("0:05");
    expect(errorText.className).toContain("text-[rgb(var(--error))]");

    // Advance remaining 5 seconds to hit 0
    act(() => {
      vi.advanceTimersByTime(5000);
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
      linkState: "Unpaired",
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
      linkState: "Unpaired",
    });

    const { unmount } = render(<PairingModal open={true} onClose={vi.fn()} />);

    await act(async () => {
      await Promise.resolve();
    });

    unmount();
    expect(clearIntervalSpy).toHaveBeenCalled();
    clearIntervalSpy.mockRestore();
  });

  it("regenerates QR code when clicking Regenerate button", async () => {
    let callCount = 0;
    const mockPairNewDevice = vi.fn().mockImplementation(() => {
      callCount += 1;
      return Promise.resolve({
        payload: JSON.stringify({ hint_addr: "127.0.0.1", hint_port: 7700 }),
        qrDataUrl: `data:image/svg+xml;base64,PHN2Zz50ZXN0${callCount}PC9zdmc+`,
        expiresAt: Math.floor(Date.now() / 1000) + 120,
      });
    });

    useRemoteStore.setState({
      pairNewDevice: mockPairNewDevice,
      linkState: "Unpaired",
    });

    render(<PairingModal open={true} onClose={vi.fn()} />);

    await act(async () => {
      await Promise.resolve();
    });

    expect(mockPairNewDevice).toHaveBeenCalledTimes(1);

    // Fast-forward 125 seconds so QR expires
    act(() => {
      vi.advanceTimersByTime(125_000);
    });

    const regenerateBtn = screen.getByRole("button", { name: "Regenerate" });
    await act(async () => {
      regenerateBtn.click();
    });

    expect(mockPairNewDevice).toHaveBeenCalledTimes(2);
  });
});
