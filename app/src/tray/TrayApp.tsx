import React, { useState, useEffect, useRef, useCallback } from "react";
import { Copy, Check, X } from "lucide-react";
import { cn } from "../shared/lib/utils";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence, type AnimationDefinition } from "framer-motion";

// ─── Types ───────────────────────────────────────────────────────────────────

type VisibilityState = "hidden" | "visible" | "fading";

// ─── Component ───────────────────────────────────────────────────────────────

export const TrayApp: React.FC = () => {
  const [visState, setVisState] = useState<VisibilityState>("hidden");
  const [partialText, setPartialText] = useState("");
  const [finalText, setFinalText] = useState("");
  const [copied, setCopied] = useState(false);
  const [isTrayEnabled, setIsTrayEnabled] = useState(true);
  const [isProcessing, setIsProcessing] = useState(false);

  // Custom settings from localStorage
  const [textColor, setTextColor] = useState("accent");
  const [hideDelay, setHideDelay] = useState(5.0);

  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const appWindow = getCurrentWindow();

  // ── Auto-scroll on new content ──────────────────────────────────────────
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [partialText, finalText]);

  // ── Settings sync ────────────────────────────────────────────────────────
  useEffect(() => {
    const sync = () => {
      setIsTrayEnabled(localStorage.getItem("isTrayEnabled") !== "false");
      setTextColor(localStorage.getItem("trayTextColor") || "accent");
      setHideDelay(parseFloat(localStorage.getItem("trayHideDuration") || "5.0"));
    };
    sync();
    window.addEventListener("storage", sync);
    return () => window.removeEventListener("storage", sync);
  }, []);

  // ── Hide timer management ─────────────────────────────────────────────────
  const clearHideTimer = useCallback(() => {
    if (hideTimerRef.current !== null) {
      clearTimeout(hideTimerRef.current);
      hideTimerRef.current = null;
    }
  }, []);

  const scheduleHide = useCallback((delayMs: number) => {
    clearHideTimer();
    hideTimerRef.current = setTimeout(() => {
      setVisState("fading");
    }, delayMs);
  }, [clearHideTimer]);

  // ── IPC Event listeners ───────────────────────────────────────────────────
  useEffect(() => {
    if (!isTrayEnabled) return;

    const unlisten: (() => void)[] = [];

    const setup = async () => {
      // 1. Check if engine is already ready (handles race conditions)
      try {
        const isReady = await invoke<boolean>("check_engine_status");
        if (isReady) {
          console.log("[HUD] Engine already ready on mount");
          setVisState("visible");
          setIsProcessing(false);
          scheduleHide(5000);
        }
      } catch (err) {
        console.error("Failed to check engine status:", err);
      }
      // ── speech_start ──────────────────────────────────────────────────────
      // Cold start: show from hidden.
      // Hot start (barge-in): tray may be fading — cancel the timer and
      // immediately bring it back to full opacity.
      const u1 = await listen("speech_start", async () => {
        clearHideTimer();                 // cancel any pending fade
        setPartialText("");               // reset partial for new utterance
        setIsProcessing(false);
        setVisState("visible");           // immediately visible (Framer handles animate)
        try {
          await appWindow.show();
        } catch (_) {}
      });
      unlisten.push(u1);

      // ── transcript_partial ────────────────────────────────────────────────
      // Emitted every 800ms during speech (throttled in Rust).
      const u2 = await listen<{ text: string }>("transcript_partial", (event) => {
        setPartialText(event.payload.text);
        setIsProcessing(false);
        // If user was speaking while fade was active, restore visibility
        clearHideTimer();
        setVisState("visible");
      });
      unlisten.push(u2);

      // ── transcript_final ──────────────────────────────────────────────────
      // Emitted once after speech_end — this is the authoritative transcription.
      // Promote partial → final, then schedule fade-out.
      const u3 = await listen<{ text: string }>("transcript_final", (event) => {
        const text = event.payload.text;
        setFinalText(text);
        setPartialText("");
        setIsProcessing(false);
        // Fade out after hideDelay seconds
        scheduleHide(hideDelay * 1000);
      });
      unlisten.push(u3);

      // ── speech_end ────────────────────────────────────────────────────────
      // Rust emits this when VAD triggers silence. The final transcript will
      // arrive shortly after via transcript_final.
      const u4 = await listen("speech_end", () => {
        // If we haven't received any text yet, we are processing.
        if (!partialText && !finalText) {
          setIsProcessing(true);
        }
        
        if (hideTimerRef.current === null) {
          scheduleHide(15000); 
        }
      });
      unlisten.push(u4);

      // ── engine_launched ───────────────────────────────────────────────────
      const u5 = await listen("engine_launched", () => {
        console.log("Engine launched event received");
        setVisState("visible");
        setIsProcessing(false);
        // Show "Ready" for 5 seconds then fade if no speech
        scheduleHide(5000);
      });
      unlisten.push(u5);
    };

    setup().catch(console.error);

    return () => {
      unlisten.forEach((fn) => fn());
      clearHideTimer();
    };
  }, [isTrayEnabled, hideDelay, clearHideTimer, scheduleHide]);

  // ── Copy to clipboard ────────────────────────────────────────────────────
  const copyToClipboard = () => {
    const text = [finalText, partialText].filter(Boolean).join("\n");
    if (!text) return;
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleClose = async () => {
    clearHideTimer();
    setVisState("hidden");
    try {
      await appWindow.hide();
    } catch (_) {}
  };

  // ── Framer exit → actually hide the OS window ─────────────────────────────
  const onAnimationComplete = (definition: AnimationDefinition) => {
    // Only trigger hide after the EXIT animation completes
    if (visState === "fading" && definition === "exit") {
      setPartialText("");
      setFinalText("");
      setVisState("hidden");
      invoke("hide_tray_window").catch(console.error);
    }
  };

  if (!isTrayEnabled) return null;

  const isVisible = visState === "visible" || visState === "fading";

  return (
    <div
      className="w-screen h-screen flex items-center justify-end pr-4 overflow-hidden select-none"
      data-tauri-drag-region
    >
      <AnimatePresence mode="wait">
        {isVisible && (
          <motion.div
            key="tray-card"
            initial={{ opacity: 0, x: 60, scale: 0.9, filter: "blur(20px)" }}
            animate={
              visState === "fading"
                ? { opacity: 0, x: 40, scale: 0.95, filter: "blur(10px)" }
                : { opacity: 1, x: 0, scale: 1, filter: "blur(0px)" }
            }
            exit={{ opacity: 0, x: 60, scale: 0.9, filter: "blur(20px)" }}
            transition={{ duration: 0.6, ease: [0.16, 1, 0.3, 1] }}
            onAnimationComplete={onAnimationComplete}
            className="w-[380px] max-h-[550px] flex flex-col relative group"
            data-tauri-drag-region
          >
            {/* Iridescent Border Wrapper */}
            <div className="absolute -inset-[1px] bg-gradient-to-br from-white/20 via-transparent to-[rgb(var(--accent))]/20 rounded-[2rem] blur-[0.5px] opacity-50" />
            
            <div 
              className="relative flex flex-col h-full w-full bg-white/[0.02] dark:bg-black/20 backdrop-blur-[50px] border border-white/10 rounded-[2rem] overflow-hidden shadow-[0_40px_100px_-20px_rgba(0,0,0,0.6)]"
              style={{
                borderColor: "rgba(var(--accent), 0.08)",
              }}
            >
              {/* Header: Liquid Frost Effect */}
              <div
                className="px-8 py-5 border-b border-white/5 flex items-center justify-between bg-white/[0.01]"
                data-tauri-drag-region
              >
                <div className="flex items-center gap-4">
                  <div className="relative">
                    <div className="w-2.5 h-2.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_15px_rgba(var(--accent),0.6)]" />
                    <div className="absolute inset-0 w-2.5 h-2.5 rounded-full bg-[rgb(var(--accent))] animate-ping opacity-40" />
                  </div>
                  <div className="flex flex-col">
                    <span className="text-[10px] font-black tracking-[0.25em] text-white/40 uppercase leading-tight">
                      VOX CORE
                    </span>
                    <span className="text-[9px] font-medium text-[rgb(var(--accent))]/60 tracking-wider">
                      NEURAL PIPELINE ACTIVE
                    </span>
                  </div>
                </div>
                
                <div className="flex items-center gap-2">
                  <button
                    onClick={copyToClipboard}
                    className="p-2 hover:bg-white/5 rounded-xl transition-all duration-300 group/btn active:scale-90"
                    title="Copy to clipboard"
                  >
                    {copied ? (
                      <Check size={16} className="text-green-400" />
                    ) : (
                      <Copy size={16} className="text-white/30 group-hover/btn:text-white/70" />
                    )}
                  </button>
                  <button
                    onClick={handleClose}
                    className="p-2 hover:bg-red-500/10 rounded-xl transition-all duration-300 group/close active:scale-90"
                    title="Close"
                  >
                    <X size={16} className="text-white/30 group-hover/close:text-red-400" />
                  </button>
                </div>
              </div>

              {/* Content: Floating Typography */}
              <div
                ref={scrollRef}
                className="flex-1 overflow-y-auto px-8 py-7 space-y-6 max-h-[420px] scroll-smooth custom-scrollbar"
              >
                {/* Finalized text with soft entry */}
                <AnimatePresence>
                  {finalText && (
                    <motion.div
                      initial={{ opacity: 0, y: 10 }}
                      animate={{ opacity: 1, y: 0 }}
                      className="text-[15px] leading-[1.7] font-medium text-white/90 selection:bg-[rgb(var(--accent))]/30"
                    >
                      {finalText}
                    </motion.div>
                  )}
                </AnimatePresence>

                {/* Streaming partial text */}
                <AnimatePresence>
                  {partialText && (
                    <motion.div 
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      className="space-y-4"
                    >
                      <p
                        className={cn(
                          "text-[15px] leading-[1.7] font-medium transition-all duration-500",
                          textColor === "accent"
                            ? "text-[rgb(var(--accent))] drop-shadow-[0_0_8px_rgba(var(--accent),0.3)]"
                            : "text-white/50"
                        )}
                      >
                        {partialText}
                        <span className="inline-flex ml-1 w-1 h-4 bg-[rgb(var(--accent))]/40 animate-pulse align-middle" />
                      </p>
                    </motion.div>
                  )}
                </AnimatePresence>

                {/* Status Indicator */}
                {!finalText && !partialText && (
                  <div className="flex flex-col items-center justify-center py-12 gap-4 opacity-20">
                    <div className="flex gap-1">
                      {[0, 1, 2].map((i) => (
                        <div
                          key={i}
                          className={cn(
                            "w-1 h-1 rounded-full bg-white animate-pulse",
                            isProcessing && "bg-[rgb(var(--accent))]"
                          )}
                          style={{ animationDelay: `${i * 0.2}s` }}
                        />
                      ))}
                    </div>
                    <span className={cn(
                      "text-[11px] font-bold tracking-[0.3em] uppercase",
                      isProcessing && "text-[rgb(var(--accent))] opacity-100"
                    )}>
                      {isProcessing ? "Analyzing" : "Ready"}
                    </span>
                  </div>
                )}
              </div>

              {/* Footer: Dynamic Ambient Light */}
              <div className="h-1.5 w-full bg-gradient-to-r from-transparent via-[rgb(var(--accent))]/30 to-transparent blur-[2px] opacity-50" />
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
