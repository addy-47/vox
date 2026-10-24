import { memo, useState, useEffect, useCallback, useRef } from "react";
import {
  RotateCcw,
  AlertCircle,
  Clock,
  Check,
  X,
  Wifi,
  Radio,
  Download,
  ScanLine,
  Smartphone,
  Link2,
} from "lucide-react";
import { Modal } from "@/shared/ui/Modal";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useRemoteStore } from "@/store/remoteStore";
import { OrbitalLoader } from "@/shared/components/common";
import { cn } from "@/shared/lib/utils";

export interface PairingModalProps {
  open: boolean;
  onClose: () => void;
}

interface WirePairingPayload {
  hint_addr?: string;
  hint_port?: number;
  device_id?: string;
  public_key?: string;
  token?: string;
}

interface QrDetails {
  qrDataUrl: string;
  expiresAt: number;
  hintAddr: string;
  hintPort: number;
}

const AndroidIcon = ({
  size = 20,
  className = "",
}: {
  size?: number;
  className?: string;
}) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="currentColor"
    className={className}
    aria-hidden="true"
  >
    <path d="M6 18c0 .55.45 1 1 1h1v3.5c0 .83.67 1.5 1.5 1.5s1.5-.67 1.5-1.5V19h2v3.5c0 .83.67 1.5 1.5 1.5s1.5-.67 1.5-1.5V19h1c.55 0 1-.45 1-1V8H6v10zM3.5 8C2.67 8 2 8.67 2 9.5v6c0 .83.67 1.5 1.5 1.5S5 16.33 5 15.5v-6C5 8.67 4.33 8 3.5 8zm17 0c-.83 0-1.5.67-1.5 1.5v6c0 .83.67 1.5 1.5 1.5s1.5-.67 1.5-1.5v-6c0-.83-.67-1.5-1.5-1.5zm-4.97-4.84l1.3-1.3c.2-.2.2-.51 0-.71-.2-.2-.51-.2-.71 0l-1.48 1.48C13.68 2.24 12.87 2 12 2s-1.68.24-2.64.63L7.88 1.15c-.2-.2-.51-.2-.71 0-.2.2-.2.51 0 .71l1.3 1.3C6.73 4.2 5.5 6.02 5.5 8h13c0-1.98-1.23-3.8-2.97-4.84zM9 6.5c-.55 0-1-.45-1-1s.45-1 1-1 1 .45 1 1-.45 1-1 1zm6 0c-.55 0-1-.45-1-1s.45-1 1-1 1 .45 1 1-.45 1-1 1z" />
  </svg>
);

const PAIRING_PHASES = [
  { id: "installed", step: "01", label: "App ready", Icon: Smartphone },
  { id: "scanning", step: "02", label: "Awaiting scan", Icon: ScanLine },
  { id: "connecting", step: "03", label: "Handshake", Icon: Link2 },
  { id: "paired", step: "04", label: "Paired", Icon: Check },
] as const;

export const PairingModal = memo(({ open, onClose }: PairingModalProps) => {
  const pairNewDevice = useRemoteStore((s) => s.pairNewDevice);
  const linkState = useRemoteStore((s) => s.linkState);
  const pendingPairingPrompt = useRemoteStore((s) => s.pendingPairingPrompt);
  const approvePairing = useRemoteStore((s) => s.approvePairing);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [details, setDetails] = useState<QrDetails | null>(null);
  const [timeLeft, setTimeLeft] = useState<number>(120);
  const [approving, setApproving] = useState<"Approve" | "Reject" | null>(null);

  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const generatePairing = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      clearTimer();

      const res = await pairNewDevice();
      let hintAddr = "127.0.0.1";
      let hintPort = 7700;

      try {
        const parsed = JSON.parse(res.payload) as WirePairingPayload;
        if (typeof parsed.hint_addr === "string") hintAddr = parsed.hint_addr;
        if (typeof parsed.hint_port === "number") hintPort = parsed.hint_port;
      } catch (e) {
        console.debug("[PairingModal] Failed to parse payload json:", e);
      }

      const remaining = Math.max(
        0,
        Math.floor((res.expiresAt - Date.now()) / 1000)
      );

      setDetails({
        qrDataUrl: res.qrDataUrl,
        expiresAt: res.expiresAt,
        hintAddr,
        hintPort,
      });
      setTimeLeft(remaining > 0 ? remaining : 120);

      timerRef.current = setInterval(() => {
        setTimeLeft((prev) => {
          if (prev <= 1) {
            clearTimer();
            return 0;
          }
          return prev - 1;
        });
      }, 1000);
    } catch (e) {
      console.warn("[PairingModal] pairNewDevice error:", e);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [pairNewDevice, clearTimer]);

  useEffect(() => {
    if (open) {
      void generatePairing();
    } else {
      clearTimer();
      setDetails(null);
      setError(null);
      setApproving(null);
    }
    return () => clearTimer();
  }, [open, generatePairing, clearTimer]);

  useEffect(() => {
    if (open && linkState === "Connected") {
      onClose();
    }
  }, [open, linkState, onClose]);

  const handleDecision = async (decision: "Approve" | "Reject") => {
    try {
      setApproving(decision);
      await approvePairing(decision);
    } catch (e) {
      console.warn("[PairingModal] approvePairing error:", e);
    } finally {
      setApproving(null);
    }
  };

  const handleDownloadApk = () => {
    window.open("https://github.com/addy-47/vox/releases", "_blank");
  };

  const minutes = Math.floor(timeLeft / 60);
  const seconds = timeLeft % 60;
  const timeFormatted = `${minutes}:${seconds.toString().padStart(2, "0")}`;

  // Live phase index:
  // 0: Token expired / idle
  // 1: QR ready & awaiting mobile camera scan
  // 2: Handshake / awaiting host approval
  // 3: Connected
  const currentPhaseIndex =
    linkState === "Connected"
      ? 3
      : pendingPairingPrompt ||
        linkState === "Connecting" ||
        linkState === "Reconnecting"
      ? 2
      : details && timeLeft > 0
      ? 1
      : 0;

  // Progress percentage across the 3 segments between the 4 nodes (0%, 33.33%, 66.66%, 100%)
  const progressPercent = (currentPhaseIndex / (PAIRING_PHASES.length - 1)) * 100;

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Pair Remote Controller"
      subtitle="Connect your mobile device to control Vox wirelessly over LAN."
      position="global"
      fitContent
      className="max-w-2xl w-[94vw] md:w-full overflow-hidden"
    >
      <div className="p-5 sm:p-6 space-y-5">
        {loading ? (
          /* Spec §7.1: OrbitalLoader MUST NOT render any accompanying text */
          <div className="h-72 flex items-center justify-center">
            <OrbitalLoader size="md" />
          </div>
        ) : error ? (
          <div className="flex flex-col items-center justify-center p-8 gap-3 text-center">
            <AlertCircle className="text-[rgb(var(--error))] w-8 h-8" />
            <p className="text-sm font-semibold text-[rgb(var(--foreground))]">
              Failed to generate pairing QR
            </p>
            <p className="text-xs text-[rgb(var(--foreground-muted))] font-mono max-w-sm break-words">
              {error}
            </p>
            <button
              type="button"
              onClick={generatePairing}
              className="mt-2 inline-flex items-center gap-1.5 px-4 py-2 rounded-xl bg-[rgba(var(--accent),0.12)] hover:bg-[rgba(var(--accent),0.2)] text-[rgb(var(--accent))] text-xs font-semibold cursor-pointer transition-colors"
            >
              <RotateCcw size={13} />
              Try Again
            </button>
          </div>
        ) : (
          <>
            {/* Top Grid: Left timeline matches exact height of Right QR Card */}
            <div className="grid grid-cols-1 md:grid-cols-12 gap-5 items-stretch">
              {/* Left Column: Full-height flex container aligned with QR card */}
              <div className="md:col-span-7 flex flex-col justify-between rounded-2xl bg-[rgba(var(--card),0.45)] border border-[rgba(var(--border),0.07)] p-4 sm:p-5">
                {pendingPairingPrompt && (
                  <div className="mb-4 rounded-xl p-3.5 bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.3)] space-y-2.5 animate-fade-in">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2 text-xs font-semibold text-[rgb(var(--accent))]">
                        <Radio size={14} className="animate-pulse" />
                        <span>Incoming Pairing Request</span>
                      </div>
                      <span className="text-[11px] font-mono text-[rgb(var(--accent))]">
                        Action required
                      </span>
                    </div>
                    <div className="text-xs">
                      <p className="font-bold text-[rgb(var(--foreground))]">
                        {pendingPairingPrompt.displayName}
                      </p>
                      <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] truncate mt-0.5">
                        ID: {pendingPairingPrompt.deviceId}
                      </p>
                    </div>
                    <div className="flex items-center gap-2 pt-1">
                      <button
                        type="button"
                        disabled={approving !== null}
                        onClick={() => void handleDecision("Approve")}
                        className="flex-1 inline-flex items-center justify-center gap-1.5 px-3 py-2 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-xs font-bold hover:opacity-90 disabled:opacity-50 transition-opacity cursor-pointer"
                      >
                        <Check size={13} strokeWidth={2.5} />
                        {approving === "Approve"
                          ? "Approving..."
                          : "Approve Device"}
                      </button>
                      <Tooltip label="Reject Request" side="top">
                        <button
                          type="button"
                          disabled={approving !== null}
                          onClick={() => void handleDecision("Reject")}
                          aria-label="Reject Pairing Request"
                          className="h-8 w-8 inline-flex items-center justify-center rounded-xl border border-[rgba(var(--border),0.15)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--border),0.3)] disabled:opacity-50 transition-colors cursor-pointer"
                        >
                          <X size={14} />
                        </button>
                      </Tooltip>
                    </div>
                  </div>
                )}

                {/* Vertical Timeline Stepper (stretches evenly across available height) */}
                <div className="relative flex-1 flex flex-col justify-between py-0.5">
                  {/* Continuous Vertical Timeline Spine bridging all 3 icons */}
                  <div
                    aria-hidden="true"
                    className="pointer-events-none absolute left-5 top-5 bottom-5 w-px -translate-x-1/2 bg-gradient-to-b from-[rgba(var(--accent),0.45)] via-[rgba(var(--border),0.15)] to-[rgba(var(--accent),0.35)]"
                  />

                  {/* Step 1: Install App */}
                  <div className="relative flex items-start gap-3.5">
                    <div className="relative z-10 w-10 h-10 rounded-xl bg-[rgb(var(--card))] border border-[rgba(var(--accent),0.25)] shadow-[0_0_12px_rgba(var(--accent),0.08)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                      <AndroidIcon size={19} />
                    </div>
                    <div className="flex-1 min-w-0 pt-0.5">
                      <div className="flex items-center gap-2">
                        <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                          01
                        </span>
                        <h4 className="text-xs sm:text-sm font-bold text-[rgb(var(--foreground))] leading-snug">
                          Install the Vox mobile app
                        </h4>
                      </div>
                      <p className="text-xs text-[rgb(var(--foreground-muted))] mt-1 leading-relaxed">
                        Download and install the companion Android APK on your
                        phone.
                      </p>
                    </div>
                    <Tooltip label="Download Android APK" side="top">
                      <button
                        type="button"
                        onClick={handleDownloadApk}
                        aria-label="Download Android APK"
                        className="relative z-10 w-9 h-9 rounded-xl bg-[rgba(var(--accent),0.1)] hover:bg-[rgba(var(--accent),0.18)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center text-[rgb(var(--accent))] transition-colors cursor-pointer shrink-0 mt-0.5"
                      >
                        <Download size={15} />
                      </button>
                    </Tooltip>
                  </div>

                  {/* Step 2: Open QR Scanner */}
                  <div className="relative flex items-start gap-3.5 my-4">
                    <div className="relative z-10 w-10 h-10 rounded-xl bg-[rgb(var(--card))] border border-[rgba(var(--accent),0.25)] shadow-[0_0_12px_rgba(var(--accent),0.08)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                      <ScanLine size={18} />
                    </div>
                    <div className="flex-1 min-w-0 pt-0.5">
                      <div className="flex items-center gap-2">
                        <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                          02
                        </span>
                        <h4 className="text-xs sm:text-sm font-bold text-[rgb(var(--foreground))] leading-snug">
                          Open the app and tap Scan to Pair
                        </h4>
                      </div>
                      <p className="text-xs text-[rgb(var(--foreground-muted))] mt-1 leading-relaxed">
                        Launch Vox on your phone and open the QR scanner from
                        the top bar.
                      </p>
                    </div>
                  </div>

                  {/* Step 3: Point & Confirm */}
                  <div className="relative flex items-start gap-3.5">
                    <div className="relative z-10 w-10 h-10 rounded-xl bg-[rgb(var(--card))] border border-[rgba(var(--accent),0.25)] shadow-[0_0_12px_rgba(var(--accent),0.08)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                      <Smartphone size={18} />
                    </div>
                    <div className="flex-1 min-w-0 pt-0.5">
                      <div className="flex items-center gap-2">
                        <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                          03
                        </span>
                        <h4 className="text-xs sm:text-sm font-bold text-[rgb(var(--foreground))] leading-snug">
                          Point your camera and confirm
                        </h4>
                      </div>
                      <p className="text-xs text-[rgb(var(--foreground-muted))] mt-1 leading-relaxed">
                        Scan the QR code on the right, then approve the pairing
                        prompt on this screen.
                      </p>
                    </div>
                  </div>
                </div>

                {/* Host Network Address Footer */}
                {details && (
                  <div className="mt-4 pt-3 border-t border-[rgba(var(--border),0.06)] flex items-center justify-between text-xs">
                    <div className="flex items-center gap-2 text-[rgb(var(--foreground-muted))]">
                      <Wifi
                        size={13}
                        className="text-[rgb(var(--accent))] shrink-0"
                      />
                      <span>LAN Host</span>
                    </div>
                    <span className="font-mono text-xs font-semibold text-[rgb(var(--foreground))]">
                      {details.hintAddr}:{details.hintPort}
                    </span>
                  </div>
                )}
              </div>

              {/* Right Column: QR Card Container with Countdown */}
              <div className="md:col-span-5 flex flex-col">
                <div className="h-full w-full rounded-2xl bg-[rgba(var(--card),0.8)] border border-[rgba(var(--border),0.08)] p-4 sm:p-5 flex flex-col items-center justify-between text-center shadow-sm">
                  {details && (
                    <div className="relative group my-auto">
                      <div
                        className={cn(
                          "p-3 rounded-2xl bg-white shadow-md transition-all duration-300",
                          timeLeft === 0
                            ? "opacity-20 grayscale scale-95"
                            : "opacity-100 scale-100"
                        )}
                      >
                        <img
                          src={details.qrDataUrl}
                          alt="Remote Pairing QR Code"
                          className="w-44 h-44 sm:w-48 sm:h-48 block select-none pointer-events-none"
                        />
                      </div>

                      {timeLeft === 0 && (
                        <div className="absolute inset-0 flex flex-col items-center justify-center gap-2.5 bg-[rgba(var(--background),0.92)] rounded-2xl p-4 text-center">
                          <span className="text-xs font-bold text-[rgb(var(--foreground))]">
                            QR Code Expired
                          </span>
                          <button
                            type="button"
                            onClick={generatePairing}
                            className="inline-flex items-center gap-1.5 px-3.5 py-2 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-xs font-bold cursor-pointer hover:opacity-90 transition-opacity"
                          >
                            <RotateCcw size={13} />
                            Regenerate
                          </button>
                        </div>
                      )}
                    </div>
                  )}

                  {/* Countdown & Regenerate Action */}
                  <div className="w-full pt-3 mt-3 border-t border-[rgba(var(--border),0.06)] flex flex-col items-center">
                    <div className="inline-flex items-center gap-1.5 text-[11px] text-[rgb(var(--foreground-muted))]">
                      <Clock size={12} className="text-[rgb(var(--accent))]" />
                      <span>Expires in</span>
                    </div>
                    <div
                      className={cn(
                        "text-2xl sm:text-3xl font-extrabold tracking-tight font-mono mt-0.5 transition-colors",
                        timeLeft <= 15 && timeLeft > 0
                          ? "text-[rgb(var(--warning))]"
                          : "text-[rgb(var(--foreground))]"
                      )}
                    >
                      {timeFormatted}
                    </div>

                    <button
                      type="button"
                      onClick={generatePairing}
                      className="mt-2 inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] cursor-pointer transition-colors"
                    >
                      <RotateCcw size={12} />
                      Get a new code
                    </button>
                  </div>
                </div>
              </div>
            </div>

            {/* Bottom Progress Pipeline: 4-Stage Connected Horizontal State Tracker */}
            <div className="rounded-2xl px-4 py-3.5 bg-[rgba(var(--card),0.55)] border border-[rgba(var(--border),0.07)]">
              <div className="relative grid grid-cols-4 items-start">
                {/* Continuous Background Track Line connecting centers of col 1 -> col 4 */}
                <div
                  aria-hidden="true"
                  className="pointer-events-none absolute top-4 left-[12.5%] right-[12.5%] h-[2px] -translate-y-1/2 bg-[rgba(var(--border),0.1)] rounded-full overflow-hidden"
                >
                  <div
                    className="h-full bg-[rgb(var(--accent))] transition-all duration-500 ease-out"
                    style={{ width: `${progressPercent}%` }}
                  />
                </div>

                {PAIRING_PHASES.map((phase, idx) => {
                  const isDone = idx < currentPhaseIndex;
                  const isActive = idx === currentPhaseIndex;

                  return (
                    <div
                      key={phase.id}
                      className="relative z-10 flex flex-col items-center text-center px-1"
                    >
                      <div
                        className={cn(
                          "w-8 h-8 rounded-full flex items-center justify-center transition-all duration-300",
                          isActive
                            ? "bg-[rgb(var(--card))] text-[rgb(var(--accent))] border-2 border-[rgb(var(--accent))] shadow-[0_0_14px_rgba(var(--accent),0.35)]"
                            : isDone
                            ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] border border-[rgb(var(--accent))]"
                            : "bg-[rgb(var(--card))] text-[rgb(var(--foreground-muted))]/50 border border-[rgba(var(--border),0.12)]"
                        )}
                      >
                        {isDone ? (
                          <Check size={14} strokeWidth={2.75} />
                        ) : (
                          <phase.Icon
                            size={14}
                            strokeWidth={isActive ? 2.25 : 1.75}
                          />
                        )}
                      </div>

                      <div className="mt-2 flex flex-col items-center">
                        <span
                          className={cn(
                            "text-[11px] sm:text-xs leading-tight transition-colors",
                            isActive
                              ? "text-[rgb(var(--foreground))] font-bold"
                              : isDone
                              ? "text-[rgb(var(--accent))] font-semibold"
                              : "text-[rgb(var(--foreground-muted))]/60 font-medium"
                          )}
                        >
                          {phase.label}
                        </span>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          </>
        )}
      </div>
    </Modal>
  );
});

PairingModal.displayName = "PairingModal";