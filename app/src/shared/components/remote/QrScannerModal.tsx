import { memo, useState, useCallback, useRef, useEffect } from "react";
import jsQR from "jsqr";
import {
  Camera,
  ArrowRight,
  Wifi,
  WifiOff,
  ShieldAlert,
  CheckCircle2,
  Flashlight,
  FlashlightOff,
  RotateCcw,
  ScanLine,
} from "lucide-react";
import { Modal } from "@/shared/ui/Modal";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useRemoteStore } from "@/store/remoteStore";
import { OrbitalLoader } from "@/shared/components/common";
import { REMOTE_CONTROL_COPY } from "@/data/settingsCopy";
import type { RemoteFailureKind } from "@/services/remoteService";
import { cn } from "@/shared/lib/utils";

interface ScannedQrPayload {
  hint_addr?: string;
  hint_port?: number;
  token?: string;
  device_id?: string;
  public_key?: string;
}

export interface QrScannerModalProps {
  open: boolean;
  onClose: () => void;
  defaultManual?: boolean;
}

export interface ErrorCopy {
  title: string;
  hint: string;
}

export const getErrorMessage = (
  kind: RemoteFailureKind | null,
  rawReason: string | null
): ErrorCopy => {
  const raw = (rawReason ?? "").toLowerCase();

  // 1. Timeout
  if (
    kind === "timeout" ||
    raw.includes("timed out") ||
    raw.includes("timeout") ||
    raw.includes("deadline")
  ) {
    return {
      title: "Connection Timed Out",
      hint: REMOTE_CONTROL_COPY.errorTimeout,
    };
  }

  // 2. Unreachable / Offline
  if (
    kind === "unreachable" ||
    raw.includes("unreachable") ||
    raw.includes("no dial candidates") ||
    raw.includes("connection refused") ||
    raw.includes("failed to lookup")
  ) {
    return {
      title: "Unable to Reach Computer",
      hint: REMOTE_CONTROL_COPY.errorUnreachable,
    };
  }

  // 3. Rejected / Declined by Host
  if (
    kind === "rejected_by_host" ||
    raw.includes("denied") ||
    raw.includes("declined") ||
    raw.includes("rejected")
  ) {
    return {
      title: "Pairing Declined",
      hint: REMOTE_CONTROL_COPY.errorRejected,
    };
  }

  // 4. Invalid or Expired Token / QR Code
  if (kind === "invalid_token" || raw.includes("token") || raw.includes("expired")) {
    return {
      title: "QR Code Expired",
      hint: REMOTE_CONTROL_COPY.errorInvalidToken,
    };
  }

  // 5. Version Mismatch
  if (kind === "version_mismatch" || raw.includes("version")) {
    return {
      title: "App Update Required",
      hint: REMOTE_CONTROL_COPY.errorVersionMismatch,
    };
  }

  // 6. Device Pairing Revoked
  if (kind === "revoked" || raw.includes("revoked")) {
    return {
      title: "Pairing Removed",
      hint: REMOTE_CONTROL_COPY.errorRevoked,
    };
  }

  // 7. Host Busy
  if (
    kind === "host_busy" ||
    raw.includes("busy") ||
    raw.includes("already attached") ||
    raw.includes("already connected")
  ) {
    return {
      title: "Computer is Busy",
      hint: REMOTE_CONTROL_COPY.errorHostBusy,
    };
  }

  // 8. Protocol / Interrupted handshake / Malformed message
  if (
    raw.includes("protocol") ||
    raw.includes("malformed") ||
    raw.includes("handshake") ||
    raw.includes("invalid host public key")
  ) {
    return {
      title: "Connection Interrupted",
      hint: REMOTE_CONTROL_COPY.errorProtocol,
    };
  }

  // 9. Generic connection error
  return {
    title: "Connection Failed",
    hint: REMOTE_CONTROL_COPY.errorGeneric,
  };
};

