import React, { useState, useEffect, useRef } from "react";
import { Copy, Check, X } from "lucide-react";
import { cn } from "../shared/lib/utils";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

export const TrayApp: React.FC = () => {
  const [activeTranscript, setActiveTranscript] = useState("");
  const [history, setHistory] = useState<string[]>([]);
  const [copied, setCopied] = useState(false);
  const [isTrayEnabled, setIsTrayEnabled] = useState(true);
  
  // Custom Settings (from localStorage)
  const [textColor, setTextColor] = useState("accent");
  const [blurDensity, setBlurDensity] = useState(40);
  const [hideDelay, setHideDelay] = useState(5.0);

  const hideTimerRef = useRef<number | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const [isVisible, setIsVisible] = useState(false);

  // Auto-scroll to bottom
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [history, activeTranscript]);

  // Sync settings
  useEffect(() => {
    const checkSettings = () => {
      setIsTrayEnabled(localStorage.getItem('isTrayEnabled') !== 'false');
      setTextColor(localStorage.getItem('trayTextColor') || 'accent');
      setBlurDensity(parseInt(localStorage.getItem('trayBlurDensity') || '40'));
      setHideDelay(parseFloat(localStorage.getItem('trayHideDuration') || '5.0'));
    };

    checkSettings();
    window.addEventListener('storage', checkSettings);
    return () => window.removeEventListener('storage', checkSettings);
  }, []);

  // IPC Event Listeners
  useEffect(() => {
    if (!isTrayEnabled) return;

    let unlistenSpeechStart: () => void;
    let unlistenPartial: () => void;
    let unlistenFinal: () => void;
    let unlistenSpeechEnd: () => void;

    const setupListeners = async () => {
      const appWindow = getCurrentWindow();

      unlistenSpeechStart = await listen("speech_start", async () => {
        if (hideTimerRef.current) window.clearTimeout(hideTimerRef.current);
        setActiveTranscript(""); // Clear previous partial on new speech
        await appWindow.show();
        await appWindow.setFocus();
        setIsVisible(true);
      });

      unlistenPartial = await listen<{ text: string }>("transcript_partial", (event) => {
        setActiveTranscript(event.payload.text);
      });

      unlistenFinal = await listen<{ text: string }>("transcript_final", (event) => {
        setActiveTranscript("");
        setHistory(prev => [...prev, event.payload.text]);
      });

      unlistenSpeechEnd = await listen("speech_end", () => {
        if (hideTimerRef.current) window.clearTimeout(hideTimerRef.current);
        
        hideTimerRef.current = window.setTimeout(() => {
          setIsVisible(false);
          // Wait for CSS transition (700ms) to finish before physically hiding
          window.setTimeout(async () => {
            await appWindow.hide();
          }, 700);
        }, hideDelay * 1000);
      });
    };

    setupListeners();

    return () => {
      if (unlistenSpeechStart) unlistenSpeechStart();
      if (unlistenPartial) unlistenPartial();
      if (unlistenFinal) unlistenFinal();
      if (unlistenSpeechEnd) unlistenSpeechEnd();
    };
  }, [isTrayEnabled, hideDelay]);

  const copyToClipboard = () => {
    const text = [...history, activeTranscript].filter(Boolean).join("\n");
    if (!text) return;
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleClose = async () => {
    if (hideTimerRef.current) window.clearTimeout(hideTimerRef.current);
    await getCurrentWindow().hide();
  };

  if (!isTrayEnabled) return null;

  return (
    <div 
      className={cn(
        "w-screen h-screen flex items-center justify-end pr-2 overflow-hidden select-none transition-all duration-700 ease-out",
        isVisible ? "opacity-100 translate-x-0" : "opacity-0 translate-x-8"
      )}
      data-tauri-drag-region
    >
      <div 
        className={cn(
          "w-[340px] max-h-[500px] flex flex-col bg-white/[0.03] border border-white/10 rounded-2xl overflow-hidden shadow-[0_32px_120px_rgba(0,0,0,0.5)] transition-all duration-700 ease-out",
          isVisible ? "scale-100" : "scale-95"
        )}
        style={{ 
          backdropFilter: `blur(${blurDensity}px)`,
          borderColor: `rgba(var(--accent), 0.1)`
        }}
        data-tauri-drag-region
      >
        {/* Header */}
        <div className="px-6 py-5 border-b border-white/5 flex items-center justify-between" data-tauri-drag-region>
          <div className="flex items-center gap-3">
            <div className="w-2 h-2 rounded-full bg-[rgb(var(--accent))] animate-pulse" />
            <span className="text-[10px] font-bold tracking-[0.2em] text-white/40 uppercase">Vox Live Engine</span>
          </div>
          <div className="flex items-center gap-2">
              <button onClick={copyToClipboard} className="p-2 hover:bg-white/5 rounded-lg transition-colors">
                {copied ? <Check size={14} className="text-green-400" /> : <Copy size={14} className="text-white/40" />}
              </button>
              <button onClick={handleClose} className="p-2 hover:bg-white/5 rounded-lg transition-colors">
                <X size={14} className="text-white/40 hover:text-red-400" />
              </button>
          </div>
        </div>

        {/* Content */}
        <div 
          ref={scrollRef}
          className="flex-1 overflow-y-auto custom-scrollbar p-6 space-y-4 max-h-[350px] scroll-smooth"
        >
          <div className="flex flex-col gap-3">
            {history.map((text, idx) => (
              <p 
                key={idx}
                className={cn(
                  "text-[14px] leading-relaxed font-medium transition-all duration-500",
                  textColor === 'accent' ? "text-white" : "text-white"
                )}
              >
                {text}
              </p>
            ))}
            
            {activeTranscript && (
              <div className="space-y-2">
                <p className={cn(
                  "text-[14px] leading-relaxed font-medium animate-in fade-in slide-in-from-bottom-1 duration-300",
                  textColor === 'accent' ? "text-[rgb(var(--accent))]" : "text-white/60"
                )}>
                  {activeTranscript}
                </p>
                <div className="flex gap-1.5 opacity-40">
                  {[1, 2, 3].map(i => (
                    <div key={i} className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-bounce" style={{ animationDelay: `${i * 0.2}s` }} />
                  ))}
                </div>
              </div>
            )}

            {!activeTranscript && history.length === 0 && (
              <p className="text-[14px] text-white/20 italic">Listening for speech...</p>
            )}
          </div>
        </div>

        <div className="h-1.5 w-full bg-gradient-to-r from-transparent via-[rgb(var(--accent))]/30 to-transparent" />
      </div>
    </div>
  );
};

