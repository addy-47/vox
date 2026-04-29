import React, { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Mic } from "lucide-react";

export const TrayApp: React.FC = () => {
  const [transcript, setTranscript] = useState("");
  const isActive = true;

  // Mock transcription stream
  useEffect(() => {
    const text = "Searching for the nearest coffee shops in your area... done.";
    let i = 0;
    const interval = setInterval(() => {
      setTranscript(text.slice(0, i));
      i++;
      if (i > text.length) clearInterval(interval);
    }, 50) as unknown as number;
    
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="w-screen h-screen flex items-end justify-center pb-12 overflow-hidden pointer-events-none">
      <AnimatePresence>
        {isActive && (
          <motion.div
            initial={{ y: 100, opacity: 0, scale: 0.9 }}
            animate={{ y: 0, opacity: 1, scale: 1 }}
            exit={{ y: 100, opacity: 0, scale: 0.9 }}
            transition={{ type: "spring", damping: 20, stiffness: 300 }}
            className="pointer-events-auto"
          >
            <div className="glass-panel px-8 py-4 rounded-full flex items-center gap-6 shadow-[0_20px_50px_rgba(0,0,0,0.5)] border-white/10 max-w-2xl min-w-[300px]">
              <div className="relative">
                <div className="w-10 h-10 rounded-full bg-primary-container/20 flex items-center justify-center border border-primary-container/30">
                  <Mic className="w-5 h-5 text-primary-container" />
                </div>
                {/* Waveform Animation */}
                <div className="absolute inset-0 flex items-center justify-center gap-0.5">
                  {[1, 2, 3, 4].map((i) => (
                    <motion.div
                      key={i}
                      animate={{ height: [8, 16, 8] }}
                      transition={{ repeat: Infinity, duration: 0.5, delay: i * 0.1 }}
                      className="w-0.5 bg-primary-container rounded-full opacity-40"
                    />
                  ))}
                </div>
              </div>

              <div className="flex-1">
                <p className="text-white/80 font-medium text-sm leading-tight tracking-tight line-clamp-1">
                  {transcript || "Listening..."}
                </p>
              </div>

              <div className="flex gap-1">
                <div className="w-1.5 h-1.5 rounded-full bg-primary-container animate-pulse" />
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
