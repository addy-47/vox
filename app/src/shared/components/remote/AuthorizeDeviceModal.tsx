import { memo, useState, useEffect, useCallback, useRef } from "react";
import { Shield, Mic, Play, Command, BarChart3, Clock, Check, X } from "lucide-react";
import { Modal } from "@/shared/ui/Modal";

export interface AuthorizeDeviceContentProps {
  deviceName?: string;
  deviceId?: string;
  timeoutSeconds?: number;
  onApprove: () => void | Promise<void>;
  onDeny: () => void | Promise<void>;
  onClose?: () => void;
}

export const AuthorizeDeviceContent = memo(
  ({
    deviceName,
    timeoutSeconds = 30,
    onApprove,
    onDeny,
    onClose,
  }: AuthorizeDeviceContentProps) => {
    const [secondsLeft, setSecondsLeft] = useState(timeoutSeconds);
    const [isActionPending, setIsActionPending] = useState(false);
    const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

    const clearTimer = useCallback(() => {
      if (timerRef.current !== null) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    }, []);

    const handleApprove = useCallback(async () => {
      if (isActionPending) return;
      setIsActionPending(true);
      clearTimer();
      try {
        await onApprove();
      } finally {
        setIsActionPending(false);
        onClose?.();
      }
    }, [isActionPending, onApprove, onClose, clearTimer]);

    const handleDeny = useCallback(async () => {
      if (isActionPending) return;
      setIsActionPending(true);
      clearTimer();
      try {
        await onDeny();
      } finally {
        setIsActionPending(false);
        onClose?.();
      }
    }, [isActionPending, onDeny, onClose, clearTimer]);

    useEffect(() => {
      setSecondsLeft(timeoutSeconds);
      setIsActionPending(false);
      clearTimer();

      timerRef.current = setInterval(() => {
        setSecondsLeft((prev) => {
          if (prev <= 1) {
            clearTimer();
            void onDeny();
            onClose?.();
            return 0;
          }
          return prev - 1;
        });
      }, 1000);

      return () => clearTimer();
    }, [timeoutSeconds, onDeny, onClose, clearTimer]);

    useEffect(() => {
      const onKeyDown = (e: KeyboardEvent) => {
        if (isActionPending) return;
        if (e.key === "Enter") {
          e.preventDefault();
          void handleApprove();
        } else if (e.key === "Escape") {
          e.preventDefault();
          void handleDeny();
        }
      };
      window.addEventListener("keydown", onKeyDown);
      return () => window.removeEventListener("keydown", onKeyDown);
    }, [isActionPending, handleApprove, handleDeny]);

    const formattedTime = `00:${String(secondsLeft).padStart(2, "0")}`;

    return (
      <div className="flex flex-col justify-between h-full p-6 sm:p-8 md:p-9 select-none">
        {/* Header */}
        <div className="flex items-start justify-between gap-4">
          <div className="flex items-start gap-3">
            <div className="text-[rgb(var(--accent))] shrink-0 mt-0.5">
              <Shield size={24} className="text-[rgb(var(--accent))]" />
            </div>
            <div className="flex flex-col min-w-0">
              <span className="text-[11px] font-mono tracking-[0.2em] uppercase font-bold text-[rgb(var(--accent))]">
                REMOTE CONTROL
              </span>
              <h2 className="text-2xl sm:text-[26px] font-bold tracking-tight text-[rgb(var(--foreground))] mt-0.5">
                Authorize Remote Device
              </h2>
              <p className="text-[12.5px] text-[rgb(var(--foreground-muted))] mt-1 leading-normal max-w-[480px]">
                {deviceName
                  ? `${deviceName} is requesting permission to control Vox over your local Wi-Fi network.`
                  : "A companion device is requesting permission to control Vox over your local Wi-Fi network."}
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={handleDeny}
            aria-label="Close"
            className="w-8 h-8 rounded-full bg-[rgba(var(--foreground),0.06)] hover:bg-[rgba(var(--foreground),0.12)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] flex items-center justify-center transition-colors cursor-pointer shrink-0"
          >
            <X size={16} />
          </button>
        </div>

        {/* 2-Column Responsive Body */}
        <div className="grid grid-cols-1 md:grid-cols-12 gap-8 items-center mt-7">
          {/* Left Column: Permissions List */}
          <div className="md:col-span-7 flex flex-col">
            <span className="text-[11px] font-mono tracking-wider uppercase font-semibold text-[rgb(var(--foreground-muted))]/80 mb-3.5 block">
              THIS DEVICE CAN
            </span>

            <div className="space-y-3.5">
              {/* Permission 1: Microphone */}
              <div className="flex items-start gap-3.5">
                <div className="w-10 h-10 rounded-xl bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                  <Mic size={18} />
                </div>
                <div className="flex flex-col min-w-0 pt-0.5">
                  <span className="text-[13.5px] font-semibold text-[rgb(var(--foreground))]">
                    Stream microphone audio
                  </span>
                  <span className="text-[12px] text-[rgb(var(--foreground-muted))] mt-0.5 leading-relaxed">
                    Send live audio from this device into Vox's speech pipeline.
                  </span>
                </div>
              </div>

              {/* Permission 2: Session Steering */}
              <div className="flex items-start gap-3.5">
                <div className="w-10 h-10 rounded-xl bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                  <Play size={16} className="translate-x-0.5" />
                </div>
                <div className="flex flex-col min-w-0 pt-0.5">
                  <span className="text-[13.5px] font-semibold text-[rgb(var(--foreground))]">
                    Start, pause, and trigger voice sessions
                  </span>
                  <span className="text-[12px] text-[rgb(var(--foreground-muted))] mt-0.5 leading-relaxed">
                    Control real-time voice interactions and remote tools.
                  </span>
                </div>
              </div>

              {/* Permission 3: Push-to-Talk */}
              <div className="flex items-start gap-3.5">
                <div className="w-10 h-10 rounded-xl bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                  <Command size={17} />
                </div>
                <div className="flex flex-col min-w-0 pt-0.5">
                  <span className="text-[13.5px] font-semibold text-[rgb(var(--foreground))]">
                    Trigger push-to-talk
                  </span>
                  <span className="text-[12px] text-[rgb(var(--foreground-muted))] mt-0.5 leading-relaxed">
                    Send speech dictation input remotely.
                  </span>
                </div>
              </div>

              {/* Permission 4: Session State */}
              <div className="flex items-start gap-3.5">
                <div className="w-10 h-10 rounded-xl bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                  <BarChart3 size={17} />
                </div>
                <div className="flex flex-col min-w-0 pt-0.5">
                  <span className="text-[13.5px] font-semibold text-[rgb(var(--foreground))]">
                    Read session state
                  </span>
                  <span className="text-[12px] text-[rgb(var(--foreground-muted))] mt-0.5 leading-relaxed">
                    View real-time session telemetry and pipeline status over local Wi-Fi.
                  </span>
                </div>
              </div>
            </div>
          </div>

          {/* Right Column: Connection Graphic & Countdown */}
          <div className="md:col-span-5 flex flex-col items-center justify-center">
            {/* SVG Phone to Laptop Illustration matching Image 2 */}
            <div className="relative w-full max-w-[230px] h-[105px] flex items-center justify-center">
              <svg
                viewBox="0 0 230 105"
                className="w-full h-full overflow-visible select-none pointer-events-none"
                fill="none"
                aria-hidden="true"
              >
                {/* Phone Outline (Left) */}
                <rect
                  x="8"
                  y="18"
                  width="38"
                  height="68"
                  rx="7"
                  className="stroke-[rgba(var(--border),0.4)] fill-[rgba(var(--card),0.9)]"
                  strokeWidth="1.5"
                />
                {/* Phone screen inner */}
                <rect
                  x="12"
                  y="23"
                  width="30"
                  height="58"
                  rx="4"
                  className="fill-[rgba(var(--background),0.95)]"
                />
                {/* Phone glowing orb */}
                <circle cx="27" cy="52" r="4.5" className="fill-[rgb(var(--accent))] animate-pulse" />
                <circle
                  cx="27"
                  cy="52"
                  r="8.5"
                  className="stroke-[rgb(var(--accent))]"
                  strokeWidth="1"
                  strokeOpacity="0.4"
                />

                {/* Dotted Wireless Beam */}
                <line
                  x1="54"
                  y1="52"
                  x2="116"
                  y2="52"
                  className="stroke-[rgb(var(--accent))]"
                  strokeWidth="2"
                  strokeDasharray="4 4"
                  strokeOpacity="0.75"
                />

                {/* Laptop Radar Waves (Behind screen) */}
                <circle
                  cx="165"
                  cy="46"
                  r="42"
                  className="stroke-[rgb(var(--accent))]"
                  strokeWidth="1"
                  strokeOpacity="0.12"
                />
                <circle
                  cx="165"
                  cy="46"
                  r="30"
                  className="stroke-[rgb(var(--accent))]"
                  strokeWidth="1"
                  strokeOpacity="0.25"
                />

                {/* Laptop Screen Body */}
                <rect
                  x="124"
                  y="20"
                  width="80"
                  height="54"
                  rx="6"
                  className="stroke-[rgba(var(--border),0.4)] fill-[rgba(var(--card),0.9)]"
                  strokeWidth="1.5"
                />
                {/* Laptop screen inner */}
                <rect
                  x="128"
                  y="24"
                  width="72"
                  height="46"
                  rx="4"
                  className="fill-[rgba(var(--background),0.95)]"
                />

                {/* Vox glowing ring orb on laptop */}
                <circle
                  cx="164"
                  cy="47"
                  r="12"
                  className="stroke-[rgb(var(--accent))] fill-[rgba(var(--card),0.9)]"
                  strokeWidth="2"
                  strokeOpacity="0.9"
                />
                <circle cx="164" cy="47" r="4" className="fill-[rgb(var(--accent))]" />

                {/* Laptop Base */}
                <path
                  d="M 114 74 L 216 74 C 217.5 74 218.5 75 217.5 76.5 L 213 81.5 C 212.5 82 211.5 82.5 210.5 82.5 L 119.5 82.5 C 118.5 82.5 117.5 82 117 81.5 L 112.5 76.5 C 111.5 75 112.5 74 114 74 Z"
                  className="fill-[rgba(var(--card),0.95)] stroke-[rgb(var(--accent))]"
                  strokeWidth="0.8"
                  strokeOpacity="0.4"
                />
              </svg>
            </div>

            {/* Countdown Timer */}
            <div className="flex flex-col items-center mt-3 text-center">
              <div className="flex items-center gap-1.5 text-[rgb(var(--foreground-muted))] text-[12px] font-medium">
                <Clock size={13} className="text-[rgb(var(--accent))]" />
                <span>Request expires in</span>
              </div>

              <span className="text-3xl font-mono font-bold tracking-wider text-[rgb(var(--foreground))] mt-1">
                {formattedTime}
              </span>

              <p className="text-[11.5px] leading-relaxed text-[rgb(var(--foreground-muted))]/70 text-center max-w-[220px] mt-2">
                If you don't approve, this request will expire automatically and the device won't be able to connect.
              </p>
            </div>
          </div>
        </div>

        {/* Footer Actions */}
        <div className="flex items-center gap-4 mt-8 pt-2">
          {/* Deny Button */}
          <button
            type="button"
            onClick={handleDeny}
            disabled={isActionPending}
            className="flex-1 h-12 rounded-2xl bg-[rgba(var(--foreground),0.05)] hover:bg-[rgba(var(--foreground),0.09)] active:scale-[0.99] border border-[rgba(var(--border),0.12)] text-[rgb(var(--foreground))] font-semibold text-sm flex items-center justify-center gap-2 transition-all cursor-pointer disabled:opacity-50"
          >
            <X size={16} />
            <span>Deny</span>
          </button>

          {/* Approve Button */}
          <button
            type="button"
            onClick={handleApprove}
            disabled={isActionPending}
            className="flex-1 h-12 rounded-2xl bg-[rgb(var(--accent))] hover:brightness-105 active:scale-[0.99] text-[rgb(var(--accent-foreground))] font-bold text-sm flex items-center justify-center gap-2 transition-all cursor-pointer shadow-[0_0_24px_rgba(var(--accent),0.35)] disabled:opacity-50"
          >
            <Check size={17} strokeWidth={2.5} />
            <span>Approve Connection</span>
          </button>
        </div>
      </div>
    );
  }
);
AuthorizeDeviceContent.displayName = "AuthorizeDeviceContent";

export interface AuthorizeDeviceModalProps extends AuthorizeDeviceContentProps {
  open: boolean;
}

export const AuthorizeDeviceModal = memo(
  ({
    open,
    deviceName,
    deviceId,
    timeoutSeconds = 30,
    onApprove,
    onDeny,
    onClose,
  }: AuthorizeDeviceModalProps) => {
    return (
      <Modal
        open={open}
        onClose={onClose ?? (() => {})}
        position="global"
        fitContent
        className="max-w-[760px] w-[94vw] md:w-full overflow-hidden"
      >
        <div className="min-h-[500px] flex flex-col justify-between">
          <AuthorizeDeviceContent
            deviceName={deviceName}
            deviceId={deviceId}
            timeoutSeconds={timeoutSeconds}
            onApprove={onApprove}
            onDeny={onDeny}
            onClose={onClose}
          />
        </div>
      </Modal>
    );
  }
);
AuthorizeDeviceModal.displayName = "AuthorizeDeviceModal";
