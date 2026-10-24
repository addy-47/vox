import { memo, useState, useCallback, useEffect } from "react";
import { Plus, Trash2 } from "lucide-react";
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

  return (
    <div className="flex flex-row items-center justify-between gap-4 h-full p-2 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
      {/* Left Column: Title & Description */}
      <div className="flex flex-col gap-1 min-w-0 flex-1">
        <div className="flex items-center justify-between gap-2">
          <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
            {REMOTE_CONTROL_COPY.title}
          </span>
        </div>

        <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium line-clamp-2">
          {!acceptsControllers
            ? REMOTE_CONTROL_COPY.description
            : pairedDevice
            ? `Paired with ${pairedDevice.displayName} over Wi-Fi.`
            : "Accepting connection from a mobile controller over Wi-Fi."}
        </p>
      </div>

      {/* Right Column: Interactive Glyph Switch & Pairing Action */}
      <div className="shrink-0 w-[116px] flex flex-col items-center justify-center">
        <RemoteControlGlyph
          enabled={acceptsControllers}
          onToggle={handleToggle}
          label={!acceptsControllers ? "OFF" : undefined}
          ariaLabel={REMOTE_CONTROL_COPY.title}
        />
        {acceptsControllers && (
          pairedDevice ? (
            <div className="mt-1 flex items-center justify-center gap-1.5 text-[10.5px] font-mono text-[rgb(var(--foreground-muted))] select-none">
              <span className="truncate max-w-[80px] text-[rgb(var(--foreground))]/90 font-medium" title={pairedDevice.displayName}>
                {pairedDevice.displayName}
              </span>
              <Tooltip label="Revoke device" side="top">
                <button
                  type="button"
                  onClick={() => void handleRevoke(pairedDevice.deviceId)}
                  className="p-0.5 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--danger))] transition-colors cursor-pointer"
                  aria-label={`Revoke ${pairedDevice.displayName}`}
                >
                  <Trash2 size={12} />
                </button>
              </Tooltip>
            </div>
          ) : (
            <button
              type="button"
              onClick={() => setIsPairingOpen(true)}
              className="mt-0.5 inline-flex items-center justify-center gap-1 text-[11px] font-mono font-bold text-[rgb(var(--accent))] hover:text-[rgb(var(--foreground))] hover:underline cursor-pointer transition-colors select-none shrink-0"
            >
              <Plus size={11} strokeWidth={2.5} />
              <span>Pair Device</span>
            </button>
          )
        )}
      </div>

      <PairingModal
        open={isPairingOpen}
        onClose={() => setIsPairingOpen(false)}
      />
    </div>
  );
});

RemoteDeskPanel.displayName = "RemoteDeskPanel";
