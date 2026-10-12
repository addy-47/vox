import React, { useState, useEffect, useCallback, useRef } from 'react';
import { Mic, Bell, Check } from 'lucide-react';
import {
  getRuntimeReport,
  getNotificationPermission,
  requestNotificationPermission,
  type NotificationPermission,
} from '@/services/setupService';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';

interface MobilePermissionStepProps {
  onNext?: () => void;
  onPermissionsChange?: (granted: boolean) => void;
  /**
   * Registers the permission-request action with the parent, which owns the
   * wizard's single footer button.
   *
   * The step used to render its own "Grant Access" button, but the footer button
   * for this step called `onNext` — so the visible primary action advanced the
   * wizard without ever asking for anything. Rather than duplicate the CTA, the
   * step hands its request handler up and the footer drives it.
   */
  onRegisterGrantAction?: (grant: () => void) => void;
  /** True while a system permission dialog is up, so the footer can lock itself. */
  onRequestingChange?: (requesting: boolean) => void;
}

export const MobilePermissionStep: React.FC<MobilePermissionStepProps> = ({
  onPermissionsChange,
  onRegisterGrantAction,
  onRequestingChange,
}) => {
  const [micGranted, setMicGranted] = useState<boolean | null>(null);
  const [notifState, setNotifState] = useState<NotificationPermission>('not_asked');
  const [isRequesting, setIsRequesting] = useState(false);

  const copy = MOBILE_ONBOARDING_COPY.permission;

  const notifsGranted = notifState === 'granted' || notifState === 'not_applicable';

  const isMountedRef = useRef(true);
  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
      onRequestingChange?.(false);
    };
  }, [onRequestingChange]);

  const checkStatus = useCallback(async () => {
    console.log('[PermStep] checkStatus:start');
    try {
      const report = await getRuntimeReport().catch(() => null);
      console.log('[PermStep] runtimeReport:', report ? `mic_access=${report.mic_access}` : 'NULL');
      if (report && isMountedRef.current) {
        setMicGranted(Boolean(report.mic_access));
      }
    } catch (e) {
      console.warn('[PermStep] runtimeReport threw:', e);
    }

    try {
      const state = await getNotificationPermission().catch(() => 'unavailable' as NotificationPermission);
      console.log('[PermStep] notifPermission:', state);
      if (isMountedRef.current) {
        setNotifState(state);
      }
    } catch (e) {
      console.warn('[PermStep] notifPermission threw:', e);
      if (isMountedRef.current) {
        setNotifState('unavailable');
      }
    }
    console.log('[PermStep] checkStatus:done');
  }, []);

  useEffect(() => {
    void checkStatus();
  }, [checkStatus]);

  useEffect(() => {
    // Microphone is required; notification denial or unavailability does not block onboarding
    const canProceed = Boolean(micGranted);
    onPermissionsChange?.(canProceed);
  }, [micGranted, onPermissionsChange]);

  const handleGrantAccess = async () => {
    if (isRequesting) return;
    console.log('[PermStep] grantAccess:enter isRequesting=', isRequesting, 'mic=', micGranted, 'notif=', notifState);
    setIsRequesting(true);
    try {
      // 1. Microphone request via WebRTC / Android wry WebChromeClient (bounded 6s timeout)
      if (navigator.mediaDevices && navigator.mediaDevices.getUserMedia && !micGranted) {
        try {
          const streamPromise = navigator.mediaDevices.getUserMedia({ audio: true });
          const timeoutPromise = new Promise<null>((resolve) => setTimeout(() => resolve(null), 6000));
          const stream = await Promise.race([streamPromise, timeoutPromise]);
          console.log('[PermStep] mic stream:', stream ? 'granted' : 'timeout/null');
          if (stream) {
            stream.getTracks().forEach((track) => track.stop());
            if (isMountedRef.current) setMicGranted(true);
          }
        } catch (e) {
          console.warn('[PermStep] mic threw:', e);
          if (isMountedRef.current) setMicGranted(false);
        }
      }

      // 2. Android native notification request via IPC (only if not already granted, bounded 6s timeout)
      console.log('[PermStep] notif branch: notifsGranted=', notifsGranted, 'state=', notifState);
      if (!notifsGranted) {
        try {
          const reqPromise = requestNotificationPermission();
          const timeoutPromise = new Promise<NotificationPermission>((resolve) =>
            setTimeout(() => resolve('unavailable'), 6000)
          );
          const state = await Promise.race([reqPromise, timeoutPromise]);
          console.log('[PermStep] notif request result:', state);
          if (isMountedRef.current) setNotifState(state);
        } catch (e) {
          console.warn('[PermStep] notif request threw:', e);
          if (isMountedRef.current) setNotifState('unavailable');
        }
      }

      await checkStatus();
      console.log('[PermStep] grantAccess:checkStatus returned');
    } catch (e) {
      console.warn('[PermStep] grantAccess outer catch:', e);
    } finally {
      console.log('[PermStep] grantAccess:finally mounted=', isMountedRef.current);
      if (isMountedRef.current) {
        setIsRequesting(false);
      }
    }
  };

  // Hand the request action up so the wizard footer can drive it. The ref
  // indirection keeps `handleGrantAccess` referentially stable across the
  // `checkStatus` identity changes that would otherwise re-run this every render.
  const grantRef = useRef(handleGrantAccess);
  grantRef.current = handleGrantAccess;
  useEffect(() => {
    onRegisterGrantAction?.(() => void grantRef.current());
  }, [onRegisterGrantAction]);

  // Lock the footer's primary button while the OS dialog is up. Without this a
  // second tap starts a second permission request behind the first.
  useEffect(() => {
    onRequestingChange?.(isRequesting);
  }, [isRequesting, onRequestingChange]);

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
          <div className="absolute inset-0 bg-[rgb(var(--accent))]/10 blur-md rounded-full pointer-events-none" />
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
                  {notifState === 'granted'
                    ? 'Granted'
                    : notifState === 'denied'
                    ? 'Denied'
                    : notifState === 'unavailable'
                    ? 'Unavailable'
                    : notifState === 'not_applicable'
                    ? 'N/A'
                    : 'Optional'}
                </span>
              </div>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-relaxed mt-0.5">
                {copy.notifDesc}
              </p>
            </div>
          </div>
        </div>

        {/* No inline CTA here on purpose. `MobileOnboardingFlow` renders a single
            footer button for this step and swaps its own label between
            "Grant Access" and "Continue" based on `onPermissionsChange`. A second
            button here gave the same action two competing call-to-actions on one
            screen, and the footer is the one that actually gates progression. */}
      </div>
    </div>
  );
};
