import React, { useState, useEffect, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Mic, Copy, Check, MessageSquareText } from "lucide-react";

export const TrayApp: React.FC = () => {
  const [transcript, setTranscript] = useState("");
  const [isActive, setIsActive] = useState(false);
  const [copied, setCopied] = useState(false);
  const activityTimerRef = useRef<number | null>(null);
  const hideTimerRef = useRef<number | null>(null);

  // Mock transcription logic & Activity detection
  useEffect(() => {
    // For testing: Trigger activity every few seconds
    const interval = setInterval(() => {
      const texts = [
        "Analyzing system telemetry...",
        "Neural link synchronized. Waiting for instruction.",
        "Processing real-time audio stream from primary gateway.",
        "Search results for 'local coffee shops' have been indexed.",
      ];
      const randomText = texts[Math.floor(Math.random() * texts.length)];
      
      setIsActive(true);
      setTranscript(prev => prev + " " + randomText);
      
      // Reset timers on activity
      if (activityTimerRef.current) window.clearTimeout(activityTimerRef.current);
      if (hideTimerRef.current) window.clearTimeout(hideTimerRef.current);

      // Start 3s fade timer
      activityTimerRef.current = window.setTimeout(() => {
        // After 3s of zero activity, start the 5s hide timer
        hideTimerRef.current = window.setTimeout(() => {
          setIsActive(false);
        }, 5000);
      }, 3000);

    }, 15000);

    return () => {
      clearInterval(interval);
      if (activityTimerRef.current) window.clearTimeout(activityTimerRef.current);
      if (hideTimerRef.current) window.clearTimeout(hideTimerRef.current);
    };
  }, []);

  const copyToClipboard = () => {
    if (!transcript) return;
    navigator.clipboard.writeText(transcript);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="w-screen h-screen flex items-center justify-end pr-4 overflow-hidden select-none">
      <AnimatePresence>
        {isActive && (
          <motion.div
            initial={{ x: 400, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: 400, opacity: 0 }}
            transition={{ type: "spring", damping: 25, stiffness: 200 }}
            className="relative"
            data-tauri-drag-region
          >
            {/* Thinking Box Panel */}
            <div className="w-[320px] max-h-[400px] flex flex-col bg-white/[0.02] backdrop-blur-[50px] border border-white/10 rounded-[32px] overflow-hidden shadow-[0_25px_80px_rgba(0,0,0,0.4)] transition-all duration-500 hover:border-white/20">
              
              {/* Header area - Draggable */}
              <div className="px-5 py-4 border-b border-white/5 flex items-center justify-between" data-tauri-drag-region>
                <div className="flex items-center gap-3">
                  <div className="relative">
                    <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/20 flex items-center justify-center">
                      <Mic className="w-4 h-4 text-[rgb(var(--accent))]" />
                    </div>
                    <div className="absolute -top-0.5 -right-0.5 w-2.5 h-2.5 rounded-full bg-[rgb(var(--accent))] animate-pulse border-2 border-black" />
                  </div>
                  <span className="text-[10px] font-bold tracking-[0.2em] text-white/40 uppercase">Live Analytics</span>
                </div>
                <button 
                  onClick={copyToClipboard}
                  className="p-2 rounded-xl hover:bg-white/5 transition-colors group"
                >
                  {copied ? <Check className="w-3.5 h-3.5 text-green-400" /> : <Copy className="w-3.5 h-3.5 text-white/40 group-hover:text-white" />}
                </button>
              </div>

              {/* Scrolling Content Area */}
              <div className="flex-1 overflow-y-auto custom-scrollbar p-5 space-y-4 max-h-[300px]">
                <div className="flex gap-4">
                  <div className="flex-shrink-0 mt-1">
                    <MessageSquareText className="w-4 h-4 text-[rgb(var(--accent))]/60" />
                  </div>
                  <div className="space-y-2">
                    <p className="text-[13px] leading-relaxed text-white/80 font-medium whitespace-pre-wrap">
                      {transcript || "Listening for speech patterns..."}
                    </p>
                    <div className="flex gap-1">
                      {[1, 2, 3].map(i => (
                        <motion.div 
                          key={i}
                          animate={{ opacity: [0.3, 1, 0.3] }}
                          transition={{ repeat: Infinity, duration: 1.5, delay: i * 0.2 }}
                          className="w-1 h-1 rounded-full bg-[rgb(var(--accent))]/40"
                        />
                      ))}
                    </div>
                  </div>
                </div>
              </div>

              {/* Footer Indicator */}
              <div className="h-1 w-full bg-gradient-to-r from-transparent via-[rgb(var(--accent))]/20 to-transparent opacity-50" />
            </div>

            {/* Thinking Box Point (Right) */}
            <div className="absolute top-1/2 -right-1 w-4 h-4 bg-white/5 border-r border-t border-white/10 rotate-45 -translate-y-1/2" />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
);
};
