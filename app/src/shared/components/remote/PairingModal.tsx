import { memo, useState, useEffect, useCallback, useRef } from "react";
import {
  RotateCcw,
  AlertCircle,
  Clock,
  Wifi,
  ScanLine,
  Smartphone,
  Link2,
} from "lucide-react";
import { useRemoteStore } from "@/store/remoteStore";
import type { RemoteLinkState } from "@/services/remoteService";
import { OrbitalLoader } from "@/shared/components/common";
import { RemoteModalShell } from "./RemoteModalShell";
import { cn } from "@/shared/lib/utils";

export interface PairingModalProps {
  open?: boolean;
  onClose?: () => void;
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

// Module-level cache so reopening the modal within the 2-minute TTL is instant with 0 flicker
let cachedQrDetails: QrDetails | null = null;
export const _resetCachedQrDetails = () => {
  cachedQrDetails = null;
};

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
  return rem;
};

interface QrCardProps {
  details: QrDetails | null;
  loading: boolean;
  onRegenerate: () => void;
}

const QrCard = memo(({ details, loading, onRegenerate }: QrCardProps) => {
  const [timeLeft, setTimeLeft] = useState<number>(() =>
    details ? computeRemainingSeconds(details.expiresAt) : 120
  );

  useEffect(() => {
    if (!details) return;
    const initial = computeRemainingSeconds(details.expiresAt);
    setTimeLeft(initial);

    const timer = setInterval(() => {
      setTimeLeft((prev) => {
        if (prev <= 1) {
          clearInterval(timer);
          return 0;
        }
        return prev - 1;
      });
    }, 1000);

    return () => clearInterval(timer);
  }, [details]);

  const minutes = Math.floor(timeLeft / 60);
  const seconds = timeLeft % 60;
  const timeFormatted = `${minutes}:${seconds.toString().padStart(2, "0")}`;

  return (
    <div className="self-stretch h-full w-full rounded-2xl bg-[rgba(var(--card),0.8)] border border-[rgba(var(--border),0.08)] p-3.5 sm:p-4 md:p-5 flex flex-col items-center justify-between text-center shadow-sm">
      {loading && !details ? (
        <div className="my-auto flex items-center justify-center w-40 h-40 sm:w-48 sm:h-48 md:w-56 md:h-56">
          <OrbitalLoader size="md" />
        </div>
      ) : details ? (
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
              className="w-40 h-40 sm:w-48 sm:h-48 md:w-56 md:h-56 block select-none pointer-events-none"
            />
          </div>

          {timeLeft === 0 && (
            <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-[rgba(var(--background),0.92)] rounded-xl md:rounded-2xl p-3 md:p-4 text-center">
              <span className="text-xs font-bold text-[rgb(var(--foreground))]">
                QR Code Expired
              </span>
              <button
                type="button"
                onClick={onRegenerate}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[rgb(var(--accent))] text-xs font-bold cursor-pointer hover:opacity-90 transition-opacity"
              >
                <RotateCcw size={12} />
                Regenerate
              </button>
            </div>
          )}
        </div>
      ) : null}

      {/* Countdown & Regenerate Action */}
      <div className="w-full pt-2 md:pt-3 mt-2 md:mt-3 border-t border-[rgba(var(--border),0.06)] flex items-center justify-between md:flex-col md:items-center">
        <div className="flex items-center gap-1.5 md:flex-col md:items-center">
          <div className="inline-flex items-center gap-1 text-[11px] text-[rgb(var(--foreground-muted))]">
            <Clock
              size={11}
              className={cn(
                "md:w-[12px] md:h-[12px] transition-colors",
                timeLeft <= 5 && timeLeft > 0
                  ? "text-[rgb(var(--error))]"
                  : timeLeft <= 20 && timeLeft > 0
                  ? "text-[rgb(var(--warning))]"
                  : "text-[rgb(var(--accent))]"
              )}
            />
            <span>Expires in</span>
          </div>
          <div
            className={cn(
              "font-mono font-extrabold tracking-tight transition-colors text-sm md:text-3xl md:mt-0.5",
              timeLeft <= 5 && timeLeft > 0
                ? "text-[rgb(var(--error))]"
                : timeLeft <= 20 && timeLeft > 0
                ? "text-[rgb(var(--warning))]"
                : "text-[rgb(var(--foreground))]"
            )}
          >
            {timeFormatted}
          </div>
        </div>

        <button
          type="button"
          onClick={onRegenerate}
          className="inline-flex items-center gap-1 px-2.5 py-1 md:mt-2 md:px-3 md:py-1.5 rounded-lg text-xs font-semibold text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] cursor-pointer transition-colors"
        >
          <RotateCcw size={11} className="md:w-[12px] md:h-[12px]" />
          <span>Get a new code</span>
        </button>
      </div>
    </div>
  );
});

QrCard.displayName = "QrCard";

export const PairingModal = memo(({ open, onClose }: PairingModalProps) => {
  const isPairingModalOpen = useRemoteStore((s) => s.isPairingModalOpen);
  const closePairingModal = useRemoteStore((s) => s.closePairingModal);
  const pairNewDevice = useRemoteStore((s) => s.pairNewDevice);
  const linkState = useRemoteStore((s) => s.linkState);

  const effectiveOpen = open !== undefined ? open : isPairingModalOpen;
  const effectiveOnClose = onClose ?? closePairingModal;

  // Read initial cache
  const hasValidCache =
    cachedQrDetails !== null && computeRemainingSeconds(cachedQrDetails.expiresAt) > 0;

  const [details, setDetails] = useState<QrDetails | null>(
    () => (hasValidCache ? cachedQrDetails : null)
  );
  const [loading, setLoading] = useState<boolean>(() => (effectiveOpen ? !hasValidCache : false));
  const [error, setError] = useState<string | null>(null);

  const generatePairing = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

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

      const newDetails: QrDetails = {
        qrDataUrl: res.qrDataUrl,
        expiresAt: res.expiresAt,
        hintAddr,
        hintPort,
      };

      cachedQrDetails = newDetails;
      setDetails(newDetails);
    } catch (e) {
      console.warn("[PairingModal] pairNewDevice error:", e);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [pairNewDevice]);

  // Synchronize on open
  useEffect(() => {
    if (!effectiveOpen) {
      return;
    }

    if (cachedQrDetails && computeRemainingSeconds(cachedQrDetails.expiresAt) > 0) {
      const cached = cachedQrDetails;
      setTimeout(() => {
        setDetails(cached);
        setLoading(false);
      }, 0);
    } else {
      void generatePairing();
    }
  }, [effectiveOpen, generatePairing]);

  const wasOpenRef = useRef(false);
  const initialLinkStateRef = useRef<RemoteLinkState | null>(null);

  useEffect(() => {
    if (effectiveOpen && !wasOpenRef.current) {
      wasOpenRef.current = true;
      initialLinkStateRef.current = linkState;
    } else if (!effectiveOpen) {
      wasOpenRef.current = false;
      initialLinkStateRef.current = null;
    }
  }, [effectiveOpen, linkState]);

  // Close smoothly on NEW pairing completion (only if linkState transitioned after modal opened)
  useEffect(() => {
    if (
      effectiveOpen &&
      wasOpenRef.current &&
      initialLinkStateRef.current !== null &&
      linkState !== initialLinkStateRef.current &&
      (linkState === "Paired" || linkState === "Connected")
    ) {
      const timer = setTimeout(() => {
        effectiveOnClose();
      }, 800);
      return () => clearTimeout(timer);
    }
    return undefined;
  }, [effectiveOpen, linkState, effectiveOnClose]);

  const handleDownloadApk = () => {
    window.open("https://github.com/addy-47/vox/releases", "_blank");
  };

  return (
    <RemoteModalShell
      open={effectiveOpen}
      onClose={effectiveOnClose}
      ariaLabel="Pair Mobile Controller"
      kickerIcon={<Link2 size={14} className="shrink-0 rotate-45 text-[rgb(var(--accent))]" />}
      kickerLabel="REMOTE CONTROL"
      title="Pair Mobile Controller"
      description="Scan this QR code from the Vox mobile app to link your devices over local Wi-Fi."
      errorState={
        error ? (
          <>
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
              className="mt-2 inline-flex items-center gap-1.5 px-4 py-2 rounded-lg bg-[rgba(var(--accent),0.12)] hover:bg-[rgba(var(--accent),0.2)] text-[rgb(var(--accent))] text-xs font-semibold cursor-pointer transition-colors"
            >
              <RotateCcw size={13} />
              Try Again
            </button>
          </>
        ) : undefined
      }
      leftColumnClassName="order-2 md:order-1"
      rightColumnClassName="order-1 md:order-2"
      contentKey="pairing"
      leftContent={
        <div className="flex h-full flex-col justify-center">
          <div className="relative flex flex-col gap-16 md:gap-18 mt-8">
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

              {/* Step 3: Scan & Authorize */}
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

            {/* Wi-Fi host — mt-auto pins it to the bottom of the left col,
                aligning with the QR card's bottom edge (grid is items-stretch) */}
            <div className="mt-auto pt-3 border-t border-[rgba(var(--border),0.06)] flex items-center justify-between text-[11px] md:text-xs">
              <div className="flex items-center gap-1.5 md:gap-2 text-[rgb(var(--foreground-muted))]">
                <Wifi
                  size={12}
                  className="text-[rgb(var(--accent))] shrink-0 md:w-[13px] md:h-[13px]"
                />
                <span>Desktop Host</span>
              </div>
              <span className="font-mono text-[11px] md:text-xs font-semibold text-[rgb(var(--foreground))]">
                {details ? `${details.hintAddr}:${details.hintPort}` : "127.0.0.1:7700"}
              </span>
            </div>
          </div>
      }
      rightContent={
        <QrCard
          details={details}
          loading={loading}
          onRegenerate={generatePairing}
        />
      }
    />
  );
});

PairingModal.displayName = "PairingModal";
