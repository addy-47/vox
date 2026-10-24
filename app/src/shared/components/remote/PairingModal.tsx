import { memo, useState, useEffect, useCallback, useRef } from "react";
import {
  RotateCcw,
  AlertCircle,
  Clock,
  Check,
  X,
  Wifi,
  ShieldCheck,
  Radio,
} from "lucide-react";
import { Modal } from "@/shared/ui/Modal";
import { useRemoteStore } from "@/store/remoteStore";
import { OrbitalLoader } from "@/shared/components/common";

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

export const PairingModal = memo(({ open, onClose }: PairingModalProps) => {
  const pairNewDevice = useRemoteStore((s) => s.pairNewDevice);
  const linkState = useRemoteStore((s) => s.linkState);
  const connectedPeer = useRemoteStore((s) => s.connectedPeer);
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

      const remaining = Math.max(0, Math.floor((res.expiresAt - Date.now()) / 1000));

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

  // Auto-close if a remote connection successfully establishes
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

  const minutes = Math.floor(timeLeft / 60);
  const seconds = timeLeft % 60;
  const timeFormatted = `${minutes}:${seconds.toString().padStart(2, "0")}`;

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Pair Remote Controller"
      subtitle="Connect your mobile device to control Vox wirelessly"
      position="global"
      className="max-w-2xl w-[94vw] md:w-full"
    >
      <div className="p-6">
        {loading ? (
          <div className="h-64 flex flex-col items-center justify-center gap-3">
            <OrbitalLoader size="md" />
            <span className="text-xs font-mono text-[rgb(var(--foreground-muted))]">
              Generating secure pairing token...
            </span>
          </div>
        ) : error ? (
          <div className="flex flex-col items-center justify-center p-8 gap-3 text-center">
            <AlertCircle className="text-[rgb(var(--error))] w-8 h-8" />
            <p className="text-sm font-medium text-[rgb(var(--foreground))]">
              Failed to generate pairing QR
            </p>
            <p className="text-xs text-[rgb(var(--foreground-muted))] font-mono max-w-sm break-words">
              {error}
            </p>
            <button
              type="button"
              onClick={generatePairing}
              className="mt-2 inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg bg-[rgba(var(--accent),0.1)] hover:bg-[rgba(var(--accent),0.2)] text-[rgb(var(--accent))] text-xs font-semibold cursor-pointer transition-colors"
            >
              <RotateCcw size={13} />
              Try Again
            </button>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-12 gap-6 items-center">
            {/* Left Column: Instructions, Status, and Approval Prompt */}
            <div className="md:col-span-7 flex flex-col justify-between h-full space-y-4">
              <div className="space-y-3">
                <div className="flex items-start gap-2.5">
                  <div className="w-5 h-5 rounded-full bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] text-[11px] font-bold flex items-center justify-center shrink-0 mt-0.5">
                    1
                  </div>
                  <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed">
                    Open the <strong className="text-[rgb(var(--foreground))]">Vox</strong> app on your mobile device connected to the same Wi-Fi network.
                  </p>
                </div>

                <div className="flex items-start gap-2.5">
                  <div className="w-5 h-5 rounded-full bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] text-[11px] font-bold flex items-center justify-center shrink-0 mt-0.5">
                    2
                  </div>
                  <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed">
                    Tap the <strong className="text-[rgb(var(--foreground))]">Remote</strong> button in the top header, then tap <strong className="text-[rgb(var(--foreground))]">Scan to Pair</strong>.
                  </p>
                </div>

                <div className="flex items-start gap-2.5">
                  <div className="w-5 h-5 rounded-full bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] text-[11px] font-bold flex items-center justify-center shrink-0 mt-0.5">
                    3
                  </div>
                  <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed">
                    Point your mobile camera at this QR code and confirm the pairing prompt when displayed.
                  </p>
                </div>
              </div>

              {/* Dynamic State Banner: Approval Prompt OR Connection Status */}
              {pendingPairingPrompt ? (
                <div className="rounded-xl p-3.5 bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.3)] space-y-2.5 animate-in fade-in zoom-in-95 duration-200">
                  <div className="flex items-center gap-2 text-xs font-semibold text-[rgb(var(--accent))]">
                    <Radio size={14} className="animate-pulse" />
                    Incoming Pairing Request
                  </div>
                  <div className="text-xs">
                    <p className="font-bold text-[rgb(var(--foreground))]">
                      {pendingPairingPrompt.displayName}
                    </p>
                    <p className="text-[10px] font-mono text-[rgb(var(--foreground-muted))] truncate">
                      ID: {pendingPairingPrompt.deviceId}
                    </p>
                  </div>
                  <div className="flex items-center gap-2 pt-1">
                    <button
                      type="button"
                      disabled={approving !== null}
                      onClick={() => handleDecision("Approve")}
                      className="flex-1 inline-flex items-center justify-center gap-1 px-3 py-1.5 rounded-lg bg-[rgb(var(--accent))] text-[rgb(var(--background))] text-xs font-bold hover:opacity-90 disabled:opacity-50 transition-all cursor-pointer"
                    >
                      <Check size={13} />
                      {approving === "Approve" ? "Approving..." : "Approve Device"}
                    </button>
                    <button
                      type="button"
                      disabled={approving !== null}
                      onClick={() => handleDecision("Reject")}
                      className="px-3 py-1.5 rounded-lg border border-[rgba(var(--foreground),0.15)] text-xs text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--foreground),0.3)] disabled:opacity-50 transition-all cursor-pointer"
                    >
                      <X size={13} />
                    </button>
                  </div>
                </div>
              ) : linkState === "Connected" ? (
                <div className="rounded-xl p-3.5 bg-emerald-500/10 border border-emerald-500/30 flex items-center gap-2.5">
                  <ShieldCheck size={18} className="text-emerald-500 shrink-0" />
                  <div className="text-xs">
                    <p className="font-semibold text-emerald-500">Connected</p>
                    <p className="text-[11px] text-[rgb(var(--foreground-muted))]">
                      Paired with {connectedPeer?.displayName || "Mobile Controller"}
                    </p>
                  </div>
                </div>
              ) : (
                <div className="rounded-xl p-3 bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.07)] space-y-2">
                  <div className="flex items-center justify-between text-xs">
                    <div className="inline-flex items-center gap-1.5 text-[rgb(var(--foreground-muted))] font-mono text-[11px]">
                      <Clock size={12} className="text-[rgb(var(--accent))]" />
                      <span>
                        Expires in <strong className="text-[rgb(var(--foreground))]">{timeFormatted}</strong>
                      </span>
                    </div>
                    {timeLeft > 0 && (
                      <button
                        type="button"
                        onClick={generatePairing}
                        className="inline-flex items-center gap-1 text-[11px] font-medium text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                        title="Regenerate token"
                      >
                        <RotateCcw size={11} />
                        Refresh
                      </button>
                    )}
                  </div>
                  {details && (
                    <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgb(var(--foreground-muted))]/80">
                      <Wifi size={11} className="text-[rgb(var(--foreground-muted))]" />
                      <span>Host: <span>{`${details.hintAddr}:${details.hintPort}`}</span></span>
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Right Column: QR Code Container */}
            <div className="md:col-span-5 flex flex-col items-center justify-center">
              {details && (
                <div className="relative group">
                  <div
                    className={`p-3 rounded-2xl bg-white shadow-xl transition-all duration-300 ${
                      timeLeft === 0 ? "opacity-25 grayscale scale-95" : "opacity-100 scale-100"
                    }`}
                  >
                    <img
                      src={details.qrDataUrl}
                      alt="Remote Pairing QR Code"
                      className="w-48 h-48 sm:w-52 sm:h-52 block select-none pointer-events-none"
                    />
                  </div>

                  {timeLeft === 0 && (
                    <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-[rgba(var(--background),0.9)] backdrop-blur-xs rounded-2xl p-4 text-center">
                      <span className="text-xs font-bold text-[rgb(var(--foreground))]">
                        QR Code Expired
                      </span>
                      <button
                        type="button"
                        onClick={generatePairing}
                        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[rgb(var(--accent))] text-[rgb(var(--background))] text-xs font-bold cursor-pointer hover:opacity-90 transition-opacity"
                      >
                        <RotateCcw size={13} />
                        Regenerate
                      </button>
                    </div>
                  )}
                </div>
              )}
              <span className="text-[10px] text-[rgb(var(--foreground-muted))] mt-2 text-center font-mono">
                Noise XX mutual authentication
              </span>
            </div>
          </div>
        )}
      </div>
    </Modal>
  );
});

PairingModal.displayName = "PairingModal";