export const QrScannerModal = memo(
  ({ open, onClose, defaultManual = false }: QrScannerModalProps) => {
    const pairDevice = useRemoteStore((s) => s.pairDevice);
    const connectHost = useRemoteStore((s) => s.connectHost);
    const connectedPeer = useRemoteStore((s) => s.connectedPeer);
    const linkState = useRemoteStore((s) => s.linkState);
    const failureReason = useRemoteStore((s) => s.failureReason);
    const failureKind = useRemoteStore((s) => s.failureKind);
    const isPairingPending = useRemoteStore((s) => s.isPairingPending);
    const pairingFailureReason = useRemoteStore((s) => s.pairingFailureReason);
    const dismissPairingFailure = useRemoteStore((s) => s.dismissPairingFailure);

    const [isManual, setIsManual] = useState(defaultManual);
    const [manualAddress, setManualAddress] = useState("");
    const [connecting, setConnecting] = useState(false);
    const [scannedSuccess, setScannedSuccess] = useState(false);
    const [cameraActive, setCameraActive] = useState(false);
    const [cameraLoading, setCameraLoading] = useState(true);
    const [cameraError, setCameraError] = useState<string | null>(null);
    const [scanError, setScanError] = useState<string | null>(null);
    const [torchOn, setTorchOn] = useState(false);
    const [torchSupported, setTorchSupported] = useState(false);

    const videoRef = useRef<HTMLVideoElement | null>(null);
    const streamRef = useRef<MediaStream | null>(null);
    const scanIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
    const rafRef = useRef<number | null>(null);
    const canvasRef = useRef<HTMLCanvasElement | null>(null);
    const connectingRef = useRef(connecting);
    const scannedSuccessRef = useRef(scannedSuccess);
    const cameraGenRef = useRef(0);

    const stopCamera = useCallback(() => {
      cameraGenRef.current += 1;
      if (scanIntervalRef.current) {
        clearInterval(scanIntervalRef.current);
        scanIntervalRef.current = null;
      }
      if (streamRef.current) {
        streamRef.current.getTracks().forEach((track) => {
          try {
            track.stop();
          } catch {
            // Ignore track stop errors
          }
        });
        streamRef.current = null;
      }
      if (videoRef.current) {
        videoRef.current.srcObject = null;
      }
      if (canvasRef.current) {
        canvasRef.current.width = 1;
        canvasRef.current.height = 1;
      }
      setCameraActive(false);
      setCameraLoading(true);
      setTorchOn(false);
      setTorchSupported(false);
    }, []);

    const resetState = useCallback(() => {
      stopCamera();
      setIsManual(defaultManual);
      setManualAddress("");
      setConnecting(false);
      setScannedSuccess(false);
      setCameraError(null);
      setScanError(null);
    }, [defaultManual, stopCamera]);

    const handleClose = useCallback(() => {
      resetState();
      dismissPairingFailure();
      onClose();
    }, [resetState, dismissPairingFailure, onClose]);

    useEffect(() => {
      connectingRef.current = connecting;
    }, [connecting]);
    useEffect(() => {
      scannedSuccessRef.current = scannedSuccess;
    }, [scannedSuccess]);

    const handlePair = useCallback(
      async (params: { address: string; deviceId: string; token: string; publicKey: string }) => {
        try {
          setConnecting(true);
          setCameraActive(false);
          setScanError(null);
          stopCamera();
          await pairDevice({
            address: params.address,
            deviceId: params.deviceId,
            token: params.token,
            publicKey: params.publicKey,
          });
          // Do not close modal immediately; wait for remote_state_changed to report Paired or Failed
        } catch (e) {
          console.warn("[QrScannerModal] pairDevice failed:", e);
          setScanError(e instanceof Error ? e.message : String(e));
          setConnecting(false);
          setScannedSuccess(false);
        }
      },
      [pairDevice, stopCamera]
    );

    const handleManualConnect = useCallback(
      async (address: string) => {
        const trimmed = address.trim();
        if (!trimmed) return;
        if (!connectedPeer) {
          setScanError(
            "Manual connection requires an existing pairing. Scan the desktop QR code to pair for the first time."
          );
          return;
        }
        try {
          setConnecting(true);
          setScanError(null);
          await connectHost({
            deviceId: connectedPeer.deviceId,
            address: trimmed,
          });
        } catch (e) {
          console.warn("[QrScannerModal] manual connectHost failed:", e);
          setScanError(e instanceof Error ? e.message : String(e));
          setConnecting(false);
        }
      },
      [connectedPeer, connectHost]
    );

    const processQrCode = useCallback(
      (raw: string) => {
        if (!raw || connectingRef.current || scannedSuccessRef.current) return;
        try {
          const parsed = JSON.parse(raw) as Partial<ScannedQrPayload>;
          if (
            typeof parsed.hint_addr === "string" &&
            typeof parsed.hint_port === "number" &&
            typeof parsed.device_id === "string" &&
            typeof parsed.token === "string" &&
            typeof parsed.public_key === "string"
          ) {
            setScannedSuccess(true);
            const target = `${parsed.hint_addr}:${parsed.hint_port}`;
            void handlePair({
              address: target,
              deviceId: parsed.device_id,
              token: parsed.token,
              publicKey: parsed.public_key,
            });
          }
        } catch {
          // Non-JSON barcode or unrelated QR code, ignore
        }
      },
      [handlePair]
    );

    const startCamera = useCallback(async () => {
      stopCamera();
      const gen = cameraGenRef.current;
      setCameraLoading(true);
      setCameraError(null);

      try {
        if (!navigator.mediaDevices?.getUserMedia) {
          throw new Error("Camera API is not supported on this device.");
        }

        const stream = await navigator.mediaDevices.getUserMedia({
          video: {
            facingMode: { ideal: "environment" },
            width: { ideal: 640 },
            height: { ideal: 480 },
          },
          audio: false,
        });

        // If modal was closed or switched to manual while awaiting getUserMedia, kill tracks immediately!
        if (gen !== cameraGenRef.current) {
          stream.getTracks().forEach((t) => {
            try {
              t.stop();
            } catch {
              // Ignore track stop errors
            }
          });
          return;
        }

        streamRef.current = stream;

        const track = stream.getVideoTracks()[0];
        if (track && typeof track.getCapabilities === "function") {
          const caps = track.getCapabilities() as { torch?: boolean };
          setTorchSupported(Boolean(caps.torch));
        }

        if (videoRef.current) {
          videoRef.current.srcObject = stream;
          try {
            await videoRef.current.play();
          } catch (playErr) {
            console.warn("[QrScannerModal] video.play error:", playErr);
          }
        }
      } catch (err) {
        if (gen !== cameraGenRef.current) return;
        console.warn("[QrScannerModal] startCamera error:", err);
        const msg =
          err instanceof Error
            ? err.message
            : "Camera access was denied or device is not available.";
        setCameraError(msg);
        setCameraLoading(false);
      }
    }, [stopCamera]);

    const toggleTorch = async () => {
      const track = streamRef.current?.getVideoTracks()[0];
      if (!track) return;
      try {
        const nextState = !torchOn;
        // @ts-expect-error torch constraint in modern mobile browsers
        await track.applyConstraints({ advanced: [{ torch: nextState }] });
        setTorchOn(nextState);
      } catch (err) {
        console.warn("[QrScannerModal] toggleTorch error:", err);
      }
    };

    const closeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    useEffect(() => {
      return () => {
        if (closeTimerRef.current) {
          clearTimeout(closeTimerRef.current);
          closeTimerRef.current = null;
        }
      };
    }, []);

    // React to linkState transitions while modal is open
    useEffect(() => {
      if (!open) return undefined;

      if (scannedSuccess && (linkState === "Paired" || linkState === "Connected")) {
        setConnecting(false);
        if (closeTimerRef.current) {
          clearTimeout(closeTimerRef.current);
        }
        closeTimerRef.current = setTimeout(() => {
          handleClose();
        }, 800);
      } else if (connecting && linkState === "Connected") {
        setConnecting(false);
        if (closeTimerRef.current) {
          clearTimeout(closeTimerRef.current);
        }
        closeTimerRef.current = setTimeout(() => {
          handleClose();
        }, 800);
      }

      if (linkState === "Failed") {
        setConnecting(false);
        setScannedSuccess(false);
        setScanError(failureReason ?? "Pairing with host failed.");
      }
      return undefined;
    }, [open, connecting, scannedSuccess, linkState, failureReason, handleClose]);

    // Pairing failures are ephemeral (spec §5.5): they arrive on the pairing
    // state channel, not as a corrupted RemoteLinkState.
    useEffect(() => {
      if (!open || pairingFailureReason === null) return;
      setConnecting(false);
      setScannedSuccess(false);
      setScanError(pairingFailureReason);
    }, [open, pairingFailureReason]);

    // Scan loop when camera is active
    useEffect(() => {
      if (!open || isManual || connecting || !cameraActive) {
        if (scanIntervalRef.current) {
          clearInterval(scanIntervalRef.current);
          scanIntervalRef.current = null;
        }
        if (rafRef.current !== null) {
          const v = videoRef.current as
            | (HTMLVideoElement & {
                cancelVideoFrameCallback?: (h: number) => void;
              })
            | null;
          if (v && typeof v.cancelVideoFrameCallback === "function") {
            v.cancelVideoFrameCallback(rafRef.current);
          } else {
            cancelAnimationFrame(rafRef.current);
          }
          rafRef.current = null;
        }
        return;
      }

      const hasBarcodeDetector = "BarcodeDetector" in window;
      let nativeDetector: {
        detect: (
          source: HTMLVideoElement
        ) => Promise<Array<{ rawValue?: string }>>;
      } | null = null;

      if (hasBarcodeDetector) {
        try {
          const DetectorCtor = (
            window as unknown as {
              BarcodeDetector: new (opts: { formats: string[] }) => {
                detect: (
                  source: HTMLVideoElement
                ) => Promise<Array<{ rawValue?: string }>>;
              };
            }
          ).BarcodeDetector;
          nativeDetector = new DetectorCtor({ formats: ["qr_code"] });
        } catch {
          nativeDetector = null;
        }
      }

      // Only allocate the canvas for the CPU (jsQR) fallback path.
      let canvas: HTMLCanvasElement | null = null;
      let ctx: CanvasRenderingContext2D | null = null;
      if (!nativeDetector) {
        if (!canvasRef.current) {
          const c = document.createElement("canvas");
          c.width = 320;
          c.height = 320;
          canvasRef.current = c;
        }
        canvas = canvasRef.current;
        ctx = canvas?.getContext("2d", { willReadFrequently: true }) ?? null;
      }

      let isScanning = false;
      let lastTime = -1;
      let lastDecodeMs = 0;
      let stopped = false;

      const decode = async () => {
        if (stopped || isScanning) return;
        if (connectingRef.current || scannedSuccessRef.current) return;
        if (performance.now() - lastDecodeMs < 80) return; // Throttle to ~12.5 FPS
        const video = videoRef.current;
        if (!video || video.readyState < 2) return;
        if (video.currentTime === lastTime) return;
        lastTime = video.currentTime;
        lastDecodeMs = performance.now();

        isScanning = true;
        try {
          if (nativeDetector) {
            const barcodes = await nativeDetector.detect(video);
            if (barcodes && barcodes.length > 0) {
              for (const b of barcodes) {
                if (b.rawValue) {
                  processQrCode(b.rawValue);
                  break;
                }
              }
            }
          } else if (ctx && canvas) {
            const vw = video.videoWidth || 320;
            const vh = video.videoHeight || 320;
            const cropSize = Math.min(vw, vh);
            const sx = (vw - cropSize) / 2;
            const sy = (vh - cropSize) / 2;
            ctx.drawImage(
              video,
              sx,
              sy,
              cropSize,
              cropSize,
              0,
              0,
              canvas.width,
              canvas.height
            );
            const imageData = ctx.getImageData(
              0,
              0,
              canvas.width,
              canvas.height
            );
            const qr = jsQR(imageData.data, canvas.width, canvas.height, {
              inversionAttempts: "attemptBoth",
            });
            if (qr && qr.data) {
              processQrCode(qr.data);
            }
          }
        } catch {
          // Frame decode skip
        } finally {
          isScanning = false;
        }
      };

      const v = videoRef.current as
        | (HTMLVideoElement & {
            requestVideoFrameCallback?: (
              cb: (now: number, meta: { mediaTime: number }) => void
            ) => number;
          })
        | null;

      if (v && typeof v.requestVideoFrameCallback === "function") {
        const loop = () => {
          if (stopped) return;
          void decode();
          const vv = videoRef.current as
            | (HTMLVideoElement & {
                requestVideoFrameCallback?: (
                  cb: (now: number, meta: { mediaTime: number }) => void
                ) => number;
              })
            | null;
          if (vv && typeof vv.requestVideoFrameCallback === "function") {
            rafRef.current = vv.requestVideoFrameCallback(loop);
          }
        };
        rafRef.current = v.requestVideoFrameCallback(loop);
      } else {
        scanIntervalRef.current = setInterval(() => {
          void decode();
        }, 100);
      }

      const videoEl = videoRef.current;

      return () => {
        stopped = true;
        if (rafRef.current !== null) {
          const vv = videoEl as
            | (HTMLVideoElement & {
                cancelVideoFrameCallback?: (h: number) => void;
              })
            | null;
          if (vv && typeof vv.cancelVideoFrameCallback === "function") {
            vv.cancelVideoFrameCallback(rafRef.current);
          } else {
            cancelAnimationFrame(rafRef.current);
          }
          rafRef.current = null;
        }
        if (scanIntervalRef.current) {
          clearInterval(scanIntervalRef.current);
          scanIntervalRef.current = null;
        }
      };
    }, [open, isManual, connecting, cameraActive, processQrCode]);

    useEffect(() => {
      let timer: ReturnType<typeof setTimeout> | null = null;
      if (open && !isManual && !connecting) {
        // Defer camera initialization by 150ms so modal scale-in animation completes at 60fps
        timer = setTimeout(() => {
          void startCamera();
        }, 150);
      } else {
        stopCamera();
      }
      return () => {
        if (timer) clearTimeout(timer);
        stopCamera();
      };
    }, [open, isManual, connecting, startCamera, stopCamera]);

    return (
      <Modal
        open={open}
        onClose={handleClose}
        icon={
          linkState === "Connected" || (scannedSuccess && linkState === "Paired") ? (
            <CheckCircle2 size={18} className="text-[rgb(var(--accent))] shrink-0" />
          ) : scanError ? (
            <WifiOff size={18} className="text-[rgb(var(--error))] shrink-0" />
          ) : (
            <ScanLine size={18} className="text-[rgb(var(--accent))] shrink-0" />
          )
        }
        headerActions={
          !isManual && torchSupported && cameraActive && !connecting && linkState !== "Connected" ? (
            <Tooltip
              label={torchOn ? "Turn Off Torch" : "Turn On Torch"}
              side="bottom"
            >
              <button
                type="button"
                onClick={() => void toggleTorch()}
                aria-label="Toggle Flashlight"
                className="flex items-center justify-center w-8 h-8 rounded-full glass-card text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]"
              >
                {torchOn ? (
                  <Flashlight size={16} className="text-[rgb(var(--accent))]" />
                ) : (
                  <FlashlightOff size={16} />
                )}
              </button>
            </Tooltip>
          ) : undefined
        }
        title={
          scannedSuccess && (linkState === "Paired" || linkState === "Connected")
            ? "Host Paired Successfully"
            : "Connect to Desktop Host"
        }
        subtitle={
          scannedSuccess && (linkState === "Paired" || linkState === "Connected")
            ? "Your phone is now paired. Tap the rocket icon on the home screen to stream."
            : scanError
            ? "Unable to complete connection to your computer."
            : isPairingPending
            ? "A confirmation prompt has appeared on your computer."
            : connecting
            ? "Establishing secure pairing with host..."
            : isManual
            ? "Enter the IP address and port displayed in your computer's pairing window."
            : "Align your camera with the pairing QR code shown on your computer screen."
        }
        position="global"
        fitContent
        className="max-w-md w-[94vw] sm:w-full overflow-hidden"
      >
        <div className="flex flex-col p-4 sm:p-5 gap-3">
          {scanError ? (
            /* Explicit In-Modal Failure Alert & Retry View */
            <div className="flex flex-col items-center justify-center py-4 sm:py-6 text-center animate-fade-in my-auto">
              <div className="w-13 h-13 sm:w-14 sm:h-14 rounded-2xl bg-[rgba(var(--error),0.10)] border border-[rgba(var(--error),0.20)] flex items-center justify-center text-[rgb(var(--error))] shadow-[0_0_24px_rgba(var(--error),0.12)] mb-3">
                <WifiOff size={26} strokeWidth={2} />
              </div>
              <div className="flex flex-col items-center gap-1.5 max-w-sm">
                {(() => {
                  const errInfo = getErrorMessage(failureKind, scanError);
                  return (
                    <>
                      <h3 className="font-display text-base sm:text-lg font-semibold text-[rgb(var(--foreground))] tracking-tight">
                        {errInfo.title}
                      </h3>
                      <p className="font-body text-xs sm:text-sm text-[rgb(var(--foreground-muted))] leading-relaxed mt-0.5">
                        {errInfo.hint}
                      </p>
                      {scanError && scanError !== errInfo.title && (
                        <p className="font-body text-xs text-[rgb(var(--foreground-muted))]/80 leading-relaxed mt-1 break-words">
                          {scanError}
                        </p>
                      )}
                    </>
                  );
                })()}
              </div>
              <div className="flex flex-col gap-2 w-full max-w-xs mt-5">
                <button
                  type="button"
                  onClick={() => {
                    dismissPairingFailure();
                    setScanError(null);
                    setScannedSuccess(false);
                    setConnecting(false);
                    if (!isManual) void startCamera();
                  }}
                  className="w-full h-11 inline-flex items-center justify-center gap-2 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] font-display text-sm font-semibold hover:opacity-90 active:scale-[0.98] transition-all cursor-pointer shadow-sm"
                >
                  <RotateCcw size={15} strokeWidth={2.2} />
                  Scan Again / Retry
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setScanError(null);
                    setIsManual(true);
                  }}
                  className="w-full h-10 inline-flex items-center justify-center rounded-xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--foreground),0.04)] hover:bg-[rgba(var(--foreground),0.08)] font-body text-xs font-medium text-[rgb(var(--foreground))] hover:border-[rgba(var(--border),0.2)] transition-all cursor-pointer"
                >
                  Enter IP Manually
                </button>
              </div>
            </div>
          ) : scannedSuccess && (linkState === "Paired" || linkState === "Connected") ? (
            /* Paired Celebration State */
            <div className="flex flex-col items-center justify-center gap-3 py-6 animate-fade-in text-center p-4">
              <div className="w-14 h-14 rounded-2xl bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] flex items-center justify-center text-[rgb(var(--accent))] shadow-[0_0_24px_rgba(var(--accent),0.2)] animate-in zoom-in-95 duration-200">
                <CheckCircle2 size={32} strokeWidth={2.5} />
              </div>
              <h3 className="font-display text-base sm:text-lg font-semibold text-[rgb(var(--foreground))] tracking-tight">
                Host Paired Successfully
              </h3>
              <p className="font-body text-xs sm:text-sm text-[rgb(var(--foreground-muted))] max-w-xs leading-relaxed">
                Your device is paired with the host. Tap the rocket icon on the home screen to connect anytime.
              </p>
            </div>
          ) : isPairingPending ? (
            /* Dedicated Waiting for Host Approval State */
            <div
              role="status"
              aria-live="polite"
              className="flex flex-col items-center justify-center py-5 sm:py-6 text-center animate-fade-in my-auto"
            >
              <div
                aria-hidden="true"
                className="relative flex items-center justify-center w-20 h-20 mb-3"
              >
                <div className="absolute inset-0 rounded-full bg-[radial-gradient(circle,rgba(var(--accent),0.20),transparent_70%)] animate-pulse" />
                <OrbitalLoader size="md" className="relative z-10" />
              </div>

              <h3 className="font-display text-base sm:text-lg font-semibold text-[rgb(var(--foreground))] tracking-tight">
                Waiting for Desktop Approval
              </h3>
              <p className="font-body text-xs sm:text-sm text-[rgb(var(--foreground-muted))] max-w-xs mt-1 leading-relaxed">
                A confirmation prompt has appeared on your computer. Tap <strong>Approve</strong> on your computer screen to link this phone.
              </p>

              <div className="w-full max-w-xs mt-5">
                <button
                  type="button"
                  onClick={handleClose}
                  className="w-full h-10 inline-flex items-center justify-center rounded-xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--foreground),0.04)] hover:bg-[rgba(var(--foreground),0.08)] font-body text-xs font-medium text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                >
                  Cancel
                </button>
              </div>
            </div>
          ) : connecting ? (
            /* Connecting Progression State */
            <div
              role="status"
              aria-live="polite"
              className="flex flex-col items-center justify-center py-5 sm:py-6 text-center animate-fade-in my-auto"
            >
              <div
                aria-hidden="true"
                className="relative flex items-center justify-center w-20 h-20 mb-3"
              >
                <div className="absolute inset-0 rounded-full bg-[radial-gradient(circle,rgba(var(--accent),0.18),transparent_68%)]" />
                <OrbitalLoader size="md" className="relative z-10" />
              </div>

              <h3 className="font-display text-base sm:text-lg font-semibold text-[rgb(var(--foreground))] tracking-tight">
                Establishing Connection...
              </h3>
              <p className="font-body text-xs sm:text-sm text-[rgb(var(--foreground-muted))] max-w-xs mt-1 leading-relaxed">
                Establishing secure Noise link with your computer.
              </p>
            </div>
          ) : isManual ? (
            /* Manual IP Connection View */
            <div className="flex flex-col gap-4 animate-fade-in">
              {cameraError && (
                <div className="flex items-center gap-2.5 p-3 rounded-lg bg-[rgba(var(--warning),0.08)] border border-[rgba(var(--warning),0.25)] text-xs text-[rgb(var(--foreground))]">
                  <ShieldAlert
                    size={16}
                    className="text-[rgb(var(--warning))] shrink-0"
                  />
                  <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed">
                    {cameraError}
                  </p>
                </div>
              )}

              <div className="space-y-2">
                <label className="block text-xs sm:text-sm font-semibold text-[rgb(var(--foreground))]">
                  Desktop Host Address
                </label>
                <div className="relative flex items-center">
                  <div className="absolute left-3 text-[rgb(var(--foreground-muted))] pointer-events-none">
                    <Wifi size={16} />
                  </div>
                  <input
                    type="text"
                    value={manualAddress}
                    onChange={(e) => setManualAddress(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && manualAddress.trim()) {
                        void handleManualConnect(manualAddress);
                      }
                    }}
                    placeholder="192.168.1.42:7700"
                    className="w-full h-11 pl-10 pr-3 rounded-xl bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.12)] text-xs sm:text-sm font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 focus:outline-none focus:border-[rgb(var(--accent))] focus:ring-1 focus:ring-[rgb(var(--accent))] transition-colors"
                    autoFocus
                  />
                </div>
                <p className="text-[11px] sm:text-xs text-[rgb(var(--foreground-muted))] pt-0.5">
                  {connectedPeer ? (
                    <>
                      Connecting to paired host{" "}
                      <span className="text-[rgb(var(--foreground))] font-semibold">
                        {connectedPeer.displayName || connectedPeer.deviceId}
                      </span>
                      . Override IP and port if changed.
                    </>
                  ) : (
                    <>
                      Shown under the QR code in{" "}
                      <span className="text-[rgb(var(--foreground))] font-semibold">
                        Settings → Remote Control
                      </span>{" "}
                      on desktop.
                    </>
                  )}
                </p>
              </div>

              <div className="flex flex-col gap-2 pt-2">
                <button
                  type="button"
                  disabled={!manualAddress.trim()}
                  onClick={() => void handleManualConnect(manualAddress)}
                  className="w-full h-11 inline-flex items-center justify-center gap-2 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-xs sm:text-sm font-bold hover:opacity-90 disabled:opacity-40 transition-opacity cursor-pointer shadow-sm"
                >
                  Connect to Host
                  <ArrowRight size={15} strokeWidth={2.5} />
                </button>

                {!cameraError && (
                  <button
                    type="button"
                    onClick={() => {
                      setIsManual(false);
                      setScanError(null);
                    }}
                    className="w-full h-9 inline-flex items-center justify-center gap-2 rounded-lg border border-[rgba(var(--border),0.08)] bg-[rgba(var(--foreground),0.03)] hover:bg-[rgba(var(--foreground),0.06)] text-xs font-semibold text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                  >
                    <Camera size={14} />
                    Switch to QR Scanner
                  </button>
                )}
              </div>
            </div>
          ) : (
            /* Native Low-Latency Camera Scanner View */
            <div className="flex flex-col gap-4 animate-fade-in">
              {/* Viewfinder Frame */}
              <div className="relative w-full aspect-square max-w-[288px] mx-auto rounded-xl overflow-hidden bg-black border border-[rgba(var(--border),0.12)] flex items-center justify-center">
                <video
                  ref={videoRef}
                  playsInline
                  muted
                  autoPlay
                  onPlaying={() => {
                    setCameraLoading(false);
                    setCameraActive(true);
                  }}
                  className={cn(
                    "w-full h-full object-cover transition-opacity duration-300",
                    cameraActive
                      ? "opacity-100"
                      : "opacity-0 pointer-events-none"
                  )}
                />

                {/* OrbitalLoader while camera initializes */}
                {cameraLoading && (
                  <div className="absolute inset-0 flex items-center justify-center bg-black/95 z-10 p-4">
                    <div className="flex flex-col items-center gap-2">
                      <div className="w-8 h-8 rounded-full border-2 border-[rgb(var(--accent))]/30 border-t-[rgb(var(--accent))] animate-spin" />
                      <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
                        Starting camera…
                      </span>
                    </div>
                  </div>
                )}

                {/* Camera Error Overlay */}
                {cameraError && (
                  <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-black/95 z-10 text-center p-5">
                    <ShieldAlert
                      size={28}
                      className="text-[rgb(var(--warning))]"
                    />
                    <span className="text-xs font-bold text-[rgb(var(--foreground))]">
                      Camera Access Required
                    </span>
                    <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed">
                      {cameraError}
                    </p>
                    <button
                      type="button"
                      onClick={() => setIsManual(true)}
                      className="mt-1 px-4 py-2 rounded-lg bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-xs font-bold cursor-pointer"
                    >
                      Enter IP Manually
                    </button>
                  </div>
                )}

                {/* Subtle vignette around the reticle */}
                {!cameraError && (
                  <div
                    aria-hidden="true"
                    className="pointer-events-none absolute inset-0 shadow-[inset_0_0_40px_rgba(0,0,0,0.65)] z-10"
                  />
                )}

                {/* Cybernetic Corner Reticle & Smooth 60fps Bidirectional Laser */}
                {!cameraError && (
                  <div className="absolute inset-6 pointer-events-none z-10 overflow-hidden rounded-lg">
                    {/* Top-Left Corner */}
                    <div className="absolute top-0 left-0 w-7 h-7 border-t-2 border-l-2 border-[rgb(var(--accent))] rounded-tl-lg" />
                    {/* Top-Right Corner */}
                    <div className="absolute top-0 right-0 w-7 h-7 border-t-2 border-r-2 border-[rgb(var(--accent))] rounded-tr-lg" />
                    {/* Bottom-Left Corner */}
                    <div className="absolute bottom-0 left-0 w-7 h-7 border-b-2 border-l-2 border-[rgb(var(--accent))] rounded-bl-lg" />
                    {/* Bottom-Right Corner */}
                    <div className="absolute bottom-0 right-0 w-7 h-7 border-b-2 border-r-2 border-[rgb(var(--accent))] rounded-br-lg" />

                    {/* Sweeping Laser Line */}
                    {!scannedSuccess && cameraActive && !scanError && (
                      <div className="efb-scan-laser-track">
                        <div className="efb-scan-laser-beam" />
                      </div>
                    )}
                  </div>
                )}

                {/* Scanned Success Feedback */}
                {scannedSuccess && (
                  <div className="absolute inset-0 bg-black/75 flex flex-col items-center justify-center gap-2 z-20 animate-fade-in">
                    <CheckCircle2
                      size={42}
                      className="text-[rgb(var(--accent))]"
                    />
                    <span className="text-sm font-bold text-white tracking-wide">
                      Host QR Detected
                    </span>
                  </div>
                )}
              </div>

              {/* Bottom Guidance & Action Controls */}
              <div className="flex flex-col gap-3 pt-1">
                <p className="text-xs sm:text-[13px] text-center text-[rgb(var(--foreground-muted))] leading-relaxed max-w-xs mx-auto">
                  Aim your camera at the QR code shown on desktop under{" "}
                  <span className="text-[rgb(var(--foreground))] font-semibold">
                    Settings → Remote Control
                  </span>
                  .
                </p>

                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => setIsManual(true)}
                    className="flex-1 h-11 inline-flex items-center justify-center gap-2 px-4 rounded-xl bg-[rgba(var(--foreground),0.05)] hover:bg-[rgba(var(--foreground),0.09)] border border-[rgba(var(--border),0.08)] text-xs sm:text-sm font-semibold text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                  >
                    <Wifi
                      size={15}
                      className="text-[rgb(var(--accent))] shrink-0"
                    />
                    Enter IP Manually
                  </button>

                  <Tooltip label="Reset Camera" side="top">
                    <button
                      type="button"
                      onClick={() => {
                        setScanError(null);
                        setScannedSuccess(false);
                        void startCamera();
                      }}
                      aria-label="Restart Camera"
                      className="h-10 w-10 inline-flex items-center justify-center rounded-lg bg-[rgba(var(--foreground),0.05)] hover:bg-[rgba(var(--foreground),0.09)] border border-[rgba(var(--border),0.08)] text-xs font-semibold text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer shrink-0"
                    >
                      <RotateCcw size={14} />
                    </button>
                  </Tooltip>
                </div>
              </div>
            </div>
          )}
        </div>
      </Modal>
    );
  }
);

QrScannerModal.displayName = "QrScannerModal";
