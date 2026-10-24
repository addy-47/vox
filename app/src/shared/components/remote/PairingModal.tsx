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
  ShieldAlert,
} from "lucide-react";
import { Modal } from "@/shared/ui/Modal";
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

const computeRemainingSeconds = (exp: number): number => {
  const expMs = exp > 0 && exp < 10_000_000_000 ? exp * 1000 : exp;
  const rem = Math.max(0, Math.ceil((expMs - Date.now()) / 1000));
  return rem > 0 ? rem : 120;
};

export const PairingModal = memo(({ open, onClose }: PairingModalProps) => {
  const pairNewDevice = useRemoteStore((s) => s.pairNewDevice);
  const linkState = useRemoteStore((s) => s.linkState);
  const pendingPairingPrompt = useRemoteStore((s) => s.pendingPairingPrompt);
  const approvePairing = useRemoteStore((s) => s.approvePairing);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [details, setDetails] = useState<QrDetails | null>(null);
  const [timeLeft, setTimeLeft] = useState<number>(120);
  const [promptTimeLeft, setPromptTimeLeft] = useState<number>(30);
  const [approving, setApproving] = useState<"Approve" | "Reject" | null>(null);

  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const promptTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const clearPromptTimer = useCallback(() => {
    if (promptTimerRef.current !== null) {
      clearInterval(promptTimerRef.current);
      promptTimerRef.current = null;
    }
  }, []);

  const handleDecision = useCallback(
    async (decision: "Approve" | "Reject") => {
      clearPromptTimer();
      try {
        setApproving(decision);
        await approvePairing(decision);
      } catch (e) {
        console.warn("[PairingModal] approvePairing error:", e);
      } finally {
        setApproving(null);
      }
    },
    [approvePairing, clearPromptTimer]
  );

  useEffect(() => {
    if (pendingPairingPrompt) {
      let remaining = 30;
      setPromptTimeLeft(30);
      clearPromptTimer();

      promptTimerRef.current = setInterval(() => {
        remaining -= 1;
        setPromptTimeLeft(remaining);
        if (remaining <= 0) {
          clearPromptTimer();
          void handleDecision("Reject");
        }
      }, 1000);
    } else {
      clearPromptTimer();
    }

    return () => clearPromptTimer();
  }, [pendingPairingPrompt, clearPromptTimer, handleDecision]);

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

      const remaining = computeRemainingSeconds(res.expiresAt);

      setDetails({
        qrDataUrl: res.qrDataUrl,
        expiresAt: res.expiresAt,
        hintAddr,
        hintPort,
      });
      setTimeLeft(remaining);

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
    let active = true;
    if (open) {
      void Promise.resolve().then(() => {
        if (active) {
          void generatePairing();
        }
      });
    } else {
      clearTimer();
    }
    return () => {
      active = false;
      clearTimer();
    };
  }, [open, generatePairing, clearTimer]);

  useEffect(() => {
    if (open && linkState === "Connected") {
      onClose();
    }
  }, [open, linkState, onClose]);

  const handleDownloadApk = () => {
    window.open("https://github.com/addy-47/vox/releases", "_blank");
  };

  const minutes = Math.floor(timeLeft / 60);
  const seconds = timeLeft % 60;
  const timeFormatted = `${minutes}:${seconds.toString().padStart(2, "0")}`;

  return (
    <Modal
      open={open}
      onClose={onClose}
      icon={
        pendingPairingPrompt ? (
          <ShieldAlert size={18} className="text-[rgb(var(--accent))] shrink-0" />
        ) : (
          <Link2 size={18} className="text-[rgb(var(--accent))] shrink-0 rotate-45" />
        )
      }
      title={pendingPairingPrompt ? "Authorize Remote Device" : "Pair Mobile Controller"}
      subtitle={
        pendingPairingPrompt
          ? "A companion device is requesting authorization to control Vox over local Wi-Fi."
          : "Scan this QR code from the Vox mobile app to link your devices over local Wi-Fi."
      }
      position="global"
      fitContent
      className="max-w-[760px] w-[94vw] md:w-full overflow-hidden"
    >
      <div className="p-3.5 sm:p-5 md:p-6 md:sm:p-7 space-y-3.5 md:space-y-6">
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
            {/* Top Grid: On small screens QR is on top, steps below; on md+ desktop, side-by-side */}
            <div className="grid grid-cols-1 md:grid-cols-12 gap-3.5 md:gap-6 items-stretch">
              {/* Left Column: order-2 on mobile, order-1 on desktop */}
              <div className="order-2 md:order-1 md:col-span-7 flex flex-col justify-between p-3 sm:p-4 md:p-5 md:sm:p-6 md:pb-2.5 md:sm:pb-3">
                {pendingPairingPrompt ? (
                  /* Left Column: Host Impact & Permissions Info */
                  <div className="flex flex-col justify-between h-full space-y-4">
                    <div className="space-y-3">
                      <div>
                        <span className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
                          Requesting Device
                        </span>
                        <h3 className="text-sm md:text-base font-bold text-[rgb(var(--foreground))] mt-0.5">
                          {pendingPairingPrompt.displayName}
                        </h3>
                        <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/75 truncate mt-0.5">
                          ID: {pendingPairingPrompt.deviceId}
                        </p>
                      </div>

                      <div className="pt-3 border-t border-[rgba(var(--border),0.08)] space-y-2">
                        <span className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
                          Capabilities & Host Impact
                        </span>
                        <ul className="text-xs text-[rgb(var(--foreground-muted))]/90 space-y-2 leading-relaxed">
                          <li className="flex items-start gap-2">
                            <span className="text-[rgb(var(--accent))] font-bold text-sm leading-none">•</span>
                            <span>Stream phone microphone audio directly into local speech pipeline.</span>
                          </li>
                          <li className="flex items-start gap-2">
                            <span className="text-[rgb(var(--accent))] font-bold text-sm leading-none">•</span>
                            <span>Start, pause, and trigger voice sessions via remote controls.</span>
                          </li>
                          <li className="flex items-start gap-2">
                            <span className="text-[rgb(var(--accent))] font-bold text-sm leading-none">•</span>
                            <span>Trigger push-to-talk speech dictation remotely.</span>
                          </li>
                          <li className="flex items-start gap-2">
                            <span className="text-[rgb(var(--accent))] font-bold text-sm leading-none">•</span>
                            <span>Read real-time session telemetry and pipeline state over local Wi-Fi.</span>
                          </li>
                        </ul>
                      </div>
                    </div>

                    <p className="text-[11px] text-[rgb(var(--foreground-muted))]/60 italic pt-2">
                      You can revoke this device at any time from Settings &gt; Interaction.
                    </p>
                  </div>
                ) : (
                  /* Left Column: 3-Step Setup Stepper */
                  <>
                    <div className="relative space-y-2.5 md:space-y-14">
                      {/* Continuous Vertical Timeline Spine */}
                      <div
                        aria-hidden="true"
                        className="pointer-events-none absolute left-4 md:left-5 top-4 md:top-5 bottom-4 md:bottom-5 w-px -translate-x-1/2 bg-gradient-to-b from-[rgba(var(--accent),0.45)] via-[rgba(var(--border),0.15)] to-[rgba(var(--accent),0.35)]"
                      />

                      {/* Step 1: Install App */}
                      <div className="relative flex items-start gap-2.5 md:gap-3.5">
                        <div className="relative z-10 w-8 h-8 md:w-10 md:h-10 rounded-lg md:rounded-xl bg-[rgb(var(--card))] border border-[rgba(var(--accent),0.25)] shadow-[0_0_12px_rgba(var(--accent),0.08)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                          <AndroidIcon size={16} className="md:w-[19px] md:h-[19px]" />
                        </div>
                        <div className="flex-1 min-w-0 pt-0.5">
                          <div className="flex items-center gap-1.5 md:gap-2">
                            <span className="text-[10px] md:text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                              01
                            </span>
                            <h4 className="text-xs md:text-sm font-bold text-[rgb(var(--foreground))] leading-tight md:leading-snug">
                              Install the Companion App
                            </h4>
                          </div>
                          <p className="text-[11px] md:text-xs text-[rgb(var(--foreground-muted))] mt-0.5 md:mt-1 leading-snug md:leading-relaxed">
                            Download and install the companion APK on your phone.{" "}
                            <button
                              type="button"
                              onClick={handleDownloadApk}
                              className="inline-flex items-center gap-1 text-[11px] md:text-xs font-semibold text-[rgb(var(--accent))] hover:underline cursor-pointer align-baseline transition-opacity hover:opacity-80"
                            >
                              <span>Download</span>
                              <Download size={11} className="shrink-0" />
                            </button>
                          </p>
                        </div>
                      </div>

                      {/* Step 2: Open QR Scanner */}
                      <div className="relative flex items-start gap-2.5 md:gap-3.5">
                        <div className="relative z-10 w-8 h-8 md:w-10 md:h-10 rounded-lg md:rounded-xl bg-[rgb(var(--card))] border border-[rgba(var(--accent),0.25)] shadow-[0_0_12px_rgba(var(--accent),0.08)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                          <ScanLine size={15} className="md:w-[18px] md:h-[18px]" />
                        </div>
                        <div className="flex-1 min-w-0 pt-0.5">
                          <div className="flex items-center gap-1.5 md:gap-2">
                            <span className="text-[10px] md:text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                              02
                            </span>
                            <h4 className="text-xs md:text-sm font-bold text-[rgb(var(--foreground))] leading-tight md:leading-snug">
                              Open QR Scanner in App
                            </h4>
                          </div>
                          <p className="text-[11px] md:text-xs text-[rgb(var(--foreground-muted))] mt-0.5 md:mt-1 leading-snug md:leading-relaxed">
                            Launch Vox on your phone and tap the scanner icon in the top bar.
                          </p>
                        </div>
                      </div>

                      {/* Step 3: Point & Confirm */}
                      <div className="relative flex items-start gap-2.5 md:gap-3.5">
                        <div className="relative z-10 w-8 h-8 md:w-10 md:h-10 rounded-lg md:rounded-xl bg-[rgb(var(--card))] border border-[rgba(var(--accent),0.25)] shadow-[0_0_12px_rgba(var(--accent),0.08)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                          <Smartphone size={15} className="md:w-[18px] md:h-[18px]" />
                        </div>
                        <div className="flex-1 min-w-0 pt-0.5">
                          <div className="flex items-center gap-1.5 md:gap-2">
                            <span className="text-[10px] md:text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                              03
                            </span>
                            <h4 className="text-xs md:text-sm font-bold text-[rgb(var(--foreground))] leading-tight md:leading-snug">
                              Scan Code & Authorize
                            </h4>
                          </div>
                          <p className="text-[11px] md:text-xs text-[rgb(var(--foreground-muted))] mt-0.5 md:mt-1 leading-snug md:leading-relaxed">
                            Aim your camera at the QR code, then approve the prompt on this screen.
                          </p>
                        </div>
                      </div>
                    </div>

                    {/* Host Network Address Footer */}
                    {details && (
                      <div className="mt-2.5 md:mt-auto pt-2.5 md:pt-3 border-t border-[rgba(var(--border),0.06)] flex items-center justify-between text-[11px] md:text-xs">
                        <div className="flex items-center gap-1.5 md:gap-2 text-[rgb(var(--foreground-muted))]">
                          <Wifi
                            size={12}
                            className="text-[rgb(var(--accent))] shrink-0 md:w-[13px] md:h-[13px]"
                          />
                          <span>Desktop Host</span>
                        </div>
                        <span className="font-mono text-[11px] md:text-xs font-semibold text-[rgb(var(--foreground))]">
                          {details.hintAddr}:{details.hintPort}
                        </span>
                      </div>
                    )}
                  </>
                )}
              </div>

              {/* Right Column: order-1 on mobile, order-2 on desktop */}
              <div className="order-1 md:order-2 md:col-span-5 flex flex-col">
                <div className="h-full w-full rounded-2xl bg-[rgba(var(--card),0.8)] border border-[rgba(var(--border),0.08)] p-3 sm:p-4 md:p-5 md:sm:p-6 flex flex-col items-center justify-between text-center shadow-sm">
                  {pendingPairingPrompt ? (
                    /* Approval Yes / No UI */
                    <>
                      <div className="my-auto flex flex-col items-center gap-3 w-full max-w-[240px]">
                        <div className="w-12 h-12 rounded-xl bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.25)] flex items-center justify-center text-[rgb(var(--accent))] mb-1">
                          <Radio size={22} className="animate-pulse" />
                        </div>
                        <span className="text-sm font-bold text-[rgb(var(--foreground))]">
                          Approve Connection?
                        </span>
                        <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed">
                          Allow <strong className="text-[rgb(var(--foreground))]">{pendingPairingPrompt.displayName}</strong> to control Vox.
                        </p>

                        <div className="w-full flex flex-col gap-2 pt-2">
                          <button
                            type="button"
                            disabled={approving !== null}
                            onClick={() => void handleDecision("Approve")}
                            className="w-full h-9 inline-flex items-center justify-center gap-2 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-xs font-bold hover:opacity-90 disabled:opacity-50 transition-opacity cursor-pointer"
                          >
                            <Check size={14} strokeWidth={2.5} />
                            {approving === "Approve" ? "Approving..." : "Approve (Yes)"}
                          </button>
                          <button
                            type="button"
                            disabled={approving !== null}
                            onClick={() => void handleDecision("Reject")}
                            className="w-full h-9 inline-flex items-center justify-center gap-2 rounded-xl border border-[rgba(var(--border),0.15)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--border),0.3)] disabled:opacity-50 text-xs font-semibold transition-colors cursor-pointer"
                          >
                            <X size={14} />
                            {approving === "Reject" ? "Rejecting..." : "Deny (No)"}
                          </button>
                        </div>
                      </div>

                      {/* Approval Countdown Timer in the exact same slot */}
                      <div className="w-full pt-2 md:pt-3.5 mt-2 md:mt-3.5 border-t border-[rgba(var(--border),0.06)] flex items-center justify-between md:flex-col md:items-center">
                        <div className="flex items-center gap-1.5 md:flex-col md:items-center">
                          <div className="inline-flex items-center gap-1 text-[11px] text-[rgb(var(--foreground-muted))]">
                            <Clock size={11} className="text-[rgb(var(--accent))] md:w-[12px] md:h-[12px]" />
                            <span>Auto-expires in</span>
                          </div>
                          <div
                            className={cn(
                              "font-mono font-extrabold tracking-tight transition-colors text-sm md:text-2xl md:sm:text-3xl md:mt-0.5",
                              promptTimeLeft <= 10
                                ? "text-[rgb(var(--warning))]"
                                : "text-[rgb(var(--foreground))]"
                            )}
                          >
                            0:{promptTimeLeft.toString().padStart(2, "0")}
                          </div>
                        </div>
                      </div>
                    </>
                  ) : (
                    /* Default QR Display */
                    <>
                      {details && (
                        <div className="relative group my-auto">
                          <div
                            className={cn(
                              "p-2.5 sm:p-3 md:p-3.5 rounded-xl md:rounded-2xl bg-white shadow-md transition-all duration-300",
                              timeLeft === 0
                                ? "opacity-20 grayscale scale-95"
                                : "opacity-100 scale-100"
                            )}
                          >
                            <img
                              src={details.qrDataUrl}
                              alt="Remote Pairing QR Code"
                              className="w-40 h-40 sm:w-48 sm:h-48 md:w-56 md:h-56 md:sm:w-60 md:sm:h-60 block select-none pointer-events-none"
                            />
                          </div>

                          {timeLeft === 0 && (
                            <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-[rgba(var(--background),0.92)] rounded-xl md:rounded-2xl p-3 md:p-4 text-center">
                              <span className="text-xs font-bold text-[rgb(var(--foreground))]">
                                QR Code Expired
                              </span>
                              <button
                                type="button"
                                onClick={generatePairing}
                                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-xs font-bold cursor-pointer hover:opacity-90 transition-opacity"
                              >
                                <RotateCcw size={12} />
                                Regenerate
                              </button>
                            </div>
                          )}
                        </div>
                      )}

                      {/* Countdown & Regenerate Action */}
                      <div className="w-full pt-2 md:pt-3.5 mt-2 md:mt-3.5 border-t border-[rgba(var(--border),0.06)] flex items-center justify-between md:flex-col md:items-center">
                        <div className="flex items-center gap-1.5 md:flex-col md:items-center">
                          <div className="inline-flex items-center gap-1 text-[11px] text-[rgb(var(--foreground-muted))]">
                            <Clock size={11} className="text-[rgb(var(--accent))] md:w-[12px] md:h-[12px]" />
                            <span>Expires in</span>
                          </div>
                          <div
                            className={cn(
                              "font-mono font-extrabold tracking-tight transition-colors text-sm md:text-2xl md:sm:text-3xl md:mt-0.5",
                              timeLeft <= 15 && timeLeft > 0
                                ? "text-[rgb(var(--warning))]"
                                : "text-[rgb(var(--foreground))]"
                            )}
                          >
                            {timeFormatted}
                          </div>
                        </div>

                        <button
                          type="button"
                          onClick={generatePairing}
                          className="inline-flex items-center gap-1 px-2.5 py-1 md:mt-2 md:px-3 md:py-1.5 rounded-lg text-xs font-semibold text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] cursor-pointer transition-colors"
                        >
                          <RotateCcw size={11} className="md:w-[12px] md:h-[12px]" />
                          <span>Get a new code</span>
                        </button>
                      </div>
                    </>
                  )}
                </div>
              </div>
            </div>
          </>
        )}
      </div>
    </Modal>
  );
});

PairingModal.displayName = "PairingModal";