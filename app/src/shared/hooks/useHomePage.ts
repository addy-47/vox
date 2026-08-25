import { useState, useEffect, useRef } from "react";
import { type InteractionState } from "@/services/eventsService";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import {
  useVoiceSession,
  type InteractionMode,
  type DialogueTurn,
} from "@/shared/context/VoiceSessionContext";

export type { InteractionMode, DialogueTurn };
export type AmbientMood =
  | "Dormant"
  | "Idle"
  | "Listening"
  | "Thinking"
  | "Speaking"
  | "Paused"
  | "Error";

export function toMood(state: InteractionState, isEngaged: boolean): AmbientMood {
  if (!isEngaged) return "Dormant";
  switch (state) {
    case "Idle":
      return "Idle";
    case "Listening":
    case "UserSpeaking":
      return "Listening";
    case "Thinking":
      return "Thinking";
    case "AssistantSpeaking":
      return "Speaking";
    case "Paused":
      return "Paused";
    case "Error":
      return "Error";
    default:
      return "Dormant";
  }
}

export function toStatusLabel(
  state: InteractionState,
  engaged: boolean,
  sleeping: boolean,
  ptt: "IDLE" | "RECORDING" | "PROCESSING",
  isPaused: boolean,
  idleTimeout: number | null
): string {
  if (!engaged) return "Dormant";
  if (isPaused) return "Paused";
  if (idleTimeout !== null) return `Idle · ${idleTimeout}s`;
  if (sleeping) return "Sleeping";
  if (ptt === "RECORDING") return "Recording";
  if (ptt === "PROCESSING") return "Processing";
  switch (state) {
    case "Idle":
      return "Ready";
    case "Listening":
      return "Ready";
    case "UserSpeaking":
      return "Listening";
    case "Thinking":
      return "Thinking";
    case "AssistantSpeaking":
      return "Speaking";
    case "Paused":
      return "Paused";
    case "Error":
      return "Error";
    default:
      return "Ready";
  }
}

export function isDotActive(
  engaged: boolean,
  state: InteractionState,
  ptt: "IDLE" | "RECORDING" | "PROCESSING",
  sleeping: boolean
): boolean {
  if (!engaged || sleeping) return false;
  if (ptt === "RECORDING" || ptt === "PROCESSING") return true;
  return state === "UserSpeaking" || state === "Thinking" || state === "AssistantSpeaking";
}

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
  const testButtonRef = useRef<HTMLButtonElement>(null);
  const testPanelRef = useRef<HTMLDivElement>(null);

  return {
    ...session,
    historyOpen,
    setHistoryOpen,
    telemetryRef,
    dialogueScrollRef,
    isMobileScreen,
    testButtonRef,
    testPanelRef,
  };
}
