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

  // Custom settings from localStorage
  const [textColor, setTextColor] = useState("accent");
  const [blurDensity, setBlurDensity] = useState(40);
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
      setBlurDensity(parseInt(localStorage.getItem("trayBlurDensity") || "40"));
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
      // ── speech_start ──────────────────────────────────────────────────────
      // Cold start: show from hidden.
      // Hot start (barge-in): tray may be fading — cancel the timer and
      // immediately bring it back to full opacity.
      const u1 = await listen("speech_start", async () => {
        clearHideTimer();                 // cancel any pending fade
        setPartialText("");               // reset partial for new utterance
        // Keep finalText — user may still want to read it
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
        // Fade out after hideDelay seconds
        scheduleHide(hideDelay * 1000);
      });
      unlisten.push(u3);

      // ── speech_end ────────────────────────────────────────────────────────
      // Rust emits this for the VAD event. The final transcript arrives via
      // transcript_final. If somehow transcript_final never fires (empty audio),
      // we still need to schedule cleanup.
      const u4 = await listen("speech_end", () => {
        // If we already scheduled hide via transcript_final, this is a no-op.
        // If transcript_final hasn't fired yet (empty speech), schedule fade.
        if (hideTimerRef.current === null) {
          scheduleHide(hideDelay * 1000);
        }
      });
      unlisten.push(u4);
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
      className="w-screen h-screen flex items-center justify-end pr-2 overflow-hidden select-none"
      data-tauri-drag-region
    >
      <AnimatePresence mode="wait">
        {isVisible && (
          <motion.div
            key="tray-card"
            initial={{ opacity: 0, x: 40, scale: 0.95, filter: "blur(10px)" }}
            animate={
              visState === "fading"
                ? { opacity: 0, x: 40, scale: 0.95, filter: "blur(10px)" }
                : { opacity: 1, x: 0, scale: 1, filter: "blur(0px)" }
            }
            exit={{ opacity: 0, x: 40, scale: 0.95, filter: "blur(10px)" }}
            transition={{ duration: 0.4, ease: [0.16, 1, 0.3, 1] }}
            onAnimationComplete={onAnimationComplete}
            className="w-[340px] max-h-[500px] flex flex-col bg-white/[0.03] border border-white/10 rounded-2xl overflow-hidden shadow-[0_32px_120px_rgba(0,0,0,0.5)]"
            style={{
              backdropFilter: `blur(${blurDensity}px)`,
              borderColor: "rgba(var(--accent), 0.1)",
            }}
            data-tauri-drag-region
          >
            {/* Header */}
            <div
              className="px-6 py-4 border-b border-white/5 flex items-center justify-between"
              data-tauri-drag-region
            >
              <div className="flex items-center gap-3">
                <div className="w-2 h-2 rounded-full bg-[rgb(var(--accent))] animate-pulse" />
                <span className="text-[10px] font-bold tracking-[0.2em] text-white/40 uppercase">
                  Vox Live
                </span>
              </div>
              <div className="flex items-center gap-1">
                <button
                  onClick={copyToClipboard}
                  className="p-2 hover:bg-white/5 rounded-lg transition-colors"
                  title="Copy to clipboard"
                >
                  {copied ? (
                    <Check size={14} className="text-green-400" />
                  ) : (
                    <Copy size={14} className="text-white/40" />
                  )}
                </button>
                <button
                  onClick={handleClose}
                  className="p-2 hover:bg-white/5 rounded-lg transition-colors"
                  title="Close"
                >
                  <X size={14} className="text-white/40 hover:text-red-400" />
                </button>
              </div>
            </div>

            {/* Content */}
            <div
              ref={scrollRef}
              className="flex-1 overflow-y-auto p-6 space-y-3 max-h-[380px] scroll-smooth"
              style={{ scrollbarWidth: "thin", scrollbarColor: "rgba(255,255,255,0.1) transparent" }}
            >
              {/* Finalized text */}
              {finalText ? (
                <p className="text-[14px] leading-relaxed font-medium text-white">
                  {finalText}
                </p>
              ) : null}

              {/* Streaming partial text */}
              {partialText ? (
                <div className="space-y-2">
                  <p
                    className={cn(
                      "text-[14px] leading-relaxed font-medium transition-all duration-300",
                      textColor === "accent"
                        ? "text-[rgb(var(--accent))]"
                        : "text-white/60"
                    )}
                  >
                    {partialText}
                  </p>
                  {/* Streaming indicator dots */}
                  <div className="flex gap-1.5 opacity-40">
                    {[0, 1, 2].map((i) => (
                      <div
                        key={i}
                        className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-bounce"
                        style={{ animationDelay: `${i * 0.15}s` }}
                      />
                    ))}
                  </div>
                </div>
              ) : null}

              {/* Empty state */}
              {!finalText && !partialText && (
                <p className="text-[13px] text-white/20 italic">
                  Listening…
                </p>
              )}
            </div>

            {/* Bottom accent line */}
            <div className="h-px w-full bg-gradient-to-r from-transparent via-[rgb(var(--accent))]/20 to-transparent" />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
