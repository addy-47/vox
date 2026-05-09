import React, { useState, useEffect, useMemo, useCallback, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Header } from "./components/Header";
import { TranscriptRenderer } from "./components/TranscriptRenderer";
import { Footer } from "./components/Footer";
import { useVisibility } from "@/shared/hooks/useVisibility";
import { useInteraction } from "@/shared/hooks/useInteraction";
import { useStreamingRenderer } from "@/shared/hooks/useStreamingRenderer";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import { CircularBuffer } from "./utils/CircularBuffer";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useTheme } from "@/shared/context/ThemeContext";

interface SystemStats {
  cpu_usage: number;
  memory_used_mb: number;
}

export const TrayApp: React.FC = () => {
  // ─── History System ────────────────────────────────────────────────────────
  const history = useMemo(() => new CircularBuffer<string>(10), []);
  const [historyIndex, setHistoryIndex] = useState<number>(-1);
  const [viewingHistory, setViewingHistory] = useState(false);

  // ─── Interaction & Text State ──────────────────────────────────────────────
  const { 
    interactionId, committedText, partialText, 
    startNewInteraction, endSpeechSegment, updatePartial, commitFinal, reset 
  } = useInteraction();
  const telemetryRef = useTelemetry();
  useTheme();
  
  const liveTargetText = useMemo(() => {
    const separator = committedText && partialText ? " " : "";
    return committedText + separator + partialText;
  }, [committedText, partialText]);

  const currentTargetText = useMemo(() => {
    if (viewingHistory && historyIndex >= 0) {
      const allHistory = history.getAll();
      return allHistory[historyIndex] || "";
    }
    return liveTargetText;
  }, [viewingHistory, historyIndex, history, liveTargetText]);

  const displayText = useStreamingRenderer(currentTargetText);
  
  // ─── Visibility & UX State ────────────────────────────────────────────────
  const { 
    state: visibilityState, setIsHovered, show, startHold, hideImmediately 
  } = useVisibility({ holdDuration: 3000, fadeDuration: 2000 });

  const [interactionState, setInteractionState] = useState<string>("Idle");
  const [copied, setCopied] = useState(false);
  const [stats, setStats] = useState<SystemStats | null>(null);

  // ─── PTT & Mode State ──────────────────────────────────────────────────────
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');
  const [interactionMode, setInteractionMode] = useState<string>('PASSIVE');

  // Bug 5a: Reset stale transcript state when tray hides
  useEffect(() => {
    if (visibilityState === 'HIDDEN') {
      reset();
      setPttStatus('IDLE');
      setInteractionState('Idle');
    }
  }, [visibilityState, reset]);

  // Sync React state to OS Window and Backend state
  useEffect(() => {
    const syncVisibility = async () => {
      try {
        if (visibilityState === 'HIDDEN') {
          invoke("hide_tray_window");
          invoke("sync_hud_visibility", { visible: false });
          invoke("set_hud_ignore_cursor", { ignore: true });
        } else if (visibilityState === 'ACTIVE' || visibilityState === 'APPEARING') {
          invoke("sync_hud_visibility", { visible: true });
          invoke("set_hud_ignore_cursor", { ignore: false });
        } else if (visibilityState === 'FADING') {
          invoke("set_hud_ignore_cursor", { ignore: true });
        }
      } catch (e) {
        console.warn("[TrayApp] Failed to sync visibility:", e);
      }
    };
    syncVisibility();
  }, [visibilityState]);

  // ─── Stable Refs for Listeners ───────────────────────────────────────────
  const stateRef = useRef({
    pttStatus,
    interactionMode,
    visibilityState,
    interactionId,
    interactionState
  });

  useEffect(() => {
    stateRef.current = { pttStatus, interactionMode, visibilityState, interactionId, interactionState };
  }, [pttStatus, interactionMode, visibilityState, interactionId, interactionState]);

  // ─── IPC Event Listeners ───────────────────────────────────────────────────
  useEffect(() => {
    let unlisteners: (() => void)[] = [];

    const setupListeners = async () => {
      try {
        const appWindow = getCurrentWindow();
        
        const u1 = await appWindow.listen("speech_start", () => {
          setViewingHistory(false);
          startNewInteraction();
          show();
        });

        const u2 = await appWindow.listen<{ text: string, session_id: number }>("transcript_partial", (event) => {
          if (stateRef.current.pttStatus === 'RECORDING') return;
          updatePartial(event.payload.text);
        });

        const u3 = await appWindow.listen<{ text: string, session_id: number }>("transcript_final", (event) => {
          if (event.payload.text) {
            commitFinal(event.payload.text);
            history.push(event.payload.text);
          }
        });

        const u4 = await appWindow.listen("speech_end", () => {
          endSpeechSegment();
          startHold();
        });

        const u5 = await appWindow.listen<SystemStats>("system_stats", (event) => {
          setStats(event.payload);
        });

        const u6 = await appWindow.listen("toggle_hud", () => {
          if (stateRef.current.visibilityState === 'HIDDEN') show();
          else hideImmediately();
        });

        const u7 = await appWindow.listen<string>("state_changed", (event) => {
          setInteractionState(event.payload);
        });

        const u8 = await appWindow.listen<{ state: string }>("ptt_status", (event) => {
          setPttStatus(event.payload.state as any);
        });

        const u9 = await appWindow.listen<string>("mode_changed_tray", (event) => {
          setInteractionMode(event.payload.toUpperCase());
        });

        unlisteners = [u1, u2, u3, u4, u5, u6, u7, u8, u9];
      } catch (err) {
        console.error("[TrayApp] Failed to setup listeners:", err);
      }
    };

    setupListeners();

    // Initial Interaction Mode setup (sync with backend)
    const fetchSettings = async () => {
      try {
        const settings = await invoke<any>("get_settings");
        if (settings?.tray_mode) {
          setInteractionMode(settings.tray_mode.toUpperCase());
        }
      } catch (e) {
        console.warn("[TrayApp] Failed to fetch interaction mode:", e);
      }
    };
    fetchSettings();

    return () => {
      unlisteners.forEach(u => u());
    };
  }, [startNewInteraction, updatePartial, commitFinal, show, endSpeechSegment, startHold, history, hideImmediately]);

  // ─── Actions ───────────────────────────────────────────────────────────────
  const copyToClipboard = () => {
    if (currentTargetText) {
      navigator.clipboard.writeText(currentTargetText);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const handlePrev = useCallback(() => {
    const all = history.getAll();
    if (all.length === 0) return;
    setViewingHistory(true);
    setHistoryIndex(prev => (prev === -1 ? all.length - 1 : Math.max(0, prev - 1)));
  }, [history]);

  const handleNext = useCallback(() => {
    const all = history.getAll();
    setHistoryIndex(prev => {
      if (prev === -1 || prev >= all.length - 1) {
        setViewingHistory(false);
        return -1;
      }
      return prev + 1;
    });
  }, [history]);

  const togglePtt = async () => {
    try {
      if (pttStatus === 'IDLE') {
        invoke("ptt_start", { owner: "Tray" });
      } else {
        invoke("ptt_stop", { owner: "Tray" });
      }
    } catch (e) {
      console.error("[TrayApp] Failed to toggle PTT:", e);
    }
  };

  const containerVariants = {
    HIDDEN: { opacity: 0, x: 20, scale: 0.98, pointerEvents: "none" as const },
    APPEARING: { opacity: 1, x: 0, scale: 1 },
    ACTIVE: { opacity: 1, x: 0, scale: 1, pointerEvents: "auto" as const },
    HOLD: { opacity: 1, x: 0, scale: 1, pointerEvents: "auto" as const },
    FADING: { opacity: 0, x: 10, scale: 0.99, transition: { duration: 2 }, pointerEvents: "none" as const }
  };

  return (
    <div 
      className="tray-container w-full h-full select-none overflow-hidden relative flex flex-col"
    >
      <AnimatePresence>
        {visibilityState !== 'HIDDEN' && (
          <motion.div 
            key={`hud-${interactionId}`}
            variants={containerVariants}
            initial="HIDDEN"
            animate={visibilityState}
            exit="HIDDEN"
            className="w-[380px] h-[250px] flex flex-col liquid-glass overflow-hidden rounded-2xl"
            onMouseEnter={() => setIsHovered(true)}
            onMouseLeave={() => setIsHovered(false)}
          >
            <Header 
              isListening={interactionState === "Listening" || interactionState === "UserSpeaking" || pttStatus === 'RECORDING'} 
              hasContent={!!currentTargetText} 
              copied={copied} 
              isPttActive={pttStatus !== 'IDLE'}
              interactionMode={interactionMode}
              onCopy={copyToClipboard} 
              onClose={hideImmediately}
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
              historyCount={history.getAll().length}
            />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};

