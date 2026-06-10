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
  const [isSleeping, setIsSleeping] = useState(false);

  // ─── PTT & Status State ──────────────────────────────────────────────────────
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');

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
    historyLimit: settings?.ui.tray_history_limit || 5,
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
      historyLimit: settings?.ui.tray_history_limit || 5,
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
  }, [pttStatus, visibilityState, interactionId, interactionState, history, settings?.ui.tray_history_limit, 
      liveTargetText, startNewInteraction, updatePartial, commitFinal, endSpeechSegment, show, startFade, cancelFade, hideImmediately, reset]);

  // ─── Actions ───────────────────────────────────────────────────────────────
  const copyToClipboard = () => {
    if (currentTargetText) {
      navigator.clipboard.writeText(currentTargetText);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const handleClose = useCallback(() => {
    const textToCommit = stateRef.current.liveTargetText;
    if (textToCommit.trim()) {
      invoke<string[]>("commit_session_to_history", { text: textToCommit }).then(h => {
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
        invoke("ptt_start", { owner: "Tray" });
      } else {
        invoke("ptt_stop", { owner: "Tray" });
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
        
        const u1 = await appWindow.listen("speech_start", () => {
          if (!active) return;
          setViewingHistory(false);
          stateRef.current.callbacks.startNewInteraction();
        });
        if (!active) { u1(); return; }
        localUnlisteners.push(u1);

        const u2 = await appWindow.listen<{ text: string, session_id: number }>("transcript_partial", (event) => {
          if (!active) return;
          if (stateRef.current.pttStatus === 'RECORDING') return;
          if (event.payload.text) {
            if (stateRef.current.visibilityState === 'HIDDEN') {
              stateRef.current.callbacks.show();
            }
            stateRef.current.callbacks.updatePartial(event.payload.text);
          }
        });
        if (!active) { u2(); return; }
        localUnlisteners.push(u2);

        const u3 = await appWindow.listen<{ text: string, session_id: number }>("transcript_final", (event) => {
          if (!active) return;
          if (event.payload.text) {
            if (stateRef.current.visibilityState === 'HIDDEN') {
              stateRef.current.callbacks.show();
            }
            stateRef.current.callbacks.commitFinal(event.payload.text);
          }
        });
        if (!active) { u3(); return; }
        localUnlisteners.push(u3);

        const u4 = await appWindow.listen("speech_end", () => {
          if (!active) return;
          stateRef.current.callbacks.endSpeechSegment();
        });
        if (!active) { u4(); return; }
        localUnlisteners.push(u4);

        const u5 = await appWindow.listen<SystemStats>("system_stats", (event) => {
          if (!active) return;
          setStats(event.payload);
        });
        if (!active) { u5(); return; }
        localUnlisteners.push(u5);

        const u6 = await appWindow.listen("toggle_hud", () => {
          if (!active) return;
          if (stateRef.current.visibilityState === 'HIDDEN') stateRef.current.callbacks.show();
          else stateRef.current.callbacks.hideImmediately();
        });
        if (!active) { u6(); return; }
        localUnlisteners.push(u6);

        const u7 = await appWindow.listen<string>("state_changed", (event) => {
          if (!active) return;
          setInteractionState(event.payload);
        });
        if (!active) { u7(); return; }
        localUnlisteners.push(u7);

        const u8 = await appWindow.listen<{ state: string }>("ptt_status", (event) => {
          if (!active) return;
          setPttStatus(event.payload.state as any);
        });
        if (!active) { u8(); return; }
        localUnlisteners.push(u8);

        const u9 = await appWindow.listen<boolean>("auto_sleep_state", (event) => {
          if (!active) return;
          const sleep = event.payload;
          setIsSleeping(sleep);
          if (sleep) {
            // Auto-sleep: Commit current session & hide HUD
            const textToCommit = stateRef.current.liveTargetText;
            if (textToCommit.trim()) {
              invoke<string[]>("commit_session_to_history", { text: textToCommit }).then(h => {
                if (active) setHistory(h.slice(0, stateRef.current.historyLimit));
              });
            }
            stateRef.current.callbacks.reset();
            stateRef.current.callbacks.hideImmediately();
          } else {
            stateRef.current.callbacks.cancelFade();
          }
        });
        if (!active) { u9(); return; }
        localUnlisteners.push(u9);

      } catch (err) {
        console.error("[TrayApp] Failed to setup listeners:", err);
      }
    };

    setupListeners();

    // Initial History Sync
    invoke<string[]>("get_transcript_history").then(h => {
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
            variants={containerVariants}
            initial="HIDDEN"
            animate={visibilityState}
            exit="HIDDEN"
            transition={{ duration: 0.15, ease: "easeOut" }}
            className={cn(
              "w-[380px] h-[250px] flex flex-col glass-elevated glass-base overflow-hidden rounded-2xl transition-all duration-1000",
              isSleeping && "grayscale-[0.8] opacity-50"
            )}
            onMouseEnter={() => setIsHovered(true)}
            onMouseLeave={() => setIsHovered(false)}
            style={{ 
               backdropFilter: `blur(${settings.ui.tray_blur_density}px) saturate(180%)`,
               WebkitBackdropFilter: `blur(${settings.ui.tray_blur_density}px) saturate(180%)`,
            }}
          >
            <Header 
              isListening={interactionState === "Listening" || interactionState === "UserSpeaking" || pttStatus === 'RECORDING'} 
              hasContent={!!currentTargetText} 
              copied={copied} 
              isPttActive={pttStatus !== 'IDLE'}
              interactionMode={settings.interaction.tray_mode.toUpperCase()}
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
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
