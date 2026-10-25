import { memo, useState, useCallback, useEffect } from "react";
import { Trash2 } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
import { useRemoteStore } from "@/store/remoteStore";
import { RemoteControlGlyph } from "./RemoteControlGlyph";
import { ConfirmModal } from "@/shared/ui/ConfirmModal";
import { Tooltip } from "@/shared/ui/Tooltip";
import { REMOTE_CONTROL_COPY } from "@/data/settingsCopy";

export const RemoteDeskPanel = memo(() => {
  const acceptsControllers = useSettingsStore(
    (s) => s.draftSettings?.remote?.accepts_controllers ?? false
  );
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const commitChanges = useSettingsStore((s) => s.commitChanges);

  const pairedDevices = useRemoteStore((s) => s.pairedDevices);
  const listPairedDevices = useRemoteStore((s) => s.listPairedDevices);
  const revokeDevice = useRemoteStore((s) => s.revokeDevice);
  const openPairingModal = useRemoteStore((s) => s.openPairingModal);

  const [isConfirmRevokeOpen, setIsConfirmRevokeOpen] = useState(false);
  const [isRevokingPending, setIsRevokingPending] = useState(false);
  const pairedDevice = pairedDevices[0] ?? null;

  useEffect(() => {
    void listPairedDevices();
  }, [listPairedDevices]);

  const handleToggle = useCallback(() => {
    const next = !acceptsControllers;
    updateDraft("remote", "accepts_controllers", next);
    void commitChanges({ domain: "remote", key: "accepts_controllers" });
  }, [acceptsControllers, updateDraft, commitChanges]);

  const handleConfirmRevoke = useCallback(async () => {
    if (!pairedDevice || isRevokingPending) return;
    setIsRevokingPending(true);
    try {
      await revokeDevice(pairedDevice.deviceId);
      setIsConfirmRevokeOpen(false);
    } catch (e) {
      console.warn("[RemoteDeskPanel] Failed to revoke device:", e);
    } finally {
      setIsRevokingPending(false);
    }
  }, [pairedDevice, isRevokingPending, revokeDevice]);

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
              <span className="group inline-flex items-center gap-1.5 align-baseline">
                <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                  {pairedDevice.displayName}
                </span>
                <Tooltip label={REMOTE_CONTROL_COPY.revokeDevice} side="top">
                  <button
                    type="button"
                    onClick={() => setIsConfirmRevokeOpen(true)}
                    className="p-1 rounded-md text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.12)] active:scale-95 cursor-pointer transition-all inline-flex items-center opacity-80 group-hover:opacity-100"
                    aria-label={REMOTE_CONTROL_COPY.revokeDeviceAria(pairedDevice.displayName)}
                  >
                    <Trash2 size={12} />
                  </button>
                </Tooltip>
              </span>
              {REMOTE_CONTROL_COPY.wirelessAudioActive}
            </>
          ) : (
            <>
              {REMOTE_CONTROL_COPY.descriptionReady}{" "}
              <button
                type="button"
                onClick={openPairingModal}
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

      {/* Two-step Revoke Confirmation Modal (Image 2) */}
      {pairedDevice && (
        <ConfirmModal
          open={isConfirmRevokeOpen}
          title="Revoke Companion Device"
          description={`Are you sure you want to revoke "${pairedDevice.displayName}"? This will permanently sever the trusted pairing between your laptop and phone.`}
          confirmLabel="Revoke Device"
          cancelLabel="Cancel"
          isDestructive={true}
          isLoading={isRevokingPending}
          onConfirm={handleConfirmRevoke}
          onCancel={() => setIsConfirmRevokeOpen(false)}
        />
      )}
    </div>
  );
});

RemoteDeskPanel.displayName = "RemoteDeskPanel";
