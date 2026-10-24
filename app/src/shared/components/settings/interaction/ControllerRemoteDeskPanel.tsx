import { memo, useState } from "react";
import { QrCode, Laptop, Wifi } from "lucide-react";
import { useRemoteStore } from "@/store/remoteStore";
import { RemoteControlGlyph } from "./RemoteControlGlyph";
import { QrScannerModal } from "@/shared/components/remote/QrScannerModal";
import { REMOTE_CONTROL_COPY } from "@/data/settingsCopy";

export const ControllerRemoteDeskPanel = memo(() => {
  const linkState = useRemoteStore((s) => s.linkState);
  const connectedPeer = useRemoteStore((s) => s.connectedPeer);

  const [isScannerOpen, setIsScannerOpen] = useState(false);
  const [defaultManual, setDefaultManual] = useState(false);

  const isConnected = linkState === "Connected";

  const handleOpenScanner = (manual = false) => {
    setDefaultManual(manual);
    setIsScannerOpen(true);
  };

  const glyphLabel = isConnected
    ? "CONNECTED"
    : linkState === "Connecting"
    ? "LINKING"
    : "IDLE";

  return (
    <div className="flex flex-row items-center justify-between gap-4 h-full p-2 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
      {/* Left Column */}
      <div className="flex flex-col gap-2 min-w-0 flex-1">
        <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
          {REMOTE_CONTROL_COPY.title}
        </span>

        {isConnected ? (
          /* Connected State */
          <div className="flex flex-col gap-1.5 mt-0.5">
            <div className="flex items-center gap-2 px-3 py-2 rounded-xl bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--accent),0.2)]">
              <Laptop size={16} className="text-[rgb(var(--accent))] shrink-0" />
              <div className="flex flex-col min-w-0 flex-1">
                <div className="flex items-center gap-1.5">
                  <span className="text-[12px] font-bold text-[rgb(var(--foreground))] truncate">
                    {connectedPeer?.displayName ?? "Vox Host"}
                  </span>
                  <span className="inline-block w-1.5 h-1.5 rounded-full bg-[rgb(var(--success))]" />
                </div>
                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70 truncate">
                  Session active
                </span>
              </div>
            </div>
            <p className="text-[10.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
              Controlling sessions and dictation from this mobile device.
            </p>
          </div>
        ) : (
          /* Disconnected / Empty State */
          <div className="flex flex-col gap-2">
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium line-clamp-2">
              Connect this phone to your laptop to use it as a wireless controller and HUD.
            </p>

            <div className="flex flex-wrap items-center gap-2 pt-0.5">
              <button
                type="button"
                onClick={() => handleOpenScanner(false)}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[rgb(var(--accent))] text-[rgb(var(--background))] text-xs font-mono font-bold hover:opacity-90 transition-opacity cursor-pointer shadow-sm"
              >
                <QrCode size={13} strokeWidth={2.5} />
                Scan Host QR
              </button>

              <button
                type="button"
                onClick={() => handleOpenScanner(true)}
                className="inline-flex items-center gap-1 text-[11px] font-mono text-[rgb(var(--accent))] hover:underline cursor-pointer"
              >
                <Wifi size={11} />
                Enter IP manually
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Right Column: Visual Glyph */}
      <div className="shrink-0 w-[116px] flex flex-col items-center justify-center">
        <RemoteControlGlyph
          enabled={isConnected}
          label={glyphLabel}
          ariaLabel={REMOTE_CONTROL_COPY.title}
        />
      </div>

      <QrScannerModal
        open={isScannerOpen}
        onClose={() => setIsScannerOpen(false)}
        defaultManual={defaultManual}
      />
    </div>
  );
});

ControllerRemoteDeskPanel.displayName = "ControllerRemoteDeskPanel";
