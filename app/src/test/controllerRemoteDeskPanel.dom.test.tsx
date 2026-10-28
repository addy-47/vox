import { describe, it, expect, afterEach } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ControllerRemoteDeskPanel } from "@/shared/components/settings/interaction/ControllerRemoteDeskPanel";
import { useRemoteStore } from "@/store/remoteStore";

afterEach(() => {
  cleanup();
  useRemoteStore.setState({
    linkState: "Unpaired",
    failureReason: null,
    failureKind: null,
    connectedPeer: null,
  });
});

describe("ControllerRemoteDeskPanel DOM behavior", () => {
  it("preserves paired host identity and renders RETRY glyph when linkState is Failed", () => {
    useRemoteStore.setState({
      linkState: "Failed",
      failureReason: "Host unreachable over local network",
      failureKind: "timeout",
      connectedPeer: { deviceId: "host_123", displayName: "Workstation Host" },
    });

    render(<ControllerRemoteDeskPanel />);

    // Must NOT render unpaired prompt
    expect(screen.queryByText(/Pair with desktop/i)).toBeNull();

    // MUST render paired host and failure details
    expect(screen.getByText(/Paired with Workstation Host/i)).toBeTruthy();
    expect(screen.getByText(/Host unreachable over local network/i)).toBeTruthy();

    // Host glyph must show RETRY status
    expect(screen.getByText("RETRY")).toBeTruthy();
    expect(screen.getByRole("button", { name: /Revoke/i })).toBeTruthy();
  });

  it("renders Paired status when linkState is Paired", () => {
    useRemoteStore.setState({
      linkState: "Paired",
      failureReason: null,
      failureKind: null,
      connectedPeer: { deviceId: "host_123", displayName: "Workstation Host" },
    });

    render(<ControllerRemoteDeskPanel />);

    expect(screen.queryByText(/Pair with desktop/i)).toBeNull();
    expect(screen.getByText(/Paired with Workstation Host/i)).toBeTruthy();
    expect(screen.getByText("PAIRED")).toBeTruthy();
  });

  it("renders Connected status when linkState is Connected", () => {
    useRemoteStore.setState({
      linkState: "Connected",
      failureReason: null,
      failureKind: null,
      connectedPeer: { deviceId: "host_123", displayName: "Workstation Host" },
    });

    render(<ControllerRemoteDeskPanel />);

    expect(screen.getByText("CONNECTED")).toBeTruthy();
    expect(screen.getByRole("button", { name: /Revoke/i })).toBeTruthy();
  });

  it("renders Scan action when linkState is Unpaired", () => {
    useRemoteStore.setState({
      linkState: "Unpaired",
      connectedPeer: null,
    });

    render(<ControllerRemoteDeskPanel />);

    expect(screen.getByText(/Pair with desktop/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: /Scan QR code/i })).toBeTruthy();
  });
});
