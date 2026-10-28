import { useRemoteStore } from "@/store/remoteStore";
import { isRealMobileDevice } from "@/layout/breakpoints";

export interface RemoteGuardResult {
  isBlocked: boolean;
  isHost: boolean;
  tooltip?: string;
  guardProps: {
    disabled?: boolean;
    "aria-disabled"?: boolean;
    title?: string;
    className?: string;
  };
}

/**
 * Evaluates whether the current instance is Desktop Host in an active remote session.
 * During an active session, Host is a passive observer (RULE-RC-GUARD) and local
 * mutating controls (like Engage) are disabled.
 */
export function useRemoteGuard(): RemoteGuardResult {
  const linkState = useRemoteStore((s) => s.linkState);
  const isHost = !isRealMobileDevice();
  const isBlocked = isHost && linkState === "Connected";

  const tooltip = isBlocked ? "Remote session active" : undefined;

  return {
    isBlocked,
    isHost,
    tooltip,
    guardProps: isBlocked
      ? {
          disabled: true,
          "aria-disabled": true,
          title: tooltip,
          className: "opacity-40 cursor-not-allowed pointer-events-none",
        }
      : {},
  };
}
