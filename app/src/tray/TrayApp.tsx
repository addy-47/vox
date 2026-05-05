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
    state: visibilityState, isHovered, setIsHovered, show, startHold, hideImmediately 
  } = useVisibility({ holdDuration: 3000, fadeDuration: 2000 });

  const [isListening, setIsListening] = useState(false);
  const [copied, setCopied] = useState(false);
  const [stats, setStats] = useState<SystemStats | null>(null);

  // Sync React state to OS Window and Backend state
  useEffect(() => {
    if (visibilityState === 'HIDDEN') {
      invoke("hide_tray_window");
      invoke("sync_hud_visibility", { visible: false });
    } else if (visibilityState === 'ACTIVE' || visibilityState === 'APPEARING') {
      invoke("sync_hud_visibility", { visible: true });
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

      if (isMounted) {
        unlisteners = [u1, u2, u3, u4, u5, u6];
      } else {
        u1(); u2(); u3(); u4(); u5(); u6();
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
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <AnimatePresence>
        {visibilityState !== 'HIDDEN' && (
          <motion.div 
            key={`hud-${interactionId}`}
            variants={containerVariants}
            initial="HIDDEN"
            animate={visibilityState}
            exit="HIDDEN"
            className="w-full h-full flex flex-col liquid-glass overflow-hidden rounded-2xl"
          >
            <Header 
              isListening={isListening} 
              hasContent={!!currentTargetText} 
              copied={copied} 
              onCopy={copyToClipboard} 
              onClose={hideImmediately}
              onPrev={handlePrev}
              onNext={handleNext}
              canPrev={history.getAll().length > 0 && historyIndex !== 0}
              canNext={viewingHistory}
            />

            <TranscriptRenderer 
              displayText={displayText} 
              isListening={isListening} 
            />

            <Footer stats={stats} />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
