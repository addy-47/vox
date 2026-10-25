import { memo, useState, useEffect, useCallback, useRef } from "react";
import {
  RotateCcw,
  AlertCircle,
  Clock,
  Check,
  CheckCircle2,
  X,
  Wifi,
  Radio,
  Download,
  ScanLine,
  Smartphone,
  Link2,
  ShieldCheck,
  Mic,
  Activity,
  Zap,
} from "lucide-react";
import { Modal } from "@/shared/ui/Modal";
import { useRemoteStore } from "@/store/remoteStore";
import { OrbitalLoader } from "@/shared/components/common";
import { REMOTE_CONTROL_COPY } from "@/data/settingsCopy";
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

interface CountdownBadgeProps {
  expiresAtMs: number;
  onExpire?: () => void;
}

const CountdownBadge = memo(({ expiresAtMs, onExpire }: CountdownBadgeProps) => {
  const [secondsLeft, setSecondsLeft] = useState(() =>
    Math.max(0, Math.ceil((expiresAtMs - Date.now()) / 1000))
  );

  useEffect(() => {
    const update = () => {
      const rem = Math.max(0, Math.ceil((expiresAtMs - Date.now()) / 1000));
      setSecondsLeft(rem);
      if (rem <= 0) {
        onExpire?.();
      }
    };
    update();
    const id = setInterval(update, 1000);
    return () => clearInterval(id);
  }, [expiresAtMs, onExpire]);

  const minutes = Math.floor(secondsLeft / 60);
  const seconds = secondsLeft % 60;
  const timeFormatted = `${minutes}:${seconds.toString().padStart(2, "0")}`;

  return (
    <div
      className={cn(
        "font-mono font-extrabold tracking-tight transition-colors text-sm md:text-2xl md:sm:text-3xl md:mt-0.5",
        secondsLeft <= 15 && secondsLeft > 0
          ? "text-[rgb(var(--warning))]"
          : "text-[rgb(var(--foreground))]"
      )}
    >
      {timeFormatted}
    </div>
  );
});

CountdownBadge.displayName = "CountdownBadge";

export const PairingModal = memo(({ open, onClose }: PairingModalProps) => {
  const pairNewDevice = useRemoteStore((s) => s.pairNewDevice);
  const linkState = useRemoteStore((s) => s.linkState);
  const connectedPeer = useRemoteStore((s) => s.connectedPeer);
  const pendingPairingPrompt = useRemoteStore((s) => s.pendingPairingPrompt);
  const approvePairing = useRemoteStore((s) => s.approvePairing);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [details, setDetails] = useState<QrDetails | null>(null);
  const [isExpired, setIsExpired] = useState(false);
  const [promptTimeLeft, setPromptTimeLeft] = useState<number>(30);
  const [approving, setApproving] = useState<"Approve" | "Reject" | null>(null);
  const [showSuccess, setShowSuccess] = useState(false);

  const promptTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

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
    if (!open) {
      clearPromptTimer();
      return;
    }
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
  }, [open, pendingPairingPrompt, clearPromptTimer, handleDecision]);

  useEffect(() => {
    if (!open || !pendingPairingPrompt || approving !== null) return undefined;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        void handleDecision("Approve");
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, pendingPairingPrompt, approving, handleDecision]);

  const generatePairing = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      setIsExpired(false);

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

      const expMs =
        typeof res.expiresAtMs === "number" && res.expiresAtMs > 0
          ? res.expiresAtMs
          : res.expiresAt > 0 && res.expiresAt < 10_000_000_000
          ? res.expiresAt * 1000
          : res.expiresAt;

      setDetails({
        qrDataUrl: res.qrDataUrl,
        expiresAt: expMs,
        hintAddr,
        hintPort,
      });
    } catch (e) {
      console.warn("[PairingModal] pairNewDevice error:", e);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [pairNewDevice]);

  useEffect(() => {
    let active = true;
    if (open) {
      setShowSuccess(false);
      void Promise.resolve().then(() => {
        if (active) {
          void generatePairing();
        }
      });
    } else {
      setShowSuccess(false);
      clearPromptTimer();
    }
    return () => {
      active = false;
      clearPromptTimer();
    };
  }, [open, generatePairing, clearPromptTimer]);

  useEffect(() => {
    if (open && linkState === "Connected") {
      setShowSuccess(true);
      const timer = setTimeout(() => {
        onClose();
      }, 800);
      return () => clearTimeout(timer);
    }
    return undefined;
  }, [open, linkState, onClose]);

  const handleExpire = useCallback(() => {
    setIsExpired(true);
  }, []);

  const handleDownloadApk = () => {
    window.open("https://github.com/addy-47/vox/releases", "_blank");
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      icon={
        showSuccess ? (
          <CheckCircle2 size={18} className="text-[rgb(var(--success))] shrink-0" />
        ) : pendingPairingPrompt ? (
          <ShieldCheck size={18} className="text-[rgb(var(--accent))] shrink-0" />
        ) : (
          <Link2 size={18} className="text-[rgb(var(--accent))] shrink-0 rotate-45" />
        )
      }
      title={
        showSuccess
          ? REMOTE_CONTROL_COPY.successTitleHost
          : pendingPairingPrompt
          ? "Authorize Remote Controller"
          : "Pair Mobile Controller"
      }
      subtitle={
        showSuccess
          ? REMOTE_CONTROL_COPY.successSubtitleHost(
              connectedPeer?.displayName ?? pendingPairingPrompt?.displayName ?? "Mobile Controller"
            )
          : pendingPairingPrompt
          ? "A companion device is requesting authorization to control Vox over local Wi-Fi."
          : "Scan this QR code from the Vox mobile app to link your devices over local Wi-Fi."
      }
      position="global"
      fitContent
      className={cn(
        "w-[94vw] overflow-hidden transition-all duration-300",
        pendingPairingPrompt ? "max-w-[540px]" : "max-w-[760px] md:w-full"
      )}
    >
      <div className="p-3.5 sm:p-5 md:p-6 md:sm:p-7 space-y-3.5 md:space-y-6">
        {showSuccess ? (
          <div className="h-72 flex flex-col items-center justify-center gap-3 animate-fade-in text-center p-6">
            <div className="w-14 h-14 rounded-2xl bg-[rgba(var(--success),0.12)] border border-[rgba(var(--success),0.25)] flex items-center justify-center text-[rgb(var(--success))] shadow-[0_0_24px_rgba(var(--success),0.15)] animate-in zoom-in-95 duration-200">
              <CheckCircle2 size={32} strokeWidth={2.5} />
            </div>
            <h3 className="text-base font-bold text-[rgb(var(--foreground))]">
              {REMOTE_CONTROL_COPY.successTitleHost}
            </h3>
            <p className="text-xs text-[rgb(var(--foreground-muted))] max-w-sm">
              {REMOTE_CONTROL_COPY.successSubtitleHost(
                connectedPeer?.displayName ?? pendingPairingPrompt?.displayName ?? "Mobile Controller"
              )}
            </p>
          </div>
        ) : loading ? (
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
        ) : pendingPairingPrompt ? (
          /* Redesigned Immaculate Security Authorization Dialog */
          <div className="space-y-4 animate-fade-in">
            {/* Device Identity Card */}
            <div className="flex items-center gap-3.5 p-3.5 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.08)]">
              <div className="w-11 h-11 rounded-xl bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                <Smartphone size={20} />
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <h3 className="text-sm font-bold text-[rgb(var(--foreground))] truncate">
                    {pendingPairingPrompt.displayName}
                  </h3>
                  <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-mono font-medium bg-[rgba(var(--success),0.12)] text-[rgb(var(--success))] border border-[rgba(var(--success),0.2)] shrink-0">
                    <Wifi size={10} />
                    Local LAN
                  </span>
                </div>
                <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] truncate mt-0.5">
                  Fingerprint: {pendingPairingPrompt.deviceId}
                </p>
              </div>
            </div>

            {/* Granted Capabilities Grid */}
            <div className="space-y-2">
              <span className="text-xs font-semibold text-[rgb(var(--foreground))]">
                Granted Controller Privileges
              </span>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                <div className="flex items-start gap-2.5 p-2.5 rounded-lg bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.06)]">
                  <div className="w-6 h-6 rounded-md bg-[rgba(var(--accent),0.08)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0 mt-0.5">
                    <Mic size={13} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <span className="block text-xs font-semibold text-[rgb(var(--foreground))]">
                      Audio Streaming
                    </span>
                    <span className="block text-[11px] text-[rgb(var(--foreground-muted))] leading-tight mt-0.5">
                      Ingest phone mic into speech pipeline
                    </span>
                  </div>
                </div>

                <div className="flex items-start gap-2.5 p-2.5 rounded-lg bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.06)]">
                  <div className="w-6 h-6 rounded-md bg-[rgba(var(--accent),0.08)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0 mt-0.5">
                    <Activity size={13} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <span className="block text-xs font-semibold text-[rgb(var(--foreground))]">
                      Session Steering
                    </span>
                    <span className="block text-[11px] text-[rgb(var(--foreground-muted))] leading-tight mt-0.5">
                      Start, pause, and trigger voice sessions
                    </span>
                  </div>
                </div>

                <div className="flex items-start gap-2.5 p-2.5 rounded-lg bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.06)]">
                  <div className="w-6 h-6 rounded-md bg-[rgba(var(--accent),0.08)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0 mt-0.5">
                    <Zap size={13} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <span className="block text-xs font-semibold text-[rgb(var(--foreground))]">
                      Push-to-Talk
                    </span>
                    <span className="block text-[11px] text-[rgb(var(--foreground-muted))] leading-tight mt-0.5">
                      Trigger wireless speech dictation
                    </span>
                  </div>
                </div>

                <div className="flex items-start gap-2.5 p-2.5 rounded-lg bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.06)]">
                  <div className="w-6 h-6 rounded-md bg-[rgba(var(--accent),0.08)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0 mt-0.5">
                    <Radio size={13} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <span className="block text-xs font-semibold text-[rgb(var(--foreground))]">
                      Telemetry Sync
                    </span>
                    <span className="block text-[11px] text-[rgb(var(--foreground-muted))] leading-tight mt-0.5">
                      Read live pipeline state and HUD data
                    </span>
                  </div>
                </div>
              </div>
            </div>

            {/* Countdown Progress Track */}
            <div className="space-y-1.5 pt-1">
              <div className="flex items-center justify-between text-[11px]">
                <div className="flex items-center gap-1.5 text-[rgb(var(--foreground-muted))]">
                  <Clock
                    size={11}
                    className={cn(
                      promptTimeLeft <= 10
                        ? "text-[rgb(var(--warning))]"
                        : "text-[rgb(var(--accent))]"
                    )}
                  />
                  <span>Auto-declining in</span>
                </div>
                <span
                  className={cn(
                    "font-mono font-bold text-xs",
                    promptTimeLeft <= 10
                      ? "text-[rgb(var(--warning))]"
                      : "text-[rgb(var(--foreground))]"
                  )}
                >
                  {promptTimeLeft}s
                </span>
              </div>
              <div className="h-1 w-full rounded-full bg-[rgba(var(--border),0.08)] overflow-hidden">
                <div
                  className={cn(
                    "h-full transition-all duration-1000 ease-linear rounded-full",
                    promptTimeLeft <= 10
                      ? "bg-[rgb(var(--warning))]"
                      : "bg-[rgb(var(--accent))]"
                  )}
                  style={{ width: `${(promptTimeLeft / 30) * 100}%` }}
                />
              </div>
            </div>

            {/* Actions: Decline & Authorize */}
            <div className="flex items-center gap-3 pt-2">
              <button
                type="button"
                disabled={approving !== null}
                onClick={() => void handleDecision("Reject")}
                className="flex-1 h-10 inline-flex items-center justify-center gap-2 rounded-xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--foreground),0.02)] hover:bg-[rgba(var(--error),0.06)] hover:border-[rgba(var(--error),0.3)] text-xs font-semibold text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer disabled:opacity-50"
              >
                <X size={14} />
                <span>{approving === "Reject" ? "Declining..." : "Decline"}</span>
                <kbd className="hidden sm:inline-block px-1.5 py-0.5 rounded text-[10px] font-mono bg-[rgba(var(--foreground),0.06)] border border-[rgba(var(--border),0.12)] text-[rgb(var(--foreground-muted))]">
                  Esc
                </kbd>
              </button>

              <button
                type="button"
                disabled={approving !== null}
                onClick={() => void handleDecision("Approve")}
                className="flex-1 h-10 inline-flex items-center justify-center gap-2 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-xs font-bold hover:opacity-95 shadow-sm transition-opacity cursor-pointer disabled:opacity-50"
              >
                <Check size={14} strokeWidth={2.5} />
                <span>{approving === "Approve" ? "Authorizing..." : "Authorize Device"}</span>
                <kbd className="hidden sm:inline-block px-1.5 py-0.5 rounded text-[10px] font-mono bg-black/15 text-[rgb(var(--accent-foreground))]">
                  ↵ Enter
                </kbd>
              </button>
            </div>

            {/* Bottom security reassurance */}
            <p className="text-[11px] text-center text-[rgb(var(--foreground-muted))]/60 pt-1">
              You can revoke this device at any time from Settings → Remote Control.
            </p>
          </div>
        ) : (
          /* Default QR Display and 3-step Stepper */
          <div className="grid grid-cols-1 md:grid-cols-12 gap-3.5 md:gap-6 items-stretch">
            {/* Left Column: order-2 on mobile, order-1 on desktop */}
            <div className="order-2 md:order-1 md:col-span-7 flex flex-col justify-between p-3 sm:p-4 md:p-5 md:sm:p-6 md:pb-2.5 md:sm:pb-3">
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
            </div>

            {/* Right Column: order-1 on mobile, order-2 on desktop */}
            <div className="order-1 md:order-2 md:col-span-5 flex flex-col">
              <div className="h-full w-full rounded-2xl bg-[rgba(var(--card),0.8)] border border-[rgba(var(--border),0.08)] p-3 sm:p-4 md:p-5 md:sm:p-6 flex flex-col items-center justify-between text-center shadow-sm">
                {/* Default QR Display */}
                {details && (
                  <div className="relative group my-auto">
                    <div
                      className={cn(
                        "p-2.5 sm:p-3 md:p-3.5 rounded-xl md:rounded-2xl bg-white shadow-md transition-all duration-300",
                        isExpired
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

                    {isExpired && (
                      <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-[rgba(var(--background),0.92)] rounded-xl md:rounded-2xl p-3 md:p-4 text-center">
                        <span className="text-xs font-bold text-[rgb(var(--foreground))]">
                          QR Code Expired
                        </span>
                        <button
                          type="button"
                          onClick={generatePairing}
                          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[rgb(var(--accent))] text-xs font-bold cursor-pointer hover:opacity-90 transition-opacity"
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
                    {details && (
                      <CountdownBadge
                        expiresAtMs={details.expiresAt}
                        onExpire={handleExpire}
                      />
                    )}
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
              </div>
            </div>
          </div>
        )}
      </div>
    </Modal>
  );
});

PairingModal.displayName = "PairingModal";