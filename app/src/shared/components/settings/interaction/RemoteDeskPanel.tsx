import { memo, useState, useCallback, useEffect, useRef } from "react";
import { Check, Trash2, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore } from "@/store/settingsStore";
import { useRemoteStore } from "@/store/remoteStore";
import { RemoteControlGlyph } from "./RemoteControlGlyph";
import { PairingModal } from "@/shared/components/remote/PairingModal";
import { Tooltip } from "@/shared/ui/Tooltip";
import { REMOTE_CONTROL_COPY } from "@/data/settingsCopy";

export const RemoteDeskPanel = memo(() => {
  const acceptsControllers = useSettingsStore((s) => s.draftSettings?.remote?.accepts_controllers ?? false);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const commitChanges = useSettingsStore((s) => s.commitChanges);

  const pairedDevices = useRemoteStore((s) => s.pairedDevices);
  const listPairedDevices = useRemoteStore((s) => s.listPairedDevices);
  const revokeDevice = useRemoteStore((s) => s.revokeDevice);
  const pendingPairingPrompt = useRemoteStore((s) => s.pendingPairingPrompt);
  const dismissPairingPrompt = useRemoteStore((s) => s.dismissPairingPrompt);

  const [isPairingOpen, setIsPairingOpen] = useState(false);
  const [isRevoking, setIsRevoking] = useState(false);
  const revokeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pairedDevice = pairedDevices[0] ?? null;

  const clearRevokeTimer = useCallback(() => {
    if (revokeTimerRef.current !== null) {
      clearTimeout(revokeTimerRef.current);
      revokeTimerRef.current = null;
    }
  }, []);

  useEffect(() => {
    void listPairedDevices();
    return () => clearRevokeTimer();
  }, [listPairedDevices, clearRevokeTimer]);

  const handleToggle = useCallback(() => {
    const next = !acceptsControllers;
    updateDraft("remote", "accepts_controllers", next);
    void commitChanges({ domain: "remote", key: "accepts_controllers" });
  }, [acceptsControllers, updateDraft, commitChanges]);

  const handleStartRevoke = useCallback(() => {
    setIsRevoking(true);
    clearRevokeTimer();
    revokeTimerRef.current = setTimeout(() => {
      setIsRevoking(false);
    }, 5000);
  }, [clearRevokeTimer]);

  const handleCancelRevoke = useCallback(() => {
    clearRevokeTimer();
    setIsRevoking(false);
  }, [clearRevokeTimer]);

  const handleConfirmRevoke = useCallback(
    async (deviceId: string) => {
      clearRevokeTimer();
      setIsRevoking(false);
      try {
        await revokeDevice(deviceId);
      } catch (e) {
        console.warn("[RemoteDeskPanel] Failed to revoke device:", e);
      }
    },
    [revokeDevice, clearRevokeTimer]
  );

  const handleClosePairing = useCallback(() => {
    setIsPairingOpen(false);
    dismissPairingPrompt();
  }, [dismissPairingPrompt]);

  return (
    <div className="flex flex-row items-center justify-between gap-4 h-full p-2 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
      {/* Left Column: Title & Description */}
      <div className="flex flex-col gap-1 min-w-0 flex-1">
        <div className="flex items-center justify-between gap-2">
          <span className="text-[12px] font-bold tracking-wider text-[rgb(var(--foreground))]">
            {REMOTE_CONTROL_COPY.title}
          </span>
        </div>

        <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium line-clamp-2">
          {!acceptsControllers ? (
            REMOTE_CONTROL_COPY.description
          ) : pairedDevice ? (
            <>
              {REMOTE_CONTROL_COPY.connectedTo}{" "}
              <span className="group inline-flex items-center align-baseline">
                <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                  {pairedDevice.displayName}
                </span>
                <span
                  className={cn(
                    "inline-flex items-center transition-all duration-200 overflow-hidden align-middle",
                    isRevoking
                      ? "max-w-[140px] opacity-100 ml-1.5"
                      : "max-w-0 opacity-0 group-hover:max-w-[40px] group-hover:opacity-100 group-hover:ml-1"
                  )}
                >
                  {isRevoking ? (
                    <span className="inline-flex items-center gap-1 bg-[rgba(var(--danger),0.12)] border border-[rgba(var(--danger),0.25)] rounded-md px-1.5 py-0.5 text-[10px] font-semibold text-[rgb(var(--danger))] animate-fade-in">
                      <span>{REMOTE_CONTROL_COPY.revokeConfirmPrompt}</span>
                      <button
                        type="button"
                        onClick={() => void handleConfirmRevoke(pairedDevice.deviceId)}
                        className="hover:scale-110 text-[rgb(var(--danger))] cursor-pointer transition-transform"
                        aria-label={REMOTE_CONTROL_COPY.revokeConfirmAria}
                      >
                        <Check size={11} strokeWidth={2.5} />
                      </button>
                      <button
                        type="button"
                        onClick={handleCancelRevoke}
                        className="hover:scale-110 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] cursor-pointer transition-transform"
                        aria-label={REMOTE_CONTROL_COPY.revokeCancelAria}
                      >
                        <X size={11} strokeWidth={2} />
                      </button>
                    </span>
                  ) : (
                    <Tooltip label={REMOTE_CONTROL_COPY.revokeDevice} side="top">
                      <button
                        type="button"
                        onClick={handleStartRevoke}
                        className="p-0.5 rounded text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.12)] cursor-pointer transition-colors inline-flex items-center"
                        aria-label={REMOTE_CONTROL_COPY.revokeDeviceAria(pairedDevice.displayName)}
                      >
                        <Trash2 size={12} />
                      </button>
                    </Tooltip>
                  )}
                </span>
              </span>
              {REMOTE_CONTROL_COPY.wirelessAudioActive}
            </>
          ) : (
            <>
              {REMOTE_CONTROL_COPY.descriptionReady}{" "}
              <button
                type="button"
                onClick={() => setIsPairingOpen(true)}
                className="inline-flex items-center gap-1 text-[11px] font-mono font-bold text-[rgb(var(--accent))] hover:text-[rgb(var(--foreground))] hover:underline cursor-pointer transition-colors select-none"
              >
                {REMOTE_CONTROL_COPY.pairNewDevice}
              </button>
            </>
          )}
        </p>
      </div>

      {/* Right Column: Interactive Glyph Switch */}
      <div className="shrink-0 w-[116px] flex flex-col items-center justify-center">
        <RemoteControlGlyph
          enabled={acceptsControllers}
          onToggle={handleToggle}
          label={acceptsControllers ? "ENABLED" : "DISABLED"}
          ariaLabel={REMOTE_CONTROL_COPY.title}
        />
      </div>

      {(isPairingOpen || Boolean(pendingPairingPrompt)) && (
        <PairingModal
          open={true}
          onClose={handleClosePairing}
        />
      )}
    </div>
  );
});

RemoteDeskPanel.displayName = "RemoteDeskPanel";
