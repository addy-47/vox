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

  it("renders header, 3 permissions, graphic and countdown timer", () => {
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

    // 3 Privileges (microphone, session control, session state)
    expect(screen.getByText("THIS DEVICE CAN")).toBeTruthy();
    expect(screen.getByText("Microphone access")).toBeTruthy();
    expect(
      screen.getByText("Stream live audio and trigger push-to-talk dictation from this device.")
    ).toBeTruthy();
    expect(screen.getByText("Start, pause, and trigger voice sessions")).toBeTruthy();
    expect(screen.getByText("Control real-time voice interactions and remote tools.")).toBeTruthy();
    expect(screen.getByText("Read session state")).toBeTruthy();
    expect(
      screen.getByText("View real-time session telemetry and pipeline status over local Wi-Fi.")
    ).toBeTruthy();
    expect(screen.queryByText("Trigger push-to-talk")).toBeNull();

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
        timeoutSeconds={25}
        onApprove={vi.fn()}
        onDeny={onDeny}
        onClose={onClose}
      />
    );

    const timerSpan = screen.getByText("00:25");
    expect(timerSpan.className).not.toContain("var(--warning)");
    expect(timerSpan.className).not.toContain("var(--error)");

    act(() => {
      vi.advanceTimersByTime(6000);
    });
    const warningSpan = screen.getByText("00:19");
    expect(warningSpan.className).toContain("text-[rgb(var(--warning))]");

    act(() => {
      vi.advanceTimersByTime(15000);
    });
    const errorSpan = screen.getByText("00:04");
    expect(errorSpan.className).toContain("text-[rgb(var(--error))]");

    await act(async () => {
      vi.advanceTimersByTime(4000);
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
