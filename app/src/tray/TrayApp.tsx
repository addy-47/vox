import React, { useState, useEffect, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { motion, AnimatePresence } from "framer-motion";
import { X, Copy, Check, Cpu, Zap, Activity } from "lucide-react";

// ─── Telemetry Data Type ───────────────────────────────────────────────────
interface SystemStats {
  cpu_usage: number;
  memory_used_mb: number;
  memory_total_mb: number;
}

export const TrayApp: React.FC = () => {
  const [partialText, setPartialText] = useState("");
  const [finalText, setFinalText] = useState("");
  const [isListening, setIsListening] = useState(false);
  const [activeSessionId, setActiveSessionId] = useState<number>(0);
  const [isVisible, setIsVisible] = useState(true);
  const [copied, setCopied] = useState(false);
  const [stats, setStats] = useState<SystemStats | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll on new content
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [partialText, finalText]);

  // IPC Event listeners
  useEffect(() => {
    document.body.classList.add("is-tray");
    let isMounted = true;
    let unlisteners: (() => void)[] = [];

    const setupListeners = async () => {
      const appWindow = getCurrentWindow();
      
      const u1 = await appWindow.listen<{ session_id: number }>("speech_start", (event) => {
        if (!isMounted) return;
        setIsListening(true);
        setPartialText("");
        setFinalText("");
        setIsVisible(true);
        setCopied(false);
        setActiveSessionId(event.payload.session_id);
      });

      const u2 = await appWindow.listen<{ text: string, session_id: number }>("transcript_partial", (event) => {
        if (!isMounted) return;
        // Accessing latest state via functional update to avoid stale closure
        setActiveSessionId(currentId => {
          if (event.payload.session_id === currentId) {
            setPartialText(event.payload.text);
          }
          return currentId;
        });
      });

      const u3 = await appWindow.listen<{ text: string, session_id: number }>("transcript_final", (event) => {
        if (!isMounted) return;
        setActiveSessionId(currentId => {
          if (event.payload.session_id === currentId) {
            setFinalText(event.payload.text);
            setPartialText("");
          }
          return currentId;
        });
      });

      const u4 = await appWindow.listen("speech_end", () => {
        if (!isMounted) return;
        setIsListening(false);
      });

      const u5 = await appWindow.listen<SystemStats>("system_stats", (event) => {
        if (!isMounted) return;
        setStats(event.payload);
      });

      if (isMounted) {
        unlisteners = [u1, u2, u3, u4, u5];
      } else {
        u1(); u2(); u3(); u4(); u5();
      }
    };

    setupListeners();

    return () => {
      isMounted = false;
      unlisteners.forEach(u => u());
    };
  }, []); // Remove isFinalized from deps, we want one stable listener

  const handleClose = () => {
    setIsVisible(false);
    // Flush state on close so previous transcripts don't reappear
    setPartialText("");
    setFinalText("");
    setIsListening(false);
  };

  const copyToClipboard = () => {
    const text = finalText || partialText;
    if (text) {
      navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className={`tray-container select-none overflow-hidden ${isVisible ? 'visible' : 'hidden'}`} data-tauri-drag-region>
      <motion.div 
        key={activeSessionId}
        initial={{ opacity: 0, x: 100 }}
        animate={{ opacity: 1, x: 0 }}
        transition={{ type: "spring", damping: 30, stiffness: 300 }}
        className="flex-1 flex flex-col relative liquid-glass overflow-hidden border border-cyan-400/20 shadow-[inset_0_1px_1px_rgba(255,255,255,0.1)]"
      >
        {/* Deep Cyan Gradient Glow (Ambient) */}
        <div className="absolute top-[-20%] left-[-10%] w-[120%] h-[50%] bg-cyan-400/10 blur-[80px] rounded-full pointer-events-none" />
        
        {/* Header */}
        <div className="px-6 py-5 flex items-center justify-between border-b border-white/5 relative z-10" data-tauri-drag-region>
          <div className="flex items-center gap-3">
            <div className="relative flex items-center justify-center">
              <motion.div 
                animate={{ 
                  scale: isListening ? [1, 1.4, 1] : 1, 
                  opacity: isListening ? [0.6, 0.2, 0.6] : 0.1 
                }}
                transition={{ repeat: Infinity, duration: 2 }}
                className="absolute w-5 h-5 rounded-full bg-cyan-400 blur-md"
              />
              <div className={`w-2.5 h-2.5 rounded-full z-10 transition-all duration-700 ${isListening ? 'bg-cyan-400 shadow-[0_0_10px_rgba(0,219,233,0.8)]' : 'bg-white/10'}`} />
            </div>
            <span className="text-[11px] font-black tracking-[0.4em] text-white/40 uppercase">
              Vox <span className="text-cyan-400">Live</span>
            </span>
          </div>
          
          <div className="flex items-center gap-2">
            {(finalText || partialText) && (
              <button 
                onClick={copyToClipboard}
                className="p-2.5 rounded-full hover:bg-cyan-400/20 transition-all text-white/30 hover:text-cyan-400 active:scale-90"
              >
                {copied ? <Check size={16} /> : <Copy size={16} />}
              </button>
            )}
            <button 
              onClick={handleClose}
              className="p-2.5 rounded-full hover:bg-white/5 transition-all text-white/20 hover:text-white/80 active:scale-90"
            >
              <X size={16} />
            </button>
          </div>
        </div>

        {/* Content */}
        <div 
          ref={scrollRef} 
          className="flex-1 overflow-y-auto px-7 py-6 custom-scrollbar relative z-10"
        >
          <AnimatePresence mode="wait">
            {(finalText || partialText) ? (
              <motion.div 
                key="text"
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                className="space-y-4"
              >
                <div className="text-[19px] leading-relaxed font-semibold tracking-tight text-white/95 drop-shadow-sm">
                  {finalText || partialText}
                  {isListening && (
                    <motion.span 
                      animate={{ opacity: [0, 1, 0] }}
                      transition={{ repeat: Infinity, duration: 0.8 }}
                      className="inline-block w-[3px] h-[1.1em] ml-1 bg-cyan-400 align-middle shadow-[0_0_8px_rgba(0,219,233,0.8)]"
                    />
                  )}
                </div>
              </motion.div>
            ) : (
              <motion.div 
                key="empty"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="h-full flex flex-col items-center justify-center"
              >
                <Activity size={32} className="mb-4 text-cyan-400/20 animate-pulse" />
                <p className="text-[10px] font-black uppercase tracking-[0.5em] text-white/10">
                  Ready to Listen
                </p>
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        {/* Telemetry Footer */}
        {stats && (
          <div className="px-6 py-4 bg-black/20 border-t border-white/5 flex items-center justify-between z-10">
             <div className="flex items-center gap-5">
                <div className="flex items-center gap-2">
                  <Cpu size={12} className="text-cyan-400" />
                  <span className="text-[10px] font-mono text-cyan-400/90 font-bold">{stats.cpu_usage.toFixed(1)}%</span>
                </div>
                <div className="flex items-center gap-2">
                  <Zap size={12} className="text-cyan-400" />
                  <span className="text-[10px] font-mono text-cyan-400/90 font-bold">{stats.memory_used_mb}MB</span>
                </div>
             </div>
             <div className="text-[9px] font-black text-white/20 tracking-[0.2em] uppercase">
                Obsidian v0.3
             </div>
          </div>
        )}
      </motion.div>
    </div>
  );
};
