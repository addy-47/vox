import { memo, useState, useCallback } from "react";
import { Rocket } from "lucide-react";
import { Tooltip } from "@/shared/ui/Tooltip";
import { ConfirmModal } from "@/shared/ui/ConfirmModal";
import { useRemoteStore } from "@/store/remoteStore";
import { isRealMobileDevice } from "@/layout/breakpoints";
import { QrScannerModal } from "./QrScannerModal";
import { cn } from "@/shared/lib/utils";

/**
 * Header Rocket Button 🚀
 * Controller (mobile): Always visible. Dials host or disconnects.
 * Host (desktop): Hidden until `linkState === "Connected"`.
 * Clicking on Host opens ConfirmModal before disconnecting.
 */
export const RocketButton = memo(() => {
  const linkState = useRemoteStore((s) => s.linkState);
  const connectedPeer = useRemoteStore((s) => s.connectedPeer);
  const pairedDevices = useRemoteStore((s) => s.pairedDevices);
  const disconnectDevice = useRemoteStore((s) => s.disconnectDevice);
  const disconnectHost = useRemoteStore((s) => s.disconnectHost);
  const connectHost = useRemoteStore((s) => s.connectHost);

  const failureReason = useRemoteStore((s) => s.failureReason);

  const isMobile = isRealMobileDevice();
  const isConnected = linkState === "Connected";
  const isConnecting = linkState === "Connecting" || linkState === "Reconnecting";
  const isFailed = linkState === "Failed";
  const pairedHost = pairedDevices.find((d) => d.role === "host") ?? pairedDevices[0] ?? null;

  const [isConfirming, setIsConfirming] = useState(false);
  const [isScannerOpen, setIsScannerOpen] = useState(false);

  const handleConfirmHostDisconnect = useCallback(async () => {
    setIsConfirming(false);
    if (connectedPeer?.deviceId) {
      try {
        await disconnectDevice(connectedPeer.deviceId);
      } catch (e) {
        console.warn("[RocketButton] disconnectDevice error:", e);
      }
    }
  }, [connectedPeer, disconnectDevice]);

  const handleControllerClick = useCallback(async () => {
    if (isConnected) {
      try {
        await disconnectHost();
      } catch (e) {
        console.warn("[RocketButton] disconnectHost error:", e);
      }
    } else if (linkState === "Paired" && pairedHost) {
      try {
        await connectHost({ deviceId: pairedHost.deviceId });
      } catch (e) {
        console.warn("[RocketButton] connectHost error:", e);
      }
    } else if (linkState === "Failed" && pairedHost) {
      // Retry connection on failure click
      try {
        await connectHost({ deviceId: pairedHost.deviceId });
      } catch (e) {
        console.warn("[RocketButton] connectHost error:", e);
      }
    } else {
      setIsScannerOpen(true);
    }
  }, [isConnected, linkState, pairedHost, disconnectHost, connectHost]);

  const handleCloseScanner = useCallback(() => {
    setIsScannerOpen(false);
  }, []);

  // Host: Hidden unless Connected
  if (!isMobile && !isConnected) {
    return null;
  }

  // Host (Desktop): Rocket icon opens ConfirmModal on click
  if (!isMobile) {
    const peerName = connectedPeer?.displayName ?? "Remote Controller";
    return (
      <>
        <Tooltip label={`Connected to ${peerName} (Click to disconnect)`} side="bottom">
          <button
            type="button"
            onClick={() => setIsConfirming(true)}
            className="inline-flex items-center justify-center w-8 h-8 rounded-xl border border-[rgba(var(--accent),0.5)] bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.2)] hover:bg-[rgba(var(--accent),0.2)] transition-all shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))] cursor-pointer animate-pulse-slow"
            aria-label={`Remote connected: ${peerName}. Click to disconnect.`}
          >
            <Rocket size={14} strokeWidth={1.75} />
          </button>
        </Tooltip>

        {isConfirming && (
          <ConfirmModal
            open={isConfirming}
            title="Disconnect Remote Controller"
            description={`Disconnect session with "${peerName}"? You can reconnect anytime.`}
            confirmLabel="Disconnect"
            cancelLabel="Cancel"
            isDestructive={true}
            onConfirm={handleConfirmHostDisconnect}
            onCancel={() => setIsConfirming(false)}
          />
        )}
      </>
    );
  }

  // Controller (Mobile): Always visible
  const controllerTooltip = isConnected
    ? `Connected to ${connectedPeer?.displayName ?? "Host"} (Tap to disconnect)`
    : isConnecting
    ? "Connecting to Host..."
    : isFailed
    ? (failureReason ? `Connection failed: ${failureReason} (Tap to retry)` : "Connection failed (Tap to retry)")
    : linkState === "Unpaired" || !pairedHost
    ? "Pair with Host"
    : "Connect to Host";

  return (
    <>
      <Tooltip label={controllerTooltip} side="bottom">
        <button
          type="button"
          onClick={() => void handleControllerClick()}
          className={cn(
            "relative inline-flex items-center justify-center w-8 h-8 rounded-xl border transition-all shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))] cursor-pointer",
            isConnected
              ? "border-[rgba(var(--accent),0.5)] bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.2)]"
              : isFailed
              ? "border-[rgba(var(--danger),0.5)] bg-[rgba(var(--danger),0.12)] text-[rgb(var(--danger))] shadow-[0_0_12px_rgba(var(--danger),0.2)]"
              : "border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)]"
          )}
          aria-label={controllerTooltip}
        >
          <Rocket
            size={14}
            strokeWidth={1.75}
            className={cn(isConnecting && "animate-spin text-[rgb(var(--accent))]")}
          />
        </button>
      </Tooltip>

      {isScannerOpen && (
        <QrScannerModal
          open={isScannerOpen}
          onClose={handleCloseScanner}
        />
      )}
    </>
  );
});

RocketButton.displayName = "RocketButton";
