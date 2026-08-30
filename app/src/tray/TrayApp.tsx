import React, { useState, useEffect, useMemo, useCallback, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Header } from "./components/Header";
import { TranscriptRenderer } from "./components/TranscriptRenderer";
import { Footer } from "./components/Footer";
import { useVisibility } from "@/shared/hooks/useVisibility";
import { useInteraction } from "@/shared/hooks/useInteraction";
import { useStreamingRenderer } from "@/shared/hooks/useStreamingRenderer";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import { hideTrayWindow, syncHudVisibility, setHudIgnoreCursor } from "@/services/windowService";
import { pttStart, pttStop } from "@/services/pipelineService";
import { commitSessionToHistory, getTranscriptHistory } from "@/services/historyService";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSettings } from "@/shared/hooks/useSettings";
import { ErrorBoundary } from "@/shared/components/common";

interface SystemStats {
  system_cpu: number;
  system_ram_pct: number;
  vox_cpu: number;
  vox_ram_mb: number;
  threads: number;
}

export const TrayApp: React.FC = () => {
  const { settings, isLoading } = useSettings();
  
  // ─── History System (Backend Backed) ──────────────────────────────────────
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState<number>(-1);
  const [viewingHistory, setViewingHistory] = useState(false);

  // ─── Interaction & Text State ──────────────────────────────────────────────
  const { 
    interactionId, committedText, partialText, 
    startNewInteraction, endSpeechSegment, updatePartial, commitFinal, reset 
  } = useInteraction();
  const telemetryRef = useTelemetry();
  
  const liveTargetText = useMemo(() => {
    const separator = committedText && partialText ? "\n" : "";
    return committedText + separator + partialText;
  }, [committedText, partialText]);

  const currentTargetText = useMemo(() => {
    if (viewingHistory && historyIndex >= 0) {
      return history[historyIndex] || "";
    }
    return liveTargetText;
  }, [viewingHistory, historyIndex, history, liveTargetText]);

  const displayText = useStreamingRenderer(currentTargetText);
  
  // ─── Visibility & UX State ────────────────────────────────────────────────
  const { 
    state: visibilityState, setIsHovered, show, startFade, cancelFade, hideImmediately 
  } = useVisibility();

  const [interactionState, setInteractionState] = useState<string>("Idle");
  const [copied, setCopied] = useState(false);
  const [stats, setStats] = useState<SystemStats | null>(null);

  // ─── PTT & Status State ──────────────────────────────────────────────────────
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');

  // Sync React state to OS Window and Backend state
  useEffect(() => {
    const syncVisibility = async () => {
      try {
        if (visibilityState === 'HIDDEN') {
          hideTrayWindow();
          syncHudVisibility(false);
          setHudIgnoreCursor(true);
          reset();
          setInteractionState("Idle");
          setPttStatus("IDLE");
          setCopied(false);
          setHistoryIndex(-1);
          setViewingHistory(false);
        } else if (visibilityState === 'ACTIVE' || visibilityState === 'APPEARING') {
          syncHudVisibility(true);
          setHudIgnoreCursor(false);
        } else if (visibilityState === 'FADING') {
          setHudIgnoreCursor(true);
        }
      } catch (e) {
        console.warn("[TrayApp] Failed to sync visibility:", e);
      }
    };
    syncVisibility();
  }, [visibilityState, reset]);

  // ─── Stable Refs for Listeners ───────────────────────────────────────────
  const historyLimit = settings?.history?.tray_history_limit || 5;

  const stateRef = useRef({
    pttStatus,
    visibilityState,
    interactionId,
    interactionState,
    history,
    historyLimit,
    liveTargetText,
    callbacks: {
      startNewInteraction,
      updatePartial,
      commitFinal,
      endSpeechSegment,
      show,
      startFade,
      cancelFade,
      hideImmediately,
      reset
    }
  });

  useEffect(() => {
    stateRef.current = { 
      pttStatus, 
      visibilityState, 
      interactionId, 
      interactionState, 
      history,
      historyLimit,
      liveTargetText,
      callbacks: {
        startNewInteraction,
        updatePartial,
        commitFinal,
        endSpeechSegment,
        show,
        startFade,
        cancelFade,
        hideImmediately,
        reset
      }
    };
  }, [pttStatus, visibilityState, interactionId, interactionState, history, historyLimit, 
      liveTargetText, startNewInteraction, updatePartial, commitFinal, endSpeechSegment, show, startFade, cancelFade, hideImmediately, reset]);

  // ─── Actions ───────────────────────────────────────────────────────────────
  const copyToClipboard = async () => {
    const textToCopy = currentTargetText;
    if (!textToCopy) return;
    try {
      await navigator.clipboard.writeText(textToCopy);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("[TrayApp] Failed to copy text: ", err);
    }
  };

  const handleClose = useCallback(() => {
    const textToCommit = stateRef.current.liveTargetText;
    if (textToCommit.trim()) {
      commitSessionToHistory(textToCommit).then((h: string[]) => {
        setHistory(h.slice(0, stateRef.current.historyLimit));
      });
    }
    stateRef.current.callbacks.reset();
    setHistoryIndex(-1);
    setViewingHistory(false);
    stateRef.current.callbacks.hideImmediately();
  }, []);

  const handlePrev = useCallback(() => {
    if (history.length === 0) return;
    setViewingHistory(true);
    setHistoryIndex(prev => (prev === -1 ? history.length - 1 : Math.max(0, prev - 1)));
  }, [history]);

  const handleNext = useCallback(() => {
    setHistoryIndex(prev => {
      if (prev === -1 || prev >= history.length - 1) {
        setViewingHistory(false);
        return -1;
      }
      return prev + 1;
    });
  }, [history]);

  const togglePtt = async () => {
    try {
      if (pttStatus === 'IDLE') {
        pttStart();
      } else {
        pttStop();
      }
    } catch (e) {
      console.error("[TrayApp] Failed to toggle PTT:", e);
    }
  };

  // ─── IPC Event Listeners ───────────────────────────────────────────────────
  useEffect(() => {
    let active = true;
    let localUnlisteners: (() => void)[] = [];

    const setupListeners = async () => {
      try {
        const appWindow = getCurrentWindow();

        const u1 = await appWindow.listen<{ state: string; turn_id: number }>("dictation_state_changed", (event: { payload: { state: string; turn_id: number } }) => {
          if (!active) return;
          const newState = event.payload.state;
          if (newState === "RECORDING") {
            setPttStatus("RECORDING");
            stateRef.current.callbacks.reset();
            setViewingHistory(false);
            if (stateRef.current.visibilityState === 'HIDDEN') {
              stateRef.current.callbacks.show();
            }
          } else if (newState === "PROCESSING") {
            setPttStatus("PROCESSING");
          } else {
            setPttStatus("IDLE");
          }
        });
        localUnlisteners.push(u1);
        if (!active) {
          localUnlisteners.forEach(u => u());
          return;
        }

        const u2 = await appWindow.listen<{ text: string; turn_id?: number }>("transcript_partial", (event: { payload: { text: string; turn_id?: number } }) => {
          if (!active) return;
          if (event.payload.text) {
            if (stateRef.current.visibilityState === 'HIDDEN') {
              stateRef.current.callbacks.show();
            }
            stateRef.current.callbacks.updatePartial(event.payload.text);
          }
        });
        localUnlisteners.push(u2);
        if (!active) {
          localUnlisteners.forEach(u => u());
          return;
        }

        const u3 = await appWindow.listen<{ text: string; turn_id?: number }>("transcript_final", (event: { payload: { text: string; turn_id?: number } }) => {
          if (!active) return;
          if (event.payload.text) {
            if (stateRef.current.visibilityState === 'HIDDEN') {
              stateRef.current.callbacks.show();
            }
            stateRef.current.callbacks.commitFinal(event.payload.text);
          }
        });
        localUnlisteners.push(u3);
        if (!active) {
          localUnlisteners.forEach(u => u());
          return;
        }

        const u5 = await appWindow.listen<SystemStats>("system_stats", (event: { payload: SystemStats }) => {
          if (!active) return;
          if (stateRef.current.visibilityState === 'HIDDEN') return;
          setStats(event.payload);
        });
        localUnlisteners.push(u5);
        if (!active) {
          localUnlisteners.forEach(u => u());
          return;
        }

        const u6 = await appWindow.listen("toggle_hud", () => {
          if (!active) return;
          if (stateRef.current.visibilityState === 'HIDDEN') stateRef.current.callbacks.show();
          else stateRef.current.callbacks.hideImmediately();
        });
        localUnlisteners.push(u6);
        if (!active) {
          localUnlisteners.forEach(u => u());
          return;
        }

        const u7 = await appWindow.listen<string>("state_changed", (event: { payload: string }) => {
          if (!active) return;
          if (stateRef.current.visibilityState === 'HIDDEN') return;
          setInteractionState(event.payload);
        });
        localUnlisteners.push(u7);
        if (!active) {
          localUnlisteners.forEach(u => u());
          return;
        }

      } catch (err) {
        console.error("[TrayApp] Failed to setup listeners:", err);
        if (!active) {
          localUnlisteners.forEach(u => u());
        }
      }
    };

    setupListeners();

    // Initial History Sync
    getTranscriptHistory().then((h: string[]) => {
      if (active) setHistory(h.slice(0, stateRef.current.historyLimit));
    });

    return () => {
      active = false;
      localUnlisteners.forEach(u => u());
    };
  }, []); // Stable Listeners

  const containerVariants = {
    HIDDEN: { opacity: 0, x: 20, scale: 0.98, pointerEvents: "none" as const },
    APPEARING: { opacity: 1, x: 0, scale: 1 },
    ACTIVE: { opacity: 1, x: 0, scale: 1, pointerEvents: "auto" as const },
    FADING: { opacity: 0, x: 10, scale: 0.99, transition: { duration: 0.5 }, pointerEvents: "none" as const }
  };

  if (isLoading || !settings) return null;

  return (
    <div 
      className="tray-container w-full h-full select-none overflow-hidden relative flex flex-col"
    >
      <AnimatePresence>
        {visibilityState !== 'HIDDEN' && (
          <motion.div 
            key="tray-card"
            variants={containerVariants}
            initial="HIDDEN"
            animate={visibilityState}
            exit="HIDDEN"
            transition={{ duration: 0.15, ease: "easeOut" }}
            className="w-[380px] h-[250px] flex flex-col glass-card overflow-hidden rounded-2xl transition-all duration-1000"
            onMouseEnter={() => setIsHovered(true)}
            onMouseLeave={() => setIsHovered(false)}
            style={{ 
               backdropFilter: `blur(20px) saturate(180%)`,
               WebkitBackdropFilter: `blur(20px) saturate(180%)`,
               backgroundColor: `rgba(var(--card), 0.88)`,
            }}
          >
            <ErrorBoundary name="TrayAppContent">
              <Header 
                isListening={interactionState === "Listening" || pttStatus === 'RECORDING'} 
                hasContent={!!currentTargetText} 
                copied={copied} 
                isPttActive={pttStatus !== 'IDLE'}
                interactionMode={String(settings.dictation?.interaction_mode || "ptt").toUpperCase()}
                onCopy={copyToClipboard} 
                onClose={handleClose}
                onTogglePtt={togglePtt}
              />

              <div className="flex-1 flex flex-col relative overflow-hidden group">
                <TranscriptRenderer 
                  displayText={displayText} 
                  interactionState={interactionState}
                  pttStatus={pttStatus}
                  telemetryRef={telemetryRef}
                />
              </div>

              <Footer 
                stats={stats} 
                onPrev={handlePrev}
                onNext={handleNext}
                historyIndex={historyIndex}
                viewingHistory={viewingHistory}
                historyCount={history.length}
              />
            </ErrorBoundary>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
