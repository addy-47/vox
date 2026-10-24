import { memo, useState, useCallback, useEffect } from "react";
import { Trash2 } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
import { useRemoteStore } from "@/store/remoteStore";
import { RemoteControlGlyph } from "./RemoteControlGlyph";
import { PairingModal } from "@/shared/components/remote/PairingModal";
import { Tooltip } from "@/shared/ui/Tooltip";
import { REMOTE_CONTROL_COPY } from "@/data/settingsCopy";

export const RemoteDeskPanel = memo(() => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const commitChanges = useSettingsStore((s) => s.commitChanges);

  const pairedDevices = useRemoteStore((s) => s.pairedDevices);
  const listPairedDevices = useRemoteStore((s) => s.listPairedDevices);
  const revokeDevice = useRemoteStore((s) => s.revokeDevice);
  const pendingPairingPrompt = useRemoteStore((s) => s.pendingPairingPrompt);
  const dismissPairingPrompt = useRemoteStore((s) => s.dismissPairingPrompt);

  const [isPairingOpen, setIsPairingOpen] = useState(false);

  const acceptsControllers = draftSettings?.remote?.accepts_controllers ?? false;
  const pairedDevice = pairedDevices[0] ?? null;

  useEffect(() => {
    void listPairedDevices();
  }, [listPairedDevices]);

  const handleToggle = useCallback(() => {
    const next = !acceptsControllers;
    updateDraft("remote", "accepts_controllers", next);
    void commitChanges({ domain: "remote", key: "accepts_controllers" });
  }, [acceptsControllers, updateDraft, commitChanges]);

  const handleRevoke = useCallback(
    async (deviceId: string) => {
      try {
        await revokeDevice(deviceId);
      } catch (e) {
        console.warn("[RemoteDeskPanel] Failed to revoke device:", e);
      }
    },
    [revokeDevice]
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
              Connected to{" "}
              <span className="group inline-flex items-center gap-1 align-baseline">
                <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                  {pairedDevice.displayName}
                </span>
                <Tooltip label="Revoke device" side="top">
                  <button
                    type="button"
                    onClick={() => void handleRevoke(pairedDevice.deviceId)}
                    className="opacity-0 group-hover:opacity-100 p-0.5 rounded text-[rgb(var(--danger))] hover:opacity-100 transition-opacity cursor-pointer inline-flex items-center"
                    aria-label={`Revoke ${pairedDevice.displayName}`}
                  >
                    <Trash2 size={12} />
                  </button>
                </Tooltip>
              </span>
              . Wireless audio and session control are active.
            </>
          ) : (
            <>
              {REMOTE_CONTROL_COPY.descriptionReady}{" "}
              <button
                type="button"
                onClick={() => setIsPairingOpen(true)}
                className="inline-flex items-center gap-1 text-[11px] font-mono font-bold text-[rgb(var(--accent))] hover:text-[rgb(var(--foreground))] hover:underline cursor-pointer transition-colors select-none"
              >
                Pair new device +
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

      <PairingModal
        open={isPairingOpen || Boolean(pendingPairingPrompt)}
        onClose={handleClosePairing}
      />
    </div>
  );
});

RemoteDeskPanel.displayName = "RemoteDeskPanel";
