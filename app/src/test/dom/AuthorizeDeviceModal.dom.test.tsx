import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, cleanup, act } from "@testing-library/react";
import { AuthorizeDeviceModal } from "@/shared/components/remote/AuthorizeDeviceModal";

describe("AuthorizeDeviceModal DOM suite (Image 3 Specification)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    cleanup();
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it("renders header, 4 permissions, graphic and countdown timer per Image 3", () => {
    render(
      <AuthorizeDeviceModal
        open={true}
        timeoutSeconds={30}
        onApprove={vi.fn()}
        onDeny={vi.fn()}
        onClose={vi.fn()}
      />
    );

    // Header per Image 3
    expect(screen.getByText("REMOTE CONTROL")).toBeTruthy();
    expect(screen.getByText("Authorize Remote Device")).toBeTruthy();
    expect(
      screen.getByText(
        "A companion device is requesting permission to control Vox over your local Wi-Fi network."
      )
    ).toBeTruthy();

    // 4 Privileges per Image 3
    expect(screen.getByText("THIS DEVICE CAN")).toBeTruthy();
    expect(screen.getByText("Stream microphone audio")).toBeTruthy();
    expect(screen.getByText("Send live audio from this device into Vox's speech pipeline.")).toBeTruthy();
    expect(screen.getByText("Start, pause, and trigger voice sessions")).toBeTruthy();
    expect(screen.getByText("Control real-time voice interactions and remote tools.")).toBeTruthy();
    expect(screen.getByText("Trigger push-to-talk")).toBeTruthy();
    expect(screen.getByText("Send speech dictation input remotely.")).toBeTruthy();
    expect(screen.getByText("Read session state")).toBeTruthy();
    expect(
      screen.getByText("View real-time session telemetry and pipeline status over local Wi-Fi.")
    ).toBeTruthy();

    // Countdown and microcopy
    expect(screen.getByText("Request expires in")).toBeTruthy();
    expect(screen.getByText("00:30")).toBeTruthy();
    expect(
      screen.getByText(
        "If you don't approve, this request will expire automatically and the device won't be able to connect."
      )
    ).toBeTruthy();

    // Actions
    expect(screen.getByRole("button", { name: /Deny/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Approve Connection/i })).toBeTruthy();
  });

  it("updates countdown timer every second and auto-denies at 0", async () => {
    const onDeny = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    render(
      <AuthorizeDeviceModal
        open={true}
        timeoutSeconds={10}
        onApprove={vi.fn()}
        onDeny={onDeny}
        onClose={onClose}
      />
    );

    expect(screen.getByText("00:10")).toBeTruthy();

    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(screen.getByText("00:07")).toBeTruthy();

    await act(async () => {
      vi.advanceTimersByTime(7000);
    });

    expect(onDeny).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("calls onApprove and closes when clicking Approve Connection", async () => {
    const onApprove = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    render(
      <AuthorizeDeviceModal
        open={true}
        onApprove={onApprove}
        onDeny={vi.fn()}
        onClose={onClose}
      />
    );

    const approveBtn = screen.getByRole("button", { name: /Approve Connection/i });
    await act(async () => {
      approveBtn.click();
    });

    expect(onApprove).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("calls onApprove when Enter key is pressed", async () => {
    const onApprove = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    render(
      <AuthorizeDeviceModal
        open={true}
        onApprove={onApprove}
        onDeny={vi.fn()}
        onClose={onClose}
      />
    );

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
    });

    expect(onApprove).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("calls onDeny and closes when clicking Deny", async () => {
    const onDeny = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    render(
      <AuthorizeDeviceModal
        open={true}
        onApprove={vi.fn()}
        onDeny={onDeny}
        onClose={onClose}
      />
    );

    const denyBtn = screen.getByRole("button", { name: /Deny/i });
    await act(async () => {
      denyBtn.click();
    });

    expect(onDeny).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
