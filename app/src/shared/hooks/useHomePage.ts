import { useState, useEffect, useRef } from "react";
import { type InteractionState } from "@/services/eventsService";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import {
  useVoiceSession,
  type InteractionMode,
  type DialogueTurn,
} from "@/shared/context/VoiceSessionContext";

export type { InteractionState };

import {
  toMood,
  toStatusLabel,
  isDotActive,
  type AmbientMood,
} from "@/shared/lib/voiceDisplay";

export type { InteractionMode, DialogueTurn, AmbientMood };
export { toMood, toStatusLabel, isDotActive };

export function useHomePage() {
  const session = useVoiceSession();
  const [historyOpen, setHistoryOpen] = useState(false);
  const [isMobileScreen, setIsMobileScreen] = useState(
    typeof window !== "undefined" ? window.innerWidth < 768 : false
  );

  useEffect(() => {
    const checkMobile = () => setIsMobileScreen(window.innerWidth < 768);
    checkMobile();
    window.addEventListener("resize", checkMobile);
    return () => window.removeEventListener("resize", checkMobile);
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
    hasCachedSession,
    pttStatus,
    transcript,
    assistantText,
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

  // Auto-scroll the dialogue rail to newest content while the user stays
  // pinned near the bottom; never yanks away a manual scroll-back.
  useEffect(() => {
    const el = dialogueScrollRef.current;
    if (!el) return;
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    if (distanceFromBottom < 120) {
      el.scrollTop = el.scrollHeight;
    }
  }, [dialogueHistory, transcript, assistantText]);

  return {
    interactionState,
    interactionMode,
    pipelineMode,
    isEngaged,
    isSleeping,
    isPaused,
    hasCachedSession,
    pttStatus,
    transcript,
    assistantText,
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
    historyOpen,
    setHistoryOpen,
    telemetryRef,
    dialogueScrollRef,
    isMobileScreen,
  };
}
