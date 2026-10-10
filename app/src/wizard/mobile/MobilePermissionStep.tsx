import React, { useState, useEffect, useCallback } from 'react';
import { Mic, Bell, Check } from 'lucide-react';
import { getRuntimeReport } from '@/services/setupService';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';

interface MobilePermissionStepProps {
  onNext?: () => void;
  onPermissionsChange?: (granted: boolean) => void;
}

export const MobilePermissionStep: React.FC<MobilePermissionStepProps> = ({
  onPermissionsChange,
}) => {
  const [micGranted, setMicGranted] = useState<boolean | null>(null);
  const [notifsGranted, setNotifsGranted] = useState<boolean | null>(null);
  const [isRequesting, setIsRequesting] = useState(false);

  const copy = MOBILE_ONBOARDING_COPY.permission;

  const checkStatus = useCallback(async () => {
    try {
      const report = await getRuntimeReport().catch(() => null);
      if (report) {
        setMicGranted(Boolean(report.mic_access));
      }
    } catch (_) {}

    // Check Notification API
    if (typeof Notification !== 'undefined') {
      setNotifsGranted(Notification.permission === 'granted');
    } else {
      setNotifsGranted(true);
    }
  }, []);

  useEffect(() => {
    void checkStatus();
  }, [checkStatus]);

  useEffect(() => {
    const bothGranted = Boolean(micGranted && notifsGranted);
    onPermissionsChange?.(bothGranted);
  }, [micGranted, notifsGranted, onPermissionsChange]);

  const handleGrantAccess = async () => {
    setIsRequesting(true);
    try {
      // 1. Microphone request via WebRTC / Android wry WebChromeClient
      if (navigator.mediaDevices && navigator.mediaDevices.getUserMedia) {
        try {
          const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
          stream.getTracks().forEach((track) => track.stop());
          setMicGranted(true);
        } catch (_) {
          setMicGranted(false);
        }
      }

      // 2. Notifications request
      if (typeof Notification !== 'undefined' && Notification.requestPermission) {
        try {
          const perm = await Notification.requestPermission();
          setNotifsGranted(perm === 'granted');
        } catch (_) {
          setNotifsGranted(false);
        }
      } else {
        setNotifsGranted(true);
      }

      await checkStatus();
    } catch (_) {
    } finally {
      setIsRequesting(false);
    }
  };

  return (
    <div className="flex flex-col h-full w-full overflow-y-auto px-6 py-4 select-none bg-transparent">
      <div className="flex-1 min-h-0 flex flex-col items-center justify-center text-center my-auto">
        {/* Clean Icon Stage: Stroke-Style Icons, no heavy container borders or backgrounds */}
        <div className="relative my-6 flex items-center justify-center gap-6 shrink-0">
          {/* Mic Icon */}
          <div className="flex flex-col items-center gap-2">
            <div className="relative flex items-center justify-center">
              <Mic
                className={`w-12 h-12 stroke-[1.5] transition-colors duration-300 ${
                  micGranted
                    ? 'text-[rgb(var(--accent))]'
                    : 'text-[rgb(var(--foreground))] opacity-80'
                }`}
              />
              {micGranted && (
                <div className="absolute -top-1 -right-1 w-4 h-4 rounded-full bg-[rgb(var(--accent))] text-black flex items-center justify-center">
                  <Check className="w-3 h-3 stroke-[3]" />
                </div>
              )}
            </div>
            <span className="text-[11px] font-mono font-semibold text-[rgb(var(--foreground-muted))] uppercase tracking-wider">
              Audio
            </span>
          </div>

          <span className="w-px h-10 bg-[rgba(var(--foreground),0.1)]" />

          {/* Notifications Icon */}
          <div className="flex flex-col items-center gap-2">
            <div className="relative flex items-center justify-center">
              <Bell
                className={`w-12 h-12 stroke-[1.5] transition-colors duration-300 ${
                  notifsGranted
                    ? 'text-[rgb(var(--accent))]'
                    : 'text-[rgb(var(--foreground))] opacity-80'
                }`}
              />
              {notifsGranted && (
                <div className="absolute -top-1 -right-1 w-4 h-4 rounded-full bg-[rgb(var(--accent))] text-black flex items-center justify-center">
                  <Check className="w-3 h-3 stroke-[3]" />
                </div>
              )}
            </div>
            <span className="text-[11px] font-mono font-semibold text-[rgb(var(--foreground-muted))] uppercase tracking-wider">
              Alerts
            </span>
          </div>

          {/* Soft ambient glow behind icons without hard border/background container */}
          <div className="absolute inset-0 bg-[rgb(var(--accent))]/10 blur-2xl rounded-full pointer-events-none" />
        </div>

        {/* Title & Body */}
        <div className="space-y-2 max-w-sm pt-2">
          <h1 className="text-2xl sm:text-3xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
            {copy.title}
          </h1>
          <p className="text-sm text-[rgb(var(--foreground-muted))] leading-relaxed max-w-xs mx-auto">
            {copy.description}
          </p>
        </div>

        {/* Permission Item Descriptions (Clean list, zero borders, zero background boxes) */}
        <div className="w-full max-w-sm space-y-4 my-6 text-left">
          {/* Microphone Row */}
          <div className="flex items-start gap-3">
            <div className="mt-0.5 shrink-0">
              {micGranted ? (
                <Check className="w-4 h-4 text-[rgb(var(--accent))] stroke-[2.5]" />
              ) : (
                <Mic className="w-4 h-4 text-[rgb(var(--foreground-muted))] stroke-[1.75]" />
              )}
            </div>
            <div className="flex-1 min-w-0">
              <div className="flex items-center justify-between">
                <span className="text-xs font-semibold text-[rgb(var(--foreground))]">
                  {copy.micTitle}
                </span>
                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))] uppercase">
                  {micGranted ? 'Granted' : 'Required'}
                </span>
              </div>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-relaxed mt-0.5">
                {copy.micDesc}
              </p>
            </div>
          </div>

          {/* Notifications Row */}
          <div className="flex items-start gap-3">
            <div className="mt-0.5 shrink-0">
              {notifsGranted ? (
                <Check className="w-4 h-4 text-[rgb(var(--accent))] stroke-[2.5]" />
              ) : (
                <Bell className="w-4 h-4 text-[rgb(var(--foreground-muted))] stroke-[1.75]" />
              )}
            </div>
            <div className="flex-1 min-w-0">
              <div className="flex items-center justify-between">
                <span className="text-xs font-semibold text-[rgb(var(--foreground))]">
                  {copy.notifTitle}
                </span>
                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))] uppercase">
                  {notifsGranted ? 'Granted' : 'Optional'}
                </span>
              </div>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-relaxed mt-0.5">
                {copy.notifDesc}
              </p>
            </div>
          </div>
        </div>

        {/* Inline Grant Access Trigger if not fully granted */}
        {(!micGranted || !notifsGranted) && (
          <button
            type="button"
            onClick={handleGrantAccess}
            disabled={isRequesting}
            className="px-4 py-2 text-xs font-mono font-medium text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 rounded-xl transition-colors active:scale-95 cursor-pointer disabled:opacity-50"
          >
            {isRequesting ? 'Requesting Access...' : copy.grantButton}
          </button>
        )}
      </div>
    </div>
  );
};
