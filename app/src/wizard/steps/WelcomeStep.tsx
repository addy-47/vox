import React, { useState, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ArrowRight, ShieldCheck, Zap, Globe, Activity, ChevronLeft, ChevronRight, Copy, X, Mic } from 'lucide-react';
import { cn } from '@/shared/lib/utils';
import { VoxOrb } from '@/shared/components/AdvancedOrb';

interface Props {
  onNext: () => void;
}

export const WelcomeStep: React.FC<Props> = ({ onNext }) => {
  const [subStep, setSubStep] = useState(1);
  const [hoveredElement, setHoveredElement] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const nextSubStep = () => setSubStep(prev => Math.min(prev + 1, 3));
  const prevSubStep = () => setSubStep(prev => Math.max(prev - 1, 1));

  return (
    <div className="flex flex-col h-full relative">
      {/* Sub-step Navigation Controls (Top Right) */}
      <div className="absolute -top-4 -right-4 flex items-center gap-2 z-50">
        <button 
          onClick={prevSubStep}
          disabled={subStep === 1}
          className="p-2 rounded-full bg-white/5 border border-white/5 hover:bg-white/10 disabled:opacity-20 transition-all"
        >
          <ChevronLeft className="w-4 h-4" />
        </button>
        <button 
          onClick={nextSubStep}
          disabled={subStep === 3}
          className="p-2 rounded-full bg-white/5 border border-white/5 hover:bg-white/10 disabled:opacity-20 transition-all"
        >
          <ChevronRight className="w-4 h-4" />
        </button>
      </div>

      <div className="flex-1 flex flex-col justify-center">
        <AnimatePresence mode="wait">
          {subStep === 1 && (
            <motion.div 
              key="step1"
              initial={{ opacity: 0, x: 20 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -20 }}
              className="flex flex-col gap-10"
            >
              <header>
                <motion.div 
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ duration: 0.3 }}
                  className="flex items-center gap-4 mb-4"
                >
                  <div className="h-[1px] w-8 bg-[#00dbe9]/30" />
                  <span className="text-[11px] font-black tracking-[0.4em] text-[#00dbe9] uppercase">STABLE RELEASE</span>
                </motion.div>

                <motion.h1 
                  className="text-5xl font-black tracking-tighter text-white mb-6 leading-[0.9]"
                >
                  Welcome 
                  <span className="text-transparent bg-clip-text bg-gradient-to-r from-white via-white to-white/50"> to Vox.</span>
                </motion.h1>

                <motion.p 
                  className="text-white/40 text-sm leading-relaxed max-w-md"
                >
                  Vox is a low-latency audio intelligence system designed to live in your system tray and provide real-time interaction.
                </motion.p>
              </header>

              <div className="grid grid-cols-2 gap-4">
                <FeatureCard 
                  icon={<ShieldCheck className="w-4 h-4 text-white/50" />}
                  title="Privacy"
                  desc="100% On-device"
                />
                <FeatureCard 
                  icon={<Zap className="w-4 h-4 text-[#d8baff]" />}
                  title="Latency"
                  desc="Low-Latency Inference"
                />
                <FeatureCard 
                  icon={<Globe className="w-4 h-4 text-[#00dbe9]" />}
                  title="Native"
                  desc="System Integration"
                />
                <FeatureCard 
                  icon={<Activity className="w-4 h-4" />}
                  title="Status"
                  desc="Awaiting Initialization"
                />
              </div>
            </motion.div>
          )}

          {subStep === 2 && (
            <motion.div 
              key="step2"
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 1.05 }}
              className="flex flex-col items-center justify-center text-center py-6 h-full"
            >
              <div className="relative w-72 h-72 mb-8 flex items-center justify-center">
                {/* Authentic Vox Orb */}
                <div className="w-full h-full">
                   <VoxOrb interactionState="Thinking" />
                </div>
                {/* Atmospheric Glow */}
                <div className="absolute inset-0 bg-[#00dbe9]/10 blur-[100px] rounded-full pointer-events-none" />
              </div>
              
              <div className="space-y-4">
                <h2 className="text-3xl font-black tracking-tight text-white uppercase leading-none">The Neural Core</h2>
                <p className="text-white/40 text-sm max-w-sm leading-relaxed mx-auto">
                  Powered by <span className="text-white/80">Qwen-ASR</span> and <span className="text-white/80">Gemma-2B</span>. 
                  Experience low-latency intelligence that processes everything locally on your hardware.
                </p>
              </div>
            </motion.div>
          )}

          {subStep === 3 && (
            <motion.div 
              key="step3"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20 }}
              className="flex flex-col h-full py-4 relative"
              ref={containerRef}
            >
              <div className="flex-1 flex flex-col items-center justify-center relative">
                {/* Central Tray HUD Mockup */}
                <div className="w-[380px] h-[240px] liquid-glass rounded-2xl border border-white/10 overflow-hidden flex flex-col text-left shadow-2xl relative z-10">
                  {/* Authentic Header */}
                  <div className="px-6 py-4 flex items-center justify-between relative z-10 border-b border-white/5">
                    <div 
                      className="flex items-center gap-3 cursor-help group/status"
                      onMouseEnter={() => setHoveredElement('status')}
                      onMouseLeave={() => setHoveredElement(null)}
                      id="step3-status"
                    >
                      <div className="relative flex items-center justify-center">
                        <div className="absolute w-5 h-5 rounded-full bg-[#00dbe9] blur-md opacity-40 animate-pulse" />
                        <div className="w-2.5 h-2.5 rounded-full bg-[#00dbe9] shadow-[0_0_10px_rgba(0,219,233,0.8)] z-10" />
                      </div>
                      <span className="text-[11px] font-black tracking-[0.4em] text-white/60 uppercase">
                        Vox <span className="text-[#00dbe9]">Live</span>
                      </span>
                    </div>
                    <div className="flex items-center gap-2">
                       <div 
                         className="p-2 rounded-lg bg-[#00dbe9]/10 text-[#00dbe9] cursor-help"
                         onMouseEnter={() => setHoveredElement('mic')}
                         onMouseLeave={() => setHoveredElement(null)}
                         id="step3-mic"
                       >
                          <Mic size={16} />
                       </div>
                       <Copy 
                         className="w-4 h-4 text-white/40 hover:text-[#00dbe9] transition-colors cursor-help p-2 box-content rounded-lg hover:bg-white/5" 
                         onMouseEnter={() => setHoveredElement('copy')}
                         onMouseLeave={() => setHoveredElement(null)}
                         id="step3-copy"
                       />
                       <X className="w-4 h-4 text-white/20 p-2 box-content" />
                    </div>
                  </div>

                  {/* Authentic Renderer Mockup */}
                  <div 
                    className="flex-1 px-5 py-4 cursor-help flex flex-col items-center justify-center text-center"
                    onMouseEnter={() => setHoveredElement('renderer')}
                    onMouseLeave={() => setHoveredElement(null)}
                    id="step3-renderer"
                  >
                    <div className="w-full space-y-2">
                       <div className="text-[17px] leading-snug font-medium tracking-tight text-white/90">
                          Actually, let's explore the tray...
                          <motion.span 
                            animate={{ opacity: [0, 1, 0] }}
                            transition={{ repeat: Infinity, duration: 0.8 }}
                            className="inline-block w-[2px] h-[1em] ml-1 align-middle bg-[#00dbe9] shadow-[0_0_8px_rgba(0,219,233,0.8)]"
                          />
                       </div>
                    </div>
                  </div>

                  {/* Authentic Footer */}
                  <div 
                    className="px-7 py-4 bg-black/40 border-t border-white/5 flex items-center justify-between"
                    onMouseEnter={() => setHoveredElement('history')}
                    onMouseLeave={() => setHoveredElement(null)}
                    id="step3-history"
                  >
                     <div className="flex items-center gap-6 opacity-60">
                        <div className="flex items-center gap-2">
                           <Activity size={12} className="text-[#00dbe9]" />
                           <span className="text-[11px] font-mono text-white/80 font-bold uppercase tracking-widest">Active</span>
                        </div>
                        <div className="flex items-center gap-2">
                           <Zap size={12} className="text-[#00dbe9]" />
                           <span className="text-[11px] font-mono text-white/80 font-bold">42MB</span>
                        </div>
                     </div>
                     <div className="flex items-center gap-1">
                        <div className="p-1.5 rounded-md bg-white/5 border border-white/5 flex items-center justify-center">
                           <ChevronLeft className="w-3.5 h-3.5 text-[#00dbe9]" />
                        </div>
                        <div className="p-1.5 rounded-md bg-white/5 border border-white/5 flex items-center justify-center">
                           <ChevronRight className="w-3.5 h-3.5 text-[#00dbe9]" />
                        </div>
                     </div>
                  </div>
                </div>

                {/* Connection Lines (SVG) */}
                <div className="absolute inset-0 pointer-events-none z-0">
                  <CalloutLine active={hoveredElement === 'status'} fromId="step3-status" containerRef={containerRef} />
                  <CalloutLine active={hoveredElement === 'mic'} fromId="step3-mic" containerRef={containerRef} />
                  <CalloutLine active={hoveredElement === 'copy'} fromId="step3-copy" containerRef={containerRef} />
                  <CalloutLine active={hoveredElement === 'renderer'} fromId="step3-renderer" containerRef={containerRef} />
                  <CalloutLine active={hoveredElement === 'history'} fromId="step3-history" containerRef={containerRef} />
                </div>
              </div>

              {/* Bottom Tooltip Info Area */}
              <div className="mt-8 border-t border-white/5 pt-8 min-h-[120px] flex flex-col items-center text-center">
                <AnimatePresence mode="wait">
                  {hoveredElement ? (
                    <motion.div
                      key={hoveredElement}
                      initial={{ opacity: 0, y: 10 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0, y: -10 }}
                      className="space-y-3 max-w-xl"
                    >
                      <div className="flex items-center justify-center gap-2">
                         <div className="h-[1px] w-4 bg-[#00dbe9]" />
                         <h3 className="text-[#00dbe9] text-xs font-black uppercase tracking-[0.2em]">
                            {tooltips[hoveredElement as keyof typeof tooltips].title}
                         </h3>
                         <div className="h-[1px] w-4 bg-[#00dbe9]" />
                      </div>
                      <p className="text-white/60 text-[13px] leading-relaxed font-medium">
                         {tooltips[hoveredElement as keyof typeof tooltips].desc}
                      </p>
                    </motion.div>
                  ) : (
                    <motion.div
                      key="default"
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      className="space-y-3 max-w-xl"
                    >
                      <div className="flex items-center justify-center gap-2">
                         <div className="h-[1px] w-4 bg-[#00dbe9]" />
                         <h3 className="text-[#00dbe9] text-sm font-black uppercase tracking-[0.2em]">
                            Vox Live HUD
                         </h3>
                         <div className="h-[1px] w-4 bg-[#00dbe9]" />
                      </div>
                      <p className="text-white/60 text-[12px] leading-relaxed font-medium">
                         A system-level transcription overlay that follows your voice. It appears instantly when you speak and disappears when finished, requiring zero context switching.
                      </p>
                      <p className="text-white/40 text-[11px] italic">
                         Hover over HUD elements to explore features...
                      </p>
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      <div className="mt-auto space-y-8">
        {/* Pagination Dots */}
        <div className="flex items-center justify-center gap-3 py-2">
          {[1, 2, 3].map((i) => (
            <button 
              key={i}
              onClick={() => setSubStep(i)}
              className="group relative p-2"
            >
              <motion.div 
                animate={{ 
                    width: subStep === i ? 24 : 8,
                    backgroundColor: subStep === i ? '#00dbe9' : 'rgba(255,255,255,0.1)'
                }}
                className={cn(
                    "h-2 rounded-full transition-all duration-500 ease-out",
                    subStep === i && "shadow-[0_0_15px_rgba(0,219,233,0.5)]"
                )}
              />
              {subStep === i && (
                  <motion.div 
                    layoutId="dot-glow"
                    className="absolute inset-0 bg-[#00dbe9]/20 blur-md rounded-full"
                  />
              )}
            </button>
          ))}
        </div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.25, duration: 0.4 }}
        >
          <button
            onClick={onNext}
            className="group relative w-full py-5 bg-zinc-950 text-white font-black rounded-2xl overflow-hidden border border-white/10 transition-all hover:bg-zinc-900 hover:border-[#00dbe9]/50 active:scale-[0.98] shadow-[0_0_40px_rgba(0,0,0,0.5)]"
          >
            <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
            <span className="relative z-10 flex items-center justify-center gap-4 uppercase tracking-[0.4em] text-[11px]">
              Begin Setup
              <ArrowRight className="w-4 h-4 transition-transform group-hover:translate-x-1 text-[#00dbe9]" />
            </span>
          </button>
        </motion.div>
      </div>
    </div>
  );
};

