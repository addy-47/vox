import { memo, useState, useEffect, useCallback } from "react";
import { Wifi, PowerOff } from "lucide-react";
import { useRemoteStore } from "@/store/remoteStore";
import { QrScannerModal } from "@/shared/components/remote/QrScannerModal";
import { Tooltip } from "@/shared/ui/Tooltip";
import { REMOTE_CONTROL_COPY } from "@/data/settingsCopy";

const ScannerGlyphButton = ({ onClick }: { onClick: () => void }) => (
  <button
    type="button"
    aria-label={REMOTE_CONTROL_COPY.scanQrPrompt}
    onClick={onClick}
    className="group flex flex-col items-center shrink-0 select-none cursor-pointer rounded-2xl px-3 py-2 transition-all duration-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] text-[rgb(var(--accent))]"
  >
    <div className="relative w-[48px] h-[48px] transition-transform duration-300 group-hover:scale-105">
      <svg
        viewBox="0 0 48 48"
        className="w-full h-full overflow-visible select-none"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        {/* Scan frame — four corner brackets */}
        <path d="M 6 14 L 6 6 L 14 6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="opacity-90" />
        <path d="M 34 6 L 42 6 L 42 14" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="opacity-90" />
        <path d="M 6 34 L 6 42 L 14 42" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="opacity-90" />
        <path d="M 34 42 L 42 42 L 42 34" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="opacity-90" />

        {/* QR module grid */}
        <rect x="14" y="14" width="6" height="6" rx="1" fill="currentColor" className="opacity-80" />
        <rect x="22" y="14" width="6" height="6" rx="1" fill="currentColor" className="opacity-45" />
        <rect x="30" y="14" width="6" height="6" rx="1" fill="currentColor" className="opacity-80" />
        <rect x="14" y="22" width="6" height="6" rx="1" fill="currentColor" className="opacity-45" />
        <rect x="30" y="22" width="6" height="6" rx="1" fill="currentColor" className="opacity-60" />
        <rect x="14" y="30" width="6" height="6" rx="1" fill="currentColor" className="opacity-70" />
        <rect x="22" y="30" width="6" height="6" rx="1" fill="currentColor" className="opacity-80" />
        <rect x="30" y="30" width="6" height="6" rx="1" fill="currentColor" className="opacity-45" />

        {/* Scan beam */}
        <line x1="8" y1="24" x2="40" y2="24" stroke="currentColor" strokeWidth="1" strokeLinecap="round" className="opacity-70 animate-pulse" />
      </svg>
    </div>

    <span className="mt-2 text-[10px] font-mono font-bold uppercase tracking-wider transition-colors text-[rgb(var(--accent))]">
      {REMOTE_CONTROL_COPY.scanQrButton}
    </span>
  </button>
);

export const ControllerRemoteDeskPanel = memo(() => {
  const linkState = useRemoteStore((s) => s.linkState);
  const connectedPeer = useRemoteStore((s) => s.connectedPeer);
  const disconnectHost = useRemoteStore((s) => s.disconnectHost);

  const [isScannerOpen, setIsScannerOpen] = useState(false);
  const [defaultManual, setDefaultManual] = useState(false);
  const [hasCameraPermission, setHasCameraPermission] = useState<boolean | null>(null);

  const isConnected = linkState === "Connected";

  useEffect(() => {
    let permStatus: PermissionStatus | null = null;
    if (typeof navigator !== "undefined" && navigator.permissions?.query) {
      navigator.permissions
        .query({ name: "camera" as PermissionName })
        .then((status) => {
          permStatus = status;
          setHasCameraPermission(status.state !== "denied");
          status.onchange = () => {
            setHasCameraPermission(status.state !== "denied");
          };
        })
        .catch(() => {
          setHasCameraPermission(true);
        });
    }
    return () => {
      if (permStatus) {
        permStatus.onchange = null;
      }
    };
  }, []);

  const handleOpenScanner = useCallback((manual = false) => {
    setDefaultManual(manual);
    setIsScannerOpen(true);
  }, []);

  const handleDisconnect = useCallback(async () => {
    try {
      await disconnectHost();
    } catch (e) {
      console.warn("[ControllerRemoteDeskPanel] Failed to disconnect host:", e);
    }
  }, [disconnectHost]);

  return (
    <div className="flex flex-row items-center justify-between gap-4 h-full p-2 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
      {/* Left Column: Title & Description */}
      <div className="flex flex-col gap-1 min-w-0 flex-1">
        <div className="flex items-center justify-between gap-2">
          <span className="text-[12px] font-bold tracking-wider text-[rgb(var(--foreground))]">
            {isConnected ? REMOTE_CONTROL_COPY.connectedHost : REMOTE_CONTROL_COPY.title}
          </span>
        </div>

        <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium line-clamp-2">
          {isConnected ? (
            <>
              {REMOTE_CONTROL_COPY.connectedTo}{" "}
              <span className="group inline-flex items-center gap-1 align-baseline">
                <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                  {connectedPeer?.displayName ?? "Vox Desktop Host"}
                </span>
                <Tooltip label={REMOTE_CONTROL_COPY.disconnect} side="top">
                  <button
                    type="button"
                    onClick={() => void handleDisconnect()}
                    className="opacity-0 group-hover:opacity-100 p-0.5 rounded text-[rgb(var(--danger))] hover:opacity-100 transition-opacity cursor-pointer inline-flex items-center"
                    aria-label={REMOTE_CONTROL_COPY.disconnectHostAria}
                  >
                    <PowerOff size={12} />
                  </button>
                </Tooltip>
              </span>
              {REMOTE_CONTROL_COPY.wirelessMicActive}
            </>
          ) : (
            <>
              {REMOTE_CONTROL_COPY.pairDeviceScreenPrompt}
              {hasCameraPermission === false && (
                <>
                  {" "}
                  <button
                    type="button"
                    onClick={() => handleOpenScanner(true)}
                    className="inline-flex items-center gap-1 text-[11px] font-mono font-bold text-[rgb(var(--accent))] hover:text-[rgb(var(--foreground))] hover:underline cursor-pointer transition-colors select-none"
                  >
                    {REMOTE_CONTROL_COPY.enterIpManually}
                  </button>
                </>
              )}
            </>
          )}
        </p>
      </div>

      {/* Right Column: Scan Action / Connected Glyph */}
      <div className="shrink-0 w-[116px] flex flex-col items-center justify-center">
        {isConnected ? (
          <div className="flex flex-col items-center justify-center gap-1 select-none">
            <div className="w-10 h-10 rounded-xl bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.25)] flex items-center justify-center text-[rgb(var(--accent))]">
              <Wifi size={18} className="animate-pulse" />
            </div>
            <span className="text-[9.5px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))] mt-0.5">
              {REMOTE_CONTROL_COPY.linked}
            </span>
          </div>
        ) : (
          <ScannerGlyphButton onClick={() => handleOpenScanner(false)} />
        )}
      </div>

      {isScannerOpen && (
        <QrScannerModal
          open={isScannerOpen}
          onClose={() => setIsScannerOpen(false)}
          defaultManual={defaultManual}
        />
      )}
    </div>
  );
});

ControllerRemoteDeskPanel.displayName = "ControllerRemoteDeskPanel";
