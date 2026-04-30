import React, { useState, useEffect, useRef } from "react";
import { Copy, Check, X } from "lucide-react";
import { cn } from "../shared/lib/utils";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

export const TrayApp: React.FC = () => {
  const [transcript, setTranscript] = useState("");
  const [copied, setCopied] = useState(false);
  const [isTrayEnabled, setIsTrayEnabled] = useState(true);
  
  // Custom Settings (from localStorage)
  const [textColor, setTextColor] = useState("accent");
  const [blurDensity, setBlurDensity] = useState(40);
  const [hideDelay, setHideDelay] = useState(5.0);

  const hideTimerRef = useRef<number | null>(null);

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
    let unlistenTranscript: () => void;
    let unlistenSpeechEnd: () => void;

    const setupListeners = async () => {
      const appWindow = getCurrentWindow();

      unlistenSpeechStart = await listen("speech_start", async () => {
        if (hideTimerRef.current) window.clearTimeout(hideTimerRef.current);
        setTranscript(""); // Clear previous transcript on new speech
        await appWindow.show();
        await appWindow.setFocus();
      });

      unlistenTranscript = await listen<string>("transcript_partial", (event) => {
        setTranscript(event.payload);
      });

      unlistenSpeechEnd = await listen("speech_end", () => {
        if (hideTimerRef.current) window.clearTimeout(hideTimerRef.current);
        hideTimerRef.current = window.setTimeout(async () => {
          await appWindow.hide();
        }, hideDelay * 1000);
      });
    };

    setupListeners();

    return () => {
      if (unlistenSpeechStart) unlistenSpeechStart();
      if (unlistenTranscript) unlistenTranscript();
      if (unlistenSpeechEnd) unlistenSpeechEnd();
    };
  }, [isTrayEnabled, hideDelay]);

  const copyToClipboard = () => {
    if (!transcript) return;
    navigator.clipboard.writeText(transcript);
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
      className="w-screen h-screen flex items-center justify-end pr-2 overflow-hidden select-none"
      data-tauri-drag-region
    >
      <div 
        className="w-[340px] max-h-[500px] flex flex-col bg-white/[0.03] border border-white/10 rounded-2xl overflow-hidden shadow-[0_32px_120px_rgba(0,0,0,0.5)] transition-all duration-700"
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
        <div className="flex-1 overflow-y-auto custom-scrollbar p-6 space-y-4 max-h-[350px]">
          <div className="flex gap-4">
            <div className="space-y-3">
              <p className={cn(
                "text-[14px] leading-relaxed font-medium transition-colors duration-500",
                textColor === 'accent' ? "text-[rgb(var(--accent))]" : textColor === 'white' ? "text-white" : "text-white/40"
              )}>
                {transcript || "Listening for speech..."}
              </p>
              <div className="flex gap-1.5 opacity-40">
                {[1, 2, 3].map(i => (
                  <div key={i} className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-bounce" style={{ animationDelay: `${i * 0.2}s` }} />
                ))}
              </div>
            </div>
          </div>
        </div>

        <div className="h-1.5 w-full bg-gradient-to-r from-transparent via-[rgb(var(--accent))]/30 to-transparent" />
      </div>
    </div>
  );
};

