import { memo, useState, useEffect, useCallback } from "react";
import { useRemoteStore } from "@/store/remoteStore";
import { QrScannerModal } from "@/shared/components/remote/QrScannerModal";
import { ConfirmModal } from "@/shared/ui/ConfirmModal";
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

const HostGlyphButton = ({
  isConnected,
  onClick,
}: {
  isConnected: boolean;
  onClick: () => void;
}) => (
  <button
    type="button"
    aria-label="Paired host status"
    onClick={onClick}
    className="group flex flex-col items-center shrink-0 select-none cursor-pointer rounded-2xl px-3 py-1.5 transition-all duration-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))]"
  >
    <div className="relative w-[44px] h-[44px] transition-transform duration-300 group-hover:scale-105">
      <svg
        viewBox="0 0 44 44"
        className="w-full h-full overflow-visible select-none"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        {/* Host Desktop / Monitor screen frame */}
        <rect
          x="7"
          y="8"
          width="30"
          height="20"
          rx="3"
          stroke="rgb(var(--accent))"
          strokeWidth="1.5"
          className="transition-colors duration-300"
        />
        {/* Screen stand */}
        <path
          d="M 18 28 L 18 33 M 26 28 L 26 33 M 14 33 L 30 33"
          stroke="rgb(var(--accent))"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="transition-colors duration-300"
        />
        {/* Inner signal icon / screen content */}
        {isConnected ? (
          <>
            <circle cx="22" cy="18" r="2.5" fill="rgb(var(--accent))" className="animate-pulse" />
            <path
              d="M 16 15 A 7 7 0 0 1 28 15"
              stroke="rgb(var(--accent))"
              strokeWidth="1.25"
              strokeLinecap="round"
              className="opacity-80"
            />
            <path
              d="M 13 12 A 11 11 0 0 1 31 12"
              stroke="rgb(var(--accent))"
              strokeWidth="1.25"
              strokeLinecap="round"
              className="opacity-50"
            />
          </>
        ) : (
          <circle cx="22" cy="18" r="2.5" fill="rgb(var(--accent))" />
        )}
      </svg>
    </div>

    <span className="mt-1 text-[10px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))] transition-colors">
      {isConnected ? "CONNECTED" : "PAIRED"}
    </span>
  </button>
);

export const ControllerRemoteDeskPanel = memo(() => {
  const linkState = useRemoteStore((s) => s.linkState);
  const pairedDevices = useRemoteStore((s) => s.pairedDevices);
  const listPairedDevices = useRemoteStore((s) => s.listPairedDevices);
  const revokeDevice = useRemoteStore((s) => s.revokeDevice);

  const [isScannerOpen, setIsScannerOpen] = useState(false);
  const [defaultManual, setDefaultManual] = useState(false);
  const [isConfirmRevokeOpen, setIsConfirmRevokeOpen] = useState(false);
  const [isRevokingPending, setIsRevokingPending] = useState(false);

  const pairedHost = pairedDevices.find((d) => d.role === "host") ?? pairedDevices[0] ?? null;
  const isConnected = linkState === "Connected";

  useEffect(() => {
    void listPairedDevices();
  }, [listPairedDevices]);

  const handleOpenScanner = useCallback((manual = false) => {
    setDefaultManual(manual);
    setIsScannerOpen(true);
  }, []);

  const handleConfirmRevoke = useCallback(async () => {
    if (!pairedHost || isRevokingPending) return;
    setIsRevokingPending(true);
    try {
      await revokeDevice(pairedHost.deviceId);
      setIsConfirmRevokeOpen(false);
    } catch (e) {
      console.warn("[ControllerRemoteDeskPanel] Failed to revoke host:", e);
    } finally {
      setIsRevokingPending(false);
    }
  }, [pairedHost, isRevokingPending, revokeDevice]);

  const isPairedOrConnected = (linkState === "Paired" || linkState === "Connected") && pairedHost !== null;

  return (
    <div className="flex flex-row items-center justify-between gap-4 h-full p-2 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
      {/* Left Column: Host Identity or Pairing Prompt */}
      <div className="flex flex-col gap-1 min-w-0 flex-1">
        <div className="flex items-center justify-between gap-2">
          <span className="text-[12px] font-bold tracking-wider text-[rgb(var(--foreground))]">
            {REMOTE_CONTROL_COPY.title}
          </span>
        </div>

        <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
          {isConnected && pairedHost ? (
            <>
              {REMOTE_CONTROL_COPY.connectedTo} {pairedHost.displayName}
              {REMOTE_CONTROL_COPY.wirelessAudioActive}{" "}
              <button
                type="button"
                onClick={() => setIsConfirmRevokeOpen(true)}
                className="inline-flex items-center gap-1 text-[11px] font-mono font-bold text-[rgb(var(--danger))] hover:underline cursor-pointer transition-colors select-none"
                aria-label={REMOTE_CONTROL_COPY.revokeHostAria(pairedHost.displayName)}
              >
                {REMOTE_CONTROL_COPY.revokeHost}
              </button>{" "}
              <span className="text-[11px] font-medium text-[rgb(var(--foreground-muted))]">
                ({pairedHost.displayName})
              </span>
            </>
          ) : pairedHost ? (
            <>
              Paired with {pairedHost.displayName}. Idle on local Wi-Fi.{" "}
              <button
                type="button"
                onClick={() => setIsConfirmRevokeOpen(true)}
                className="inline-flex items-center gap-1 text-[11px] font-mono font-bold text-[rgb(var(--danger))] hover:underline cursor-pointer transition-colors select-none"
                aria-label={REMOTE_CONTROL_COPY.revokeHostAria(pairedHost.displayName)}
              >
                {REMOTE_CONTROL_COPY.revokeHost}
              </button>{" "}
              <span className="text-[11px] font-medium text-[rgb(var(--foreground-muted))]">
                ({pairedHost.displayName})
              </span>
            </>
          ) : (
            <>Pair with desktop host over local Wi-Fi to use Vox remotely.</>
          )}
        </p>
      </div>

      {/* Right Column: Scan Action or Connected Status with Revoke Trigger */}
      <div className="shrink-0 w-[116px] flex flex-col items-center justify-center">
        {isPairedOrConnected ? (
          <HostGlyphButton
            isConnected={isConnected}
            onClick={() => setIsConfirmRevokeOpen(true)}
          />
        ) : (
          <ScannerGlyphButton onClick={() => handleOpenScanner(false)} />
        )}
      </div>

      {/* QR Scanner / Manual IP Modal */}
      {isScannerOpen && (
        <QrScannerModal
          open={isScannerOpen}
          onClose={() => setIsScannerOpen(false)}
          defaultManual={defaultManual}
        />
      )}

      {/* Dedicated Two-Step Revoke Confirmation Modal (Image 2) */}
      {pairedHost && (
        <ConfirmModal
          open={isConfirmRevokeOpen}
          title="Revoke Paired Host"
          description={`Are you sure you want to revoke "${pairedHost.displayName}"? This will permanently sever the trusted pairing between your phone and your desktop computer.`}
          confirmLabel="Revoke"
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

ControllerRemoteDeskPanel.displayName = "ControllerRemoteDeskPanel";
