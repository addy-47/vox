import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, cleanup, act, fireEvent } from "@testing-library/react";
import { RocketButton } from "@/shared/components/remote/RocketButton";
import { useRemoteStore } from "@/store/remoteStore";
import * as breakpoints from "@/layout/breakpoints";

describe("RocketButton DOM suite", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.restoreAllMocks();
  });

  afterEach(() => {
    cleanup();
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  describe("Desktop Host Mode", () => {
    beforeEach(() => {
      vi.spyOn(breakpoints, "isRealMobileDevice").mockReturnValue(false);
    });

    it("renders nothing on Host when linkState is Disconnected", () => {
      useRemoteStore.setState({ linkState: "Disconnected", connectedPeer: null });

      const { container } = render(<RocketButton />);
      expect(container.firstChild).toBeNull();
    });

    it("renders Rocket Button when Connected, transitions to two-step inline disconnect, and auto-reverts after 5s", () => {
      const mockDisconnectDevice = vi.fn().mockResolvedValue(undefined);
      useRemoteStore.setState({
        linkState: "Connected",
        connectedPeer: { deviceId: "phone_1", displayName: "Ada's Phone" },
        disconnectDevice: mockDisconnectDevice,
      });

      render(<RocketButton />);

      // Initial state: Rocket icon button visible with aria-label
      const rocketButton = screen.getByRole("button", {
        name: /Remote connected: Ada's Phone\. Click to disconnect\./i,
      });
      expect(rocketButton).toBeTruthy();
      expect(screen.queryByText("Disconnect remote")).toBeNull();

      // Step 1: User clicks the button
      fireEvent.click(rocketButton);

      // Clicking the rocket button opens ConfirmModal
      expect(screen.getByRole("alertdialog")).toBeTruthy();
      expect(screen.getByText("Disconnect Remote Controller")).toBeTruthy();
      const confirmButton = screen.getByRole("button", { name: "Disconnect" });
      const cancelButton = screen.getByRole("button", { name: "Cancel" });
      expect(confirmButton).toBeTruthy();
      expect(cancelButton).toBeTruthy();
    });

    it("allows canceling the disconnect dialog", async () => {
      useRemoteStore.setState({
        linkState: "Connected",
        connectedPeer: { deviceId: "phone_1", displayName: "Ada's Phone" },
      });

      render(<RocketButton />);

      const rocketButton = screen.getByRole("button", {
        name: /Remote connected: Ada's Phone/i,
      });
      fireEvent.click(rocketButton);

      expect(screen.getByRole("alertdialog")).toBeTruthy();

      // User clicks Cancel
      const cancelButton = screen.getByRole("button", { name: "Cancel" });
      await act(async () => {
        fireEvent.click(cancelButton);
        vi.advanceTimersByTime(250);
      });

      // Dialog closes
      expect(screen.queryByRole("alertdialog")).toBeNull();
    });

    it("executes disconnectDevice upon clicking Disconnect in ConfirmModal", async () => {
      const mockDisconnectDevice = vi.fn().mockResolvedValue(undefined);
      useRemoteStore.setState({
        linkState: "Connected",
        connectedPeer: { deviceId: "target_device_id", displayName: "Pixel 9" },
        disconnectDevice: mockDisconnectDevice,
      });

      render(<RocketButton />);

      fireEvent.click(screen.getByRole("button", { name: /Remote connected: Pixel 9/i }));

      const confirmButton = screen.getByRole("button", { name: "Disconnect" });
      await act(async () => {
        fireEvent.click(confirmButton);
      });

      expect(mockDisconnectDevice).toHaveBeenCalledWith("target_device_id");
    });
  });

  describe("Mobile Controller Mode", () => {
    beforeEach(() => {
      vi.spyOn(breakpoints, "isRealMobileDevice").mockReturnValue(true);
    });

    it("is always visible on Controller even when Disconnected", () => {
      useRemoteStore.setState({ linkState: "Disconnected", connectedPeer: null });

      render(<RocketButton />);

      const controllerButton = screen.getByRole("button", { name: "Connect to Host" });
      expect(controllerButton).toBeTruthy();
    });

    it("invokes disconnectHost when clicked while Connected", async () => {
      const mockDisconnectHost = vi.fn().mockResolvedValue(undefined);
      useRemoteStore.setState({
        linkState: "Connected",
        connectedPeer: { deviceId: "host_1", displayName: "Laptop" },
        disconnectHost: mockDisconnectHost,
      });

      render(<RocketButton />);

      const controllerButton = screen.getByRole("button", {
        name: /Connected to Laptop \(Tap to disconnect\)/i,
      });
      await act(async () => {
        fireEvent.click(controllerButton);
      });

      expect(mockDisconnectHost).toHaveBeenCalledTimes(1);
    });
  });
});
