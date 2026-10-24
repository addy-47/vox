import { memo, useState, useEffect, useRef, useCallback } from "react";
import jsQR from "jsqr";
import { Camera, AlertCircle, ArrowRight, Laptop } from "lucide-react";
import { Modal } from "@/shared/ui/Modal";
import { useRemoteStore } from "@/store/remoteStore";
import { OrbitalLoader } from "@/shared/components/common";

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

export const QrScannerModal = memo(
  ({ open, onClose, defaultManual = false }: QrScannerModalProps) => {
    const connectHost = useRemoteStore((s) => s.connectHost);
    const linkState = useRemoteStore((s) => s.linkState);

    const [isManual, setIsManual] = useState(defaultManual);
    const [manualAddress, setManualAddress] = useState("");
    const [connecting, setConnecting] = useState(false);
    const [cameraError, setCameraError] = useState<string | null>(null);
    const [scanError, setScanError] = useState<string | null>(null);

    const videoRef = useRef<HTMLVideoElement | null>(null);
    const canvasRef = useRef<HTMLCanvasElement | null>(null);
    const streamRef = useRef<MediaStream | null>(null);
    const animFrameRef = useRef<number | null>(null);
    const scanIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

    const stopCamera = useCallback(() => {
      if (canvasRef.current) {
        canvasRef.current.width = 1;
        canvasRef.current.height = 1;
      }
      if (animFrameRef.current !== null) {
        cancelAnimationFrame(animFrameRef.current);
        animFrameRef.current = null;
      }
      if (scanIntervalRef.current !== null) {
        clearInterval(scanIntervalRef.current);
        scanIntervalRef.current = null;
      }
      if (streamRef.current) {
        streamRef.current.getTracks().forEach((track) => {
          try {
            track.stop();
          } catch {
            // ignore
          }
        });
        streamRef.current = null;
      }
      if (videoRef.current) {
        videoRef.current.srcObject = null;
      }
    }, []);

    const handleConnect = useCallback(
      async (address: string, deviceId?: string) => {
        try {
          setConnecting(true);
          setScanError(null);
          await connectHost({ address, deviceId });
          stopCamera();
          onClose();
        } catch (e) {
          console.warn("[QrScannerModal] connectHost failed:", e);
          setScanError(e instanceof Error ? e.message : String(e));
          setConnecting(false);
        }
      },
      [connectHost, stopCamera, onClose]
    );

    // Camera scanner loop
    useEffect(() => {
      if (!open || isManual) {
        stopCamera();
        return;
      }

      let active = true;
      setCameraError(null);

      const startCamera = async () => {
        try {
          if (typeof navigator.mediaDevices.getUserMedia !== "function") {
            throw new Error("Camera API not available in this environment.");
          }

          const stream = await navigator.mediaDevices.getUserMedia({
            video: { facingMode: "environment" },
            audio: false,
          });

          if (!active) {
            stream.getTracks().forEach((t) => t.stop());
            return;
          }

          streamRef.current = stream;

          if (videoRef.current) {
            videoRef.current.srcObject = stream;
            videoRef.current.setAttribute("playsinline", "true");
            await videoRef.current.play();
          }

          // Scan at ~20 fps (every 50ms)
          scanIntervalRef.current = setInterval(() => {
            if (!active) return;
            const video = videoRef.current;
            const canvas = canvasRef.current;
            if (!video || !canvas || video.readyState !== video.HAVE_ENOUGH_DATA) return;

            const width = video.videoWidth;
            const height = video.videoHeight;
            if (width === 0 || height === 0) return;

            canvas.width = width;
            canvas.height = height;

            const ctx = canvas.getContext("2d", { willReadFrequently: true });
            if (!ctx) return;

            ctx.drawImage(video, 0, 0, width, height);
            const imageData = ctx.getImageData(0, 0, width, height);
            const code = jsQR(imageData.data, imageData.width, imageData.height, {
              inversionAttempts: "dontInvert",
            });

            if (code && code.data) {
              try {
                const parsed = JSON.parse(code.data) as ScannedQrPayload;
                if (typeof parsed.hint_addr === "string" && typeof parsed.hint_port === "number") {
                  const target = `${parsed.hint_addr}:${parsed.hint_port}`;
                  stopCamera();
                  void handleConnect(target, typeof parsed.device_id === "string" ? parsed.device_id : undefined);
                }
              } catch (e) {
                console.debug("[QrScannerModal] Invalid QR json:", e);
              }
            }
          }, 50);
        } catch (err) {
          console.warn("[QrScannerModal] Camera initialization error:", err);
          if (active) {
            setCameraError(
              err instanceof Error ? err.message : "Unable to access camera."
            );
            setIsManual(true);
          }
        }
      };

      void startCamera();

      return () => {
        active = false;
        stopCamera();
      };
    }, [open, isManual, stopCamera, handleConnect]);

    // Close when connected
    useEffect(() => {
      if (open && linkState === "Connected") {
        stopCamera();
        onClose();
      }
    }, [open, linkState, stopCamera, onClose]);

    // Clean up on component unmount
    useEffect(() => {
      return () => stopCamera();
    }, [stopCamera]);

    return (
      <Modal
        open={open}
        onClose={() => {
          stopCamera();
          onClose();
        }}
        title="Connect to Vox Host"
        subtitle={
          isManual
            ? "Enter the host IP address and port shown on your laptop"
            : "Point camera at the QR code displayed on your laptop screen"
        }
        position="page"
        className="max-w-md w-full"
      >
        <div className="flex flex-col p-6 gap-4">
          {connecting ? (
            <div className="flex flex-col items-center justify-center p-8 gap-3">
              <OrbitalLoader size="md" />
              <span className="text-xs font-mono text-[rgb(var(--foreground-muted))]">
                Connecting to host...
              </span>
            </div>
          ) : isManual ? (
            /* Manual IP Entry View */
            <div className="flex flex-col gap-4">
              {cameraError && (
                <div className="flex items-center gap-2 p-3 rounded-xl bg-[rgba(var(--error),0.1)] border border-[rgba(var(--error),0.2)] text-[rgb(var(--error))] text-xs font-medium">
                  <AlertCircle size={15} className="shrink-0" />
                  <span>Camera unavailable: {cameraError}</span>
                </div>
              )}

              {scanError && (
                <div className="flex items-center gap-2 p-3 rounded-xl bg-[rgba(var(--error),0.1)] border border-[rgba(var(--error),0.2)] text-[rgb(var(--error))] text-xs font-medium">
                  <AlertCircle size={15} className="shrink-0" />
                  <span>Connection failed: {scanError}</span>
                </div>
              )}

              <div className="flex flex-col gap-1.5">
                <label className="text-xs uppercase font-mono font-bold tracking-wider text-[rgb(var(--foreground-muted))]">
                  Host Address (IP:Port)
                </label>
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    value={manualAddress}
                    onChange={(e) => setManualAddress(e.target.value)}
                    placeholder="192.168.1.100:7700"
                    className="flex-1 px-3.5 py-2.5 rounded-xl bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.25)] text-sm font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 focus:outline-none focus:border-[rgb(var(--accent))] transition-colors"
                  />
                  <button
                    type="button"
                    disabled={!manualAddress.trim()}
                    onClick={() => void handleConnect(manualAddress.trim())}
                    className="inline-flex items-center justify-center gap-1.5 px-4 py-2.5 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--background))] text-xs font-mono font-bold hover:opacity-90 disabled:opacity-40 transition-opacity cursor-pointer shrink-0"
                  >
                    Connect
                    <ArrowRight size={13} strokeWidth={2.5} />
                  </button>
                </div>
              </div>

              {!cameraError && (
                <button
                  type="button"
                  onClick={() => setIsManual(false)}
                  className="mt-2 inline-flex items-center justify-center gap-1.5 text-xs font-mono text-[rgb(var(--accent))] hover:underline cursor-pointer"
                >
                  <Camera size={13} />
                  Switch to QR Camera Scanner
                </button>
              )}
            </div>
          ) : (
            /* Camera Viewfinder View */
            <div className="flex flex-col items-center gap-4">
              <div className="relative w-full aspect-square max-w-[280px] rounded-2xl overflow-hidden bg-black shadow-inner border border-[rgba(var(--border),0.2)]">
                <video
                  ref={videoRef}
                  className="w-full h-full object-cover"
                  muted
                  playsInline
                />
                <canvas ref={canvasRef} className="hidden" />

                {/* Reticle Overlay */}
                <div className="absolute inset-8 pointer-events-none border border-white/20 rounded-xl">
                  {/* Top-left corner */}
                  <div className="absolute top-0 left-0 w-5 h-5 border-t-2 border-l-2 border-[rgb(var(--accent))] rounded-tl-md" />
                  {/* Top-right corner */}
                  <div className="absolute top-0 right-0 w-5 h-5 border-t-2 border-r-2 border-[rgb(var(--accent))] rounded-tr-md" />
                  {/* Bottom-left corner */}
                  <div className="absolute bottom-0 left-0 w-5 h-5 border-b-2 border-l-2 border-[rgb(var(--accent))] rounded-bl-md" />
                  {/* Bottom-right corner */}
                  <div className="absolute bottom-0 right-0 w-5 h-5 border-b-2 border-r-2 border-[rgb(var(--accent))] rounded-br-md" />

                  {/* Animated Scanning Beam */}
                  <div className="absolute left-0 right-0 h-[2px] bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.8)] animate-bounce" />
                </div>
              </div>

              {scanError && (
                <div className="flex items-center gap-2 p-2.5 rounded-xl bg-[rgba(var(--error),0.1)] text-[rgb(var(--error))] text-xs font-medium">
                  <AlertCircle size={14} className="shrink-0" />
                  <span>{scanError}</span>
                </div>
              )}

              <button
                type="button"
                onClick={() => setIsManual(true)}
                className="inline-flex items-center gap-1.5 text-xs font-mono text-[rgb(var(--accent))] hover:underline cursor-pointer"
              >
                <Laptop size={13} />
                Enter host IP manually
              </button>
            </div>
          )}
        </div>
      </Modal>
    );
  }
);

QrScannerModal.displayName = "QrScannerModal";
