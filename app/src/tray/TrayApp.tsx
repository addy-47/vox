import React, { useState, useEffect, useMemo, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import { Header } from "./components/Header";
import { TranscriptRenderer } from "./components/TranscriptRenderer";
import { Footer } from "./components/Footer";
import { useInteraction } from "./hooks/useInteraction";
import { useVisibility } from "./hooks/useVisibility";
import { useStreamingRenderer } from "./hooks/useStreamingRenderer";
import { CircularBuffer } from "./utils/CircularBuffer";

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
    startNewInteraction, endSpeechSegment, updatePartial, commitFinal 
  } = useInteraction();
  
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

  const [isListening, setIsListening] = useState(false);
  const [copied, setCopied] = useState(false);
  const [stats, setStats] = useState<SystemStats | null>(null);

  // ─── PTT & Mode State ──────────────────────────────────────────────────────
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');
  const [amplitudeBuffer, setAmplitudeBuffer] = useState<number[]>(new Array(40).fill(0));

  // Sync React state to OS Window and Backend state
  useEffect(() => {
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
  }, [visibilityState]);

  // ─── IPC Event Listeners ───────────────────────────────────────────────────
  useEffect(() => {
    let isMounted = true;
    let unlisteners: (() => void)[] = [];

    const setupListeners = async () => {
      const appWindow = getCurrentWindow();
      
      const u1 = await appWindow.listen("speech_start", () => {
        if (!isMounted) return;
        setIsListening(true);
        setViewingHistory(false);
        startNewInteraction();
        show();
      });

      const u2 = await appWindow.listen<{ text: string, session_id: number }>("transcript_partial", (event) => {
        if (!isMounted) return;
        updatePartial(event.payload.text);
      });

      const u3 = await appWindow.listen<{ text: string, session_id: number }>("transcript_final", (event) => {
        if (!isMounted) return;
        if (event.payload.text) {
          commitFinal(event.payload.text);
          history.push(event.payload.text);
        }
      });

      const u4 = await appWindow.listen("speech_end", () => {
        if (!isMounted) return;
        setIsListening(false);
        endSpeechSegment();
        startHold();
      });

      const u5 = await appWindow.listen<SystemStats>("system_stats", (event) => {
        if (!isMounted) return;
        setStats(event.payload);
      });

      const u6 = await appWindow.listen("toggle_hud", () => {
        if (!isMounted) return;
        if (visibilityState === 'HIDDEN') show();
        else hideImmediately();
      });

      const u7 = await appWindow.listen<{ amplitude: number }>("audio_amplitude", (event) => {
        if (!isMounted) return;
        setAmplitudeBuffer(prev => {
          const next = [...prev.slice(1), event.payload.amplitude];
          return next;
        });
      });

      const u8 = await appWindow.listen<{ state: string }>("ptt_status", (event) => {
        if (!isMounted) return;
        setPttStatus(event.payload.state as any);
      });


      if (isMounted) {
        unlisteners = [u1, u2, u3, u4, u5, u6, u7, u8];
      } else {
        u1(); u2(); u3(); u4(); u5(); u6(); u7(); u8();
      }
    };

    setupListeners();

    return () => {
      isMounted = false;
      unlisteners.forEach(u => u());
    };
  }, [startNewInteraction, show, updatePartial, commitFinal, endSpeechSegment, startHold, history, visibilityState, hideImmediately]);

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

  const togglePtt = () => {
    if (pttStatus === 'IDLE') {
      invoke("ptt_start");
    } else {
      invoke("ptt_stop");
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
            className="w-[420px] h-[250px] flex flex-col liquid-glass overflow-hidden rounded-2xl"
            onMouseEnter={() => setIsHovered(true)}
            onMouseLeave={() => setIsHovered(false)}
          >
            <Header 
              isListening={isListening || pttStatus === 'RECORDING'} 
              hasContent={!!currentTargetText} 
              copied={copied} 
              isPttActive={pttStatus !== 'IDLE'}
              onCopy={copyToClipboard} 
              onClose={hideImmediately}
              onTogglePtt={togglePtt}
            />

            <div className="flex-1 flex flex-col relative overflow-hidden group">
              {/* Navigation Buttons - Top Right of Body */}
              {(history.getAll().length > 0) && (
                <div className="absolute top-2 right-4 flex items-center gap-1 z-30 opacity-0 group-hover:opacity-100 transition-opacity duration-300">
                  <button 
                    onClick={handlePrev}
                    disabled={historyIndex === 0}
                    className="p-1.5 rounded-md hover:bg-white/5 disabled:opacity-20 transition-all text-white/40 hover:text-white"
                  >
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><path d="m15 18-6-6 6-6"/></svg>
                  </button>
                  <button 
                    onClick={handleNext}
                    disabled={!viewingHistory}
                    className="p-1.5 rounded-md hover:bg-white/5 disabled:opacity-20 transition-all text-white/40 hover:text-white"
                  >
                    <motion.svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><path d="m9 18 6-6-6-6"/></motion.svg>
                  </button>
                </div>
              )}

              <TranscriptRenderer 
                displayText={displayText} 
                isListening={isListening || pttStatus === 'RECORDING'} 
                pttStatus={pttStatus}
                amplitudeBuffer={amplitudeBuffer}
              />
            </div>

            <Footer stats={stats} />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
