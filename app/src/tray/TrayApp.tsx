import React, { useState, useEffect, useMemo, useCallback, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Header } from "./components/Header";
import { TranscriptRenderer } from "./components/TranscriptRenderer";
import { Footer } from "./components/Footer";
import { useVisibility } from "@/shared/hooks/useVisibility";
import { useInteraction } from "@/shared/hooks/useInteraction";
import { useStreamingRenderer } from "@/shared/hooks/useStreamingRenderer";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSettings } from "@/shared/context/SettingsContext";
import { cn } from "@/shared/lib/utils";

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
    const separator = committedText && partialText ? " " : "";
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
    state: visibilityState, setIsHovered, show, startHold, hideImmediately 
  } = useVisibility({ 
    holdDuration: (settings?.ui.tray_hide_delay || 3) * 1000, 
    fadeDuration: settings?.ui.tray_fade_transition === 'Snappy' ? 500 : 1500 
  });

  const [interactionState, setInteractionState] = useState<string>("Idle");
  const [copied, setCopied] = useState(false);
  const [stats, setStats] = useState<SystemStats | null>(null);
  const [isSleeping, setIsSleeping] = useState(false);

  // ─── PTT & Status State ──────────────────────────────────────────────────────
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');

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
    visibilityState,
    interactionId,
    interactionState,
    history,
    historyLimit: settings?.ui.tray_history_limit || 10,
    callbacks: {
      startNewInteraction,
      updatePartial,
      commitFinal,
      endSpeechSegment,
      show,
      startHold,
      hideImmediately
    }
  });

  useEffect(() => {
    stateRef.current = { 
      pttStatus, 
      visibilityState, 
      interactionId, 
      interactionState, 
      history,
      historyLimit: settings?.ui.tray_history_limit || 10,
      callbacks: {
        startNewInteraction,
        updatePartial,
        commitFinal,
        endSpeechSegment,
        show,
        startHold,
        hideImmediately
      }
    };
  }, [pttStatus, visibilityState, interactionId, interactionState, history, settings?.ui.tray_history_limit, 
      startNewInteraction, updatePartial, commitFinal, endSpeechSegment, show, startHold, hideImmediately]);

  // ─── IPC Event Listeners ───────────────────────────────────────────────────
  useEffect(() => {
    let unlisteners: (() => void)[] = [];

    const setupListeners = async () => {
      try {
        const appWindow = getCurrentWindow();
        
        const u1 = await appWindow.listen("speech_start", () => {
          setViewingHistory(false);
          stateRef.current.callbacks.startNewInteraction();
          stateRef.current.callbacks.show();
        });

        const u2 = await appWindow.listen<{ text: string, session_id: number }>("transcript_partial", (event) => {
          if (stateRef.current.pttStatus === 'RECORDING') return;
          stateRef.current.callbacks.updatePartial(event.payload.text);
        });

        const u3 = await appWindow.listen<{ text: string, session_id: number }>("transcript_final", (event) => {
          if (event.payload.text) {
            stateRef.current.callbacks.commitFinal(event.payload.text);
            invoke<string[]>("get_transcript_history").then(h => {
              setHistory(h.slice(0, stateRef.current.historyLimit));
            });
          }
        });

        const u4 = await appWindow.listen("speech_end", () => {
          stateRef.current.callbacks.endSpeechSegment();
          stateRef.current.callbacks.startHold();
        });

        const u5 = await appWindow.listen<SystemStats>("system_stats", (event) => {
          setStats(event.payload);
        });

        const u6 = await appWindow.listen("toggle_hud", () => {
          if (stateRef.current.visibilityState === 'HIDDEN') stateRef.current.callbacks.show();
          else stateRef.current.callbacks.hideImmediately();
        });

        const u7 = await appWindow.listen<string>("state_changed", (event) => {
          setInteractionState(event.payload);
        });

        const u8 = await appWindow.listen<{ state: string }>("ptt_status", (event) => {
          setPttStatus(event.payload.state as any);
        });

        const u9 = await appWindow.listen<boolean>("auto_sleep_state", (event) => {
          setIsSleeping(event.payload);
        });

        unlisteners = [u1, u2, u3, u4, u5, u6, u7, u8, u9];
      } catch (err) {
        console.error("[TrayApp] Failed to setup listeners:", err);
      }
    };

    setupListeners();

    // Initial History Sync
    invoke<string[]>("get_transcript_history").then(h => {
      setHistory(h.slice(0, stateRef.current.historyLimit));
    });

    return () => {
      unlisteners.forEach(u => u());
    };
  }, []); // Stable Listeners

  // ─── Actions ───────────────────────────────────────────────────────────────
  const copyToClipboard = () => {
    if (currentTargetText) {
      navigator.clipboard.writeText(currentTargetText);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

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

  if (isLoading || !settings) return null;

  return (
    <div 
      className="tray-container w-full h-full select-none overflow-hidden relative flex flex-col"
    >
      <AnimatePresence>
        {visibilityState !== 'HIDDEN' && (
          <motion.div 
            variants={containerVariants}
            initial="HIDDEN"
            animate={visibilityState}
            exit="HIDDEN"
            transition={
              settings.ui.tray_fade_transition === 'Snappy' 
                ? { duration: 0.1, ease: "easeOut" }
                : settings.ui.tray_fade_transition === 'Smooth'
                ? { duration: 0.4, ease: "easeInOut" }
                : { type: "spring", damping: 20, stiffness: 100 }
            }
            className={cn(
              "w-[380px] h-[250px] flex flex-col liquid-glass overflow-hidden rounded-2xl transition-all duration-1000",
              isSleeping && "grayscale-[0.8] opacity-50"
            )}
            onMouseEnter={() => setIsHovered(true)}
            onMouseLeave={() => setIsHovered(false)}
            style={{ 
               // Dynamically apply blur from settings
               backdropFilter: `blur(${settings.ui.tray_blur_density}px) saturate(180%)`,
               WebkitBackdropFilter: `blur(${settings.ui.tray_blur_density}px) saturate(180%)`,
               backgroundColor: settings.ui.tray_glass_tint ? `rgba(var(--accent), 0.1)` : undefined,
            }}
          >
            <Header 
              isListening={interactionState === "Listening" || interactionState === "UserSpeaking" || pttStatus === 'RECORDING'} 
              hasContent={!!currentTargetText} 
              copied={copied} 
              isPttActive={pttStatus !== 'IDLE'}
              interactionMode={settings.interaction.tray_mode.toUpperCase()}
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
              historyCount={history.length}
            />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
