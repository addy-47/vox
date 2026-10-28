import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { type InteractionState } from "@/services/eventsService";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import {
  useVoiceSession,
  type InteractionMode,
  type DialogueTurn,
} from "@/shared/context/VoiceSessionContext";

export type { InteractionState };

import {
  toStatusLabel,
  isDotActive,
  type AmbientMood,
} from "@/shared/lib/voiceDisplay";
import { useSessionStore } from "@/store/sessionStore";
import { useRemoteStore } from "@/store/remoteStore";
import { resolveActivityDisplay, resolveRemoteActivityDisplay, type ActivityDisplayConfig } from "@/shared/lib/activityRegistry";
import { useRemoteGuard } from "@/shared/hooks/useRemoteGuard";
import { isMobileWidth } from "@/layout/breakpoints";
import { installViewportResizeGate, subscribeViewportFrame } from "@/layout/viewportResize";

export type { InteractionMode, DialogueTurn, AmbientMood, ActivityDisplayConfig };
export { toStatusLabel, isDotActive, resolveActivityDisplay };

export function useHomePage() {
  const session = useVoiceSession();
  const [isMobileScreen, setIsMobileScreen] = useState(
    typeof window !== "undefined" ? isMobileWidth() : false
  );

  // Coalesced through the single viewport-resize gate (no private listener).
  useEffect(() => {
    let isMounted = true;
    installViewportResizeGate();
    const checkMobile = () => {
      if (isMounted) setIsMobileScreen(isMobileWidth());
    };
    checkMobile();
    const unsubscribe = subscribeViewportFrame(checkMobile);
    return () => {
      isMounted = false;
      unsubscribe();
    };
  }, []);

  const telemetryRef = useTelemetry();
  const dialogueScrollRef = useRef<HTMLDivElement>(null);

  const {
    interactionState,
    interactionMode,
    pipelineMode,
    isEngaged,
    isSleeping,
    isPaused,
    pttStatus,
    cpuWarning,
    isTemporarySession,
    isTextModeOpen,
    isPlaybackMuted,
    isMicMuted,
    dialogueHistory,
    isLaunching,
    isThinking,
    restoreError,
    dismissRestoreError,
    restoreSignal,
    engage,
    cancelLaunch,
    disengage,
    pause,
    resume,
    handlePttStart,
    handlePttStop,
    handlePttCancel,
    submitText,
    toggleTemporarySession,
    setTextModeOpen,
    togglePlaybackMute,
    toggleMicMute,
  } = session;

  const activeActivity = useSessionStore((s) => s.activeActivity);

  const { isBlocked: isRemoteBlocked, isHost } = useRemoteGuard();
  const linkState = useRemoteStore((s) => s.linkState);
  const remoteFailureReason = useRemoteStore(
    (s) => (s.linkState === "Failed" ? s.failureReason : null)
  );

  // Dismiss the failed-connect banner; resets when the link state moves so a
  // new failure surfaces again.
  const [dismissedForLink, setDismissedForLink] = useState<string | null>(null);
  const remoteFailureDismissed = dismissedForLink === linkState;


  // Status badge precedence: the host's single badge carries the remote link
  // state (Connected / Connecting / Reconnecting / Failed) directly from the
  // `remote_state_changed` stream. Everything else falls through to the local
  // interaction-state cascade.
  const activityDisplay = useMemo(() => {
    return (
      (isHost ? resolveRemoteActivityDisplay(linkState) : null) ??
      resolveActivityDisplay(interactionState, activeActivity, isEngaged, isSleeping, isPaused)
    );
  }, [isHost, linkState, interactionState, activeActivity, isEngaged, isSleeping, isPaused]);

  const shouldAutoScrollRef = useRef(true);

  const handleDialogueScroll = useCallback(() => {
    const el = dialogueScrollRef.current;
    if (!el) return;
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    // Pinned if within 40px of bottom
    shouldAutoScrollRef.current = distanceFromBottom <= 40;
  }, []);

  // When submitting text, immediately re-pin auto-scroll to bottom
  const handleSubmitText = useCallback(async (text: string) => {
    shouldAutoScrollRef.current = true;
    await submitText(text);
    requestAnimationFrame(() => {
      const el = dialogueScrollRef.current;
      if (el) {
        el.scrollTop = el.scrollHeight;
      }
    });
  }, [submitText]);

  // Auto-scroll the dialogue rail whenever turns update
  useEffect(() => {
    const el = dialogueScrollRef.current;
    if (!el || !shouldAutoScrollRef.current) return;

    requestAnimationFrame(() => {
      if (dialogueScrollRef.current && shouldAutoScrollRef.current) {
        dialogueScrollRef.current.scrollTop = dialogueScrollRef.current.scrollHeight;
      }
    });
  }, [dialogueHistory]);

  return {
    interactionState,
    interactionMode,
    pipelineMode,
    isEngaged,
    isSleeping,
    isPaused,
    pttStatus,
    cpuWarning,
    isTemporarySession,
    isTextModeOpen,
    isPlaybackMuted,
    isMicMuted,
    dialogueHistory,
    isLaunching,
    isThinking,
    restoreError,
    dismissRestoreError,
    restoreSignal,
    engage,
    cancelLaunch,
    disengage,
    pause,
    resume,
    handlePttStart,
    handlePttStop,
    handlePttCancel,
    submitText: handleSubmitText,
    toggleTemporarySession,
    setTextModeOpen,
    togglePlaybackMute,
    toggleMicMute,
    telemetryRef,
    dialogueScrollRef,
    handleDialogueScroll,
    shouldAutoScrollRef,
    isMobileScreen,
    isHost,
    activityDisplay,
    activeActivity,
    isRemoteBlocked,
    remoteFailureReason: remoteFailureDismissed ? null : remoteFailureReason,
    dismissRemoteFailure: () => {
      setDismissedForLink(linkState);
      useRemoteStore.getState().dismissFailure();
    },
  };
}
