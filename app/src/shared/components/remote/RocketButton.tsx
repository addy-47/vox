import { memo, useState, useCallback, useEffect, useRef } from "react";
import { Rocket, Check, X } from "lucide-react";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useRemoteStore } from "@/store/remoteStore";
import { isRealMobileDevice } from "@/layout/breakpoints";
import { QrScannerModal } from "./QrScannerModal";
import { cn } from "@/shared/lib/utils";

/**
 * Header Rocket Button 🚀
 * Controller (mobile): Always visible. Dials host or disconnects.
 * Host (desktop): Hidden until `linkState === "Connected"`.
 * Two-step unboxed inline transition for disconnect with 5-second auto-revert.
 */
export const RocketButton = memo(() => {
  const linkState = useRemoteStore((s) => s.linkState);
  const connectedPeer = useRemoteStore((s) => s.connectedPeer);
  const disconnectDevice = useRemoteStore((s) => s.disconnectDevice);
  const disconnectHost = useRemoteStore((s) => s.disconnectHost);

  const isMobile = isRealMobileDevice();
  const isConnected = linkState === "Connected";
  const isConnecting = linkState === "Connecting" || linkState === "Reconnecting";

  const [isConfirming, setIsConfirming] = useState(false);
  const [isScannerOpen, setIsScannerOpen] = useState(false);
  const revertTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearRevertTimer = useCallback(() => {
    if (revertTimerRef.current !== null) {
      clearTimeout(revertTimerRef.current);
      revertTimerRef.current = null;
    }
  }, []);

  const handleStartConfirm = useCallback(() => {
    setIsConfirming(true);
    clearRevertTimer();
    revertTimerRef.current = setTimeout(() => {
      setIsConfirming(false);
    }, 5000);
  }, [clearRevertTimer]);

  const handleCancelConfirm = useCallback(() => {
    clearRevertTimer();
    setIsConfirming(false);
  }, [clearRevertTimer]);

  const handleConfirmHostDisconnect = useCallback(async () => {
    clearRevertTimer();
    setIsConfirming(false);
    if (connectedPeer?.deviceId) {
      try {
        await disconnectDevice(connectedPeer.deviceId);
      } catch (e) {
        console.warn("[RocketButton] disconnectDevice error:", e);
      }
    }
  }, [connectedPeer, disconnectDevice, clearRevertTimer]);

  const handleControllerClick = useCallback(async () => {
    if (isConnected) {
      try {
        await disconnectHost();
      } catch (e) {
        console.warn("[RocketButton] disconnectHost error:", e);
      }
    } else {
      setIsScannerOpen(true);
    }
  }, [isConnected, disconnectHost]);

  useEffect(() => {
    return () => clearRevertTimer();
  }, [clearRevertTimer]);

  // Host: Hidden unless Connected
  if (!isMobile && !isConnected) {
    return null;
  }

  // Host (Desktop): Two-step unboxed inline disconnect
  if (!isMobile) {
    if (isConfirming) {
      return (
        <div className="inline-flex items-center gap-1.5 h-8 animate-fade-in shrink-0">
          <span className="text-[11px] font-bold tracking-wide text-[rgb(var(--accent))] select-none whitespace-nowrap">
            Disconnect remote
          </span>
          <div className="flex items-center gap-0.5">
            <Tooltip label="Confirm disconnect" side="bottom">
              <button
                type="button"
                onClick={() => void handleConfirmHostDisconnect()}
                className="p-1 text-[rgb(var(--danger))] hover:text-[rgb(var(--danger))] hover:scale-115 transition-all cursor-pointer"
                aria-label="Confirm disconnect"
              >
                <Check size={13} strokeWidth={2.5} />
              </button>
            </Tooltip>
            <Tooltip label="Cancel" side="bottom">
              <button
                type="button"
                onClick={handleCancelConfirm}
                className="p-1 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:scale-115 transition-all cursor-pointer"
                aria-label="Cancel disconnect"
              >
                <X size={13} strokeWidth={2} />
              </button>
            </Tooltip>
          </div>
        </div>
      );
    }

    const peerName = connectedPeer?.displayName ?? "Remote Controller";
    return (
      <Tooltip label={`Connected to ${peerName}`} side="bottom">
        <button
          type="button"
          onClick={handleStartConfirm}
          className="inline-flex items-center justify-center w-8 h-8 rounded-xl border border-[rgba(var(--accent),0.5)] bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.2)] hover:bg-[rgba(var(--accent),0.2)] transition-all shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))] cursor-pointer animate-pulse-slow"
          aria-label={`Remote connected: ${peerName}. Click to disconnect.`}
        >
          <Rocket size={14} strokeWidth={1.75} />
        </button>
      </Tooltip>
    );
  }

  // Controller (Mobile): Always visible
  const controllerTooltip = isConnected
    ? `Connected to ${connectedPeer?.displayName ?? "Host"} (Tap to disconnect)`
    : isConnecting
    ? "Connecting to Host..."
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
              : "border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)]"
          )}
          aria-label={controllerTooltip}
        >
          <Rocket
            size={14}
            strokeWidth={1.75}
            className={cn(isConnecting && "animate-spin text-[rgb(var(--accent))]")}
          />
          {isConnected && (
            <span className="absolute top-1 right-1 w-1.5 h-1.5 rounded-full bg-[rgb(var(--success))] ring-1 ring-[rgb(var(--background))]" />
          )}
        </button>
      </Tooltip>

      <QrScannerModal
        open={isScannerOpen}
        onClose={() => setIsScannerOpen(false)}
      />
    </>
  );
});

RocketButton.displayName = "RocketButton";
