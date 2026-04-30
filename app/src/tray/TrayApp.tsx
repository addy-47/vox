import React, { useState, useEffect, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Copy, Check, X } from "lucide-react";
import { cn } from "../shared/lib/utils";

export const TrayApp: React.FC = () => {
  const [transcript, setTranscript] = useState("");
  const [isActive, setIsActive] = useState(false);
  const [copied, setCopied] = useState(false);
  const [isTrayEnabled, setIsTrayEnabled] = useState(true);
  const [isManuallyClosed, setIsManuallyClosed] = useState(false);
  
  // Custom Settings (from localStorage)
  const [textColor, setTextColor] = useState("accent");
  const [blurDensity, setBlurDensity] = useState(40);
  const [hideDelay, setHideDelay] = useState(5.0);

  const activityTimerRef = useRef<number | null>(null);
  const hideTimerRef = useRef<number | null>(null);
  const audioContextRef = useRef<AudioContext | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const streamRef = useRef<MediaStream | null>(null);

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

  // Voice Activity Detection (VAD) Logic
  useEffect(() => {
    if (!isTrayEnabled || isManuallyClosed) return;

    const startVAD = async () => {
      try {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
        streamRef.current = stream;
        
        const audioContext = new AudioContext();
        audioContextRef.current = audioContext;
        
        const source = audioContext.createMediaStreamSource(stream);
        const analyser = audioContext.createAnalyser();
        analyser.fftSize = 256;
        source.connect(analyser);
        analyserRef.current = analyser;

        const bufferLength = analyser.frequencyBinCount;
        const dataArray = new Uint8Array(bufferLength);

        const checkAudio = () => {
          if (!analyserRef.current) return;
          analyserRef.current.getByteFrequencyData(dataArray);
          
          let sum = 0;
          for (let i = 0; i < bufferLength; i++) {
            sum += dataArray[i];
          }
          const average = sum / bufferLength;

          // Threshold for speech detection
          if (average > 25) {
            handleSpeechDetected();
          }

          requestAnimationFrame(checkAudio);
        };

        checkAudio();
      } catch (err) {
        console.error("VAD Initialization Error:", err);
      }
    };

    const handleSpeechDetected = () => {
      setIsActive(true);
      setIsManuallyClosed(false);
      
      // Clear timers
      if (activityTimerRef.current) window.clearTimeout(activityTimerRef.current);
      if (hideTimerRef.current) window.clearTimeout(hideTimerRef.current);

      // Simple mock transcription for demonstration
      if (Math.random() > 0.95) {
        setTranscript(prev => prev + " [Speech detected...]");
      }

      // Start the hide timer after silence
      activityTimerRef.current = window.setTimeout(() => {
        hideTimerRef.current = window.setTimeout(() => {
          setIsActive(false);
        }, hideDelay * 1000);
      }, 1000);
    };

    startVAD();

    return () => {
      if (streamRef.current) streamRef.current.getTracks().forEach(track => track.stop());
      if (audioContextRef.current) audioContextRef.current.close();
    };
  }, [isTrayEnabled, isManuallyClosed, hideDelay]);

  const copyToClipboard = () => {
    if (!transcript) return;
    navigator.clipboard.writeText(transcript);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  if (!isTrayEnabled) return null;

  return (
    <div 
      className="w-screen h-screen flex items-center justify-end pr-2 overflow-hidden select-none"
      data-tauri-drag-region
    >
      <AnimatePresence>
        {isActive && !isManuallyClosed && (
          <motion.div
            initial={{ x: 400, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: 400, opacity: 0 }}
            transition={{ type: "spring", damping: 25, stiffness: 200 }}
            className="relative cursor-move"
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
                   <button onClick={() => setIsManuallyClosed(true)} className="p-2 hover:bg-white/5 rounded-lg transition-colors">
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
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
