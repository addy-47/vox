import { memo, useState, useEffect } from "react";
import { Laptop, Wifi, Scan, PowerOff } from "lucide-react";
import { useRemoteStore } from "@/store/remoteStore";
import { QrScannerModal } from "@/shared/components/remote/QrScannerModal";

const ScannerGlyphButton = ({ onClick }: { onClick: () => void }) => (
  <button
    type="button"
    onClick={onClick}
    className="group relative flex flex-col items-center justify-center p-2 rounded-2xl bg-[rgba(var(--accent),0.06)] hover:bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.2)] hover:border-[rgba(var(--accent),0.4)] transition-all cursor-pointer shadow-sm active:scale-95"
    title="Open QR Scanner"
    aria-label="Scan QR Code to Pair"
  >
    <div className="relative w-14 h-14 flex items-center justify-center">
      {/* Corner reticle brackets */}
      <div className="absolute top-1 left-1 w-3 h-3 border-t-2 border-l-2 border-[rgb(var(--accent))] rounded-tl-sm transition-transform group-hover:-translate-x-0.5 group-hover:-translate-y-0.5" />
      <div className="absolute top-1 right-1 w-3 h-3 border-t-2 border-r-2 border-[rgb(var(--accent))] rounded-tr-sm transition-transform group-hover:translate-x-0.5 group-hover:-translate-y-0.5" />
      <div className="absolute bottom-1 left-1 w-3 h-3 border-b-2 border-l-2 border-[rgb(var(--accent))] rounded-bl-sm transition-transform group-hover:-translate-x-0.5 group-hover:translate-y-0.5" />
      <div className="absolute bottom-1 right-1 w-3 h-3 border-b-2 border-r-2 border-[rgb(var(--accent))] rounded-br-sm transition-transform group-hover:translate-x-0.5 group-hover:translate-y-0.5" />

      {/* Center QR/Scan icon with subtle pulsing beacon */}
      <Scan size={24} className="text-[rgb(var(--accent))] transition-transform group-hover:scale-110" />

      {/* Laser line inside button */}
      <div className="absolute inset-x-2 top-1/2 h-[1.5px] bg-[rgb(var(--accent))] opacity-75 shadow-[0_0_6px_rgba(var(--accent),0.9)] animate-pulse pointer-events-none" />
    </div>

    <span className="text-[10px] font-mono font-bold tracking-wider text-[rgb(var(--accent))] uppercase mt-1">
      Scan QR
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
    if (typeof navigator !== "undefined" && navigator.permissions?.query) {
      navigator.permissions
        .query({ name: "camera" as PermissionName })
        .then((status) => {
          setHasCameraPermission(status.state !== "denied");
          status.onchange = () => {
            setHasCameraPermission(status.state !== "denied");
          };
        })
        .catch(() => {
          setHasCameraPermission(true);
        });
    } else {
      setHasCameraPermission(true);
    }
  }, []);

  const handleOpenScanner = (manual = false) => {
    setDefaultManual(manual);
    setIsScannerOpen(true);
  };

  return (
    <div className="flex flex-row items-center justify-between gap-4 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
      {isConnected ? (
        /* Connected Host Dashboard */
        <div className="flex flex-col gap-2.5 w-full">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                Connected Host
              </span>
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-mono font-bold text-emerald-400 bg-emerald-500/10 border border-emerald-500/20">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                ACTIVE
              </span>
            </div>

            <button
              type="button"
              onClick={() => void disconnectHost()}
              className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs font-semibold text-[rgb(var(--danger))] bg-[rgba(var(--danger),0.08)] hover:bg-[rgba(var(--danger),0.16)] border border-[rgba(var(--danger),0.2)] transition-colors cursor-pointer"
              title="Disconnect from host"
            >
              <PowerOff size={12} />
              Disconnect
            </button>
          </div>

          <div className="flex items-center gap-3 p-3 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.08)]">
            <div className="w-10 h-10 rounded-xl bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
              <Laptop size={18} />
            </div>
            <div className="flex flex-col min-w-0 flex-1">
              <span className="text-xs font-bold text-[rgb(var(--foreground))] truncate">
                {connectedPeer?.displayName ?? "Vox Desktop Host"}
              </span>
              <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70 truncate">
                {connectedPeer?.deviceId ? `ID: ${connectedPeer.deviceId.slice(0, 16)}...` : "Paired Controller"}
              </span>
            </div>
          </div>

          <p className="text-[10.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
            Controlling sessions and dictation wirelessly from this mobile device.
          </p>
        </div>
      ) : (
        /* Pre-Connection Setup */
        <>
          <div className="flex flex-col gap-1.5 min-w-0 flex-1">
            <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
              Pair with Host
            </span>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/80 leading-relaxed font-medium">
              Control Vox running on your desktop. Tap the scanner to link with your host screen.
            </p>

            {hasCameraPermission === false && (
              <button
                type="button"
                onClick={() => handleOpenScanner(true)}
                className="inline-flex items-center gap-1.5 text-[11px] font-mono text-[rgb(var(--accent))] hover:underline cursor-pointer pt-1"
              >
                <Wifi size={11} />
                Enter IP manually
              </button>
            )}
          </div>

          <div className="shrink-0 flex items-center justify-center">
            <ScannerGlyphButton onClick={() => handleOpenScanner(false)} />
          </div>
        </>
      )}

      <QrScannerModal
        open={isScannerOpen}
        onClose={() => setIsScannerOpen(false)}
        defaultManual={defaultManual}
      />
    </div>
  );
});

ControllerRemoteDeskPanel.displayName = "ControllerRemoteDeskPanel";
