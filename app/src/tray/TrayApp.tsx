import React, { useState, useEffect, useRef, useCallback } from "react";
import { Copy, Check, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";

// ─── Types ───────────────────────────────────────────────────────────────────

type VisibilityState = "hidden" | "visible" | "fading";

// ─── Component ───────────────────────────────────────────────────────────────

export const TrayApp: React.FC = () => {
  const [partialText, setPartialText] = useState("");
  const [finalText, setFinalText] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll on new content
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [partialText, finalText]);

  // IPC Event listeners
  useEffect(() => {
    let isMounted = true;
    let unlisteners: (() => void)[] = [];

    const setupListeners = async () => {
      console.log("[HUD] Setting up minimal IPC listeners");
      const appWindow = getCurrentWindow();
      
      const u1 = await appWindow.listen("speech_start", () => {
        console.log("[HUD] Speech start received");
        setPartialText("");
        setFinalText("");
      });

      const u2 = await appWindow.listen<{ text: string }>("transcript_partial", (event) => {
        console.log("[HUD] Partial received:", event.payload.text);
        setPartialText(event.payload.text);
      });

      const u3 = await appWindow.listen<{ text: string }>("transcript_final", (event) => {
        console.log("[HUD] Final received:", event.payload.text);
        setFinalText(event.payload.text);
        setPartialText("");
      });

      const u4 = await appWindow.listen("speech_end", () => {
        console.log("[HUD] Speech end received");
      });

      if (isMounted) {
        unlisteners = [u1, u2, u3, u4];
      } else {
        u1();
        u2();
        u3();
        u4();
      }
    };

    setupListeners();

    return () => {
      console.log("[HUD] Cleaning up listeners");
      isMounted = false;
      unlisteners.forEach(u => u());
    };
  }, []);

  return (
    <div className="fixed inset-0 flex items-center justify-center p-8 select-none" data-tauri-drag-region>
      <div className="w-[400px] flex flex-col relative pointer-events-auto bg-[#0A0A0A]/90 backdrop-blur-3xl border border-white/20 rounded-[2rem] overflow-hidden shadow-[0_32px_64px_-16px_rgba(0,0,0,0.6)]">
        
        {/* Header */}
        <div className="px-8 py-5 border-b border-white/10 flex items-center gap-4" data-tauri-drag-region>
          <div className="w-2.5 h-2.5 rounded-full bg-green-500 animate-pulse shadow-[0_0_12px_rgba(34,197,94,0.6)]" />
          <span className="text-[11px] font-black tracking-[0.25em] text-white/50 uppercase" data-tauri-drag-region>
            Vox STT Debug
          </span>
        </div>

        {/* Content Area */}
        <div ref={scrollRef} className="flex-1 overflow-y-auto px-8 py-7 min-h-[160px] max-h-[500px]">
          <div className="space-y-6">
            {finalText && (
              <div className="text-[16px] text-white/95 leading-relaxed font-semibold tracking-tight">
                FINAL: {finalText}
              </div>
            )}
            {partialText && (
              <div className="text-[16px] text-green-400 leading-relaxed font-semibold opacity-90 italic tracking-tight">
                PARTIAL: {partialText}
              </div>
            )}
            {!finalText && !partialText && (
              <div className="text-[12px] text-white/30 uppercase tracking-widest font-bold">
                Awaiting Speech...
              </div>
            )}
          </div>
        </div>

      </div>
    </div>
  );
};