const FeatureCard = ({ icon, title, desc }: { icon: React.ReactNode, title: string, desc: string }) => (
  <div className="p-5 bg-white/[0.02] border border-white/5 rounded-2xl hover:bg-white/[0.04] transition-all hover:border-white/10 group">
    <div className="mb-3 w-8 h-8 rounded-lg bg-white/5 flex items-center justify-center group-hover:bg-white/10 transition-colors">
      {icon}
    </div>
    <div className="text-[11px] font-bold text-white/80 tracking-widest uppercase mb-1">{title}</div>
    <div className="text-white text-sm font-medium">{desc}</div>
  </div>
);

const tooltips = {
  status: { title: "Live Status", desc: "Passive VAD detection shows when Vox is actively listening to your environment." },
  mic: { title: "Push-To-Talk", desc: "Override passive listening for absolute control. Perfect for high-precision input in crowded environments." },
  copy: { title: "Instant Copy", desc: "One-click to move the finalized transcript to your clipboard for any application." },
  history: { title: "Ephemeral History", desc: "Quickly browse the last 10 transcripts without leaving your current window." },
  renderer: { title: "Fluid Streaming", desc: "Transcripts stream character-by-character with sub-50ms latency." }
};

const CalloutLine = ({ active, fromId, containerRef }: { active: boolean, fromId: string, containerRef: React.RefObject<HTMLDivElement | null> }) => {
  const [coords, setCoords] = useState<{ x1: number, y1: number, x2: number, y2: number } | null>(null);

  React.useEffect(() => {
    if (active && containerRef.current) {
      const fromEl = document.getElementById(fromId);
      const containerRect = containerRef.current.getBoundingClientRect();
      
      if (fromEl) {
        const fromRect = fromEl.getBoundingClientRect();
        
        // Tooltip target point is centered at the bottom of the container
        // We target the top of the bottom info area
        setCoords({
          x1: fromRect.left + fromRect.width / 2 - containerRect.left,
          y1: fromRect.top + fromRect.height / 2 - containerRect.top,
          x2: containerRect.width / 2, // Centered X
          y2: containerRect.height - 110 // Targeted Y in the bottom area
        });
      }
    } else {
      setCoords(null);
    }
  }, [active, fromId, containerRef]);

  if (!coords) return null;

  return (
    <svg className="absolute inset-0 w-full h-full">
      <motion.path
        d={`M ${coords.x1} ${coords.y1} L ${coords.x2} ${coords.y2}`}
        stroke="#00dbe9"
        strokeWidth="1.5"
        strokeDasharray="4 4"
        fill="none"
        initial={{ pathLength: 0, opacity: 0 }}
        animate={{ pathLength: 1, opacity: 0.3 }}
        transition={{ duration: 0.3 }}
      />
      <motion.circle
        cx={coords.x1}
        cy={coords.y1}
        r="3"
        fill="#00dbe9"
        initial={{ scale: 0 }}
        animate={{ scale: 1 }}
      />
      <motion.circle
        cx={coords.x2}
        cy={coords.y2}
        r="3"
        fill="#00dbe9"
        initial={{ scale: 0 }}
        animate={{ scale: 1 }}
      />
    </svg>
  );
};
