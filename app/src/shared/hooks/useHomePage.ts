import { useState, useEffect, useRef, useCallback } from "react";
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

  // Auto-scroll the dialogue rail whenever turns, transcript, or assistant text updates
  useEffect(() => {
    const el = dialogueScrollRef.current;
    if (!el || !shouldAutoScrollRef.current) return;

    requestAnimationFrame(() => {
      if (dialogueScrollRef.current && shouldAutoScrollRef.current) {
        dialogueScrollRef.current.scrollTop = dialogueScrollRef.current.scrollHeight;
      }
    });
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
    submitText: handleSubmitText,
    toggleTemporarySession,
    setTextModeOpen,
    togglePlaybackMute,
    toggleMicMute,
    historyOpen,
    setHistoryOpen,
    telemetryRef,
    dialogueScrollRef,
    handleDialogueScroll,
    shouldAutoScrollRef,
    isMobileScreen,
  };
}
