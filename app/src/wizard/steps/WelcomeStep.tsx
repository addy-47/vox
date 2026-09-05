import React, { useState, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ArrowRight, Zap, Activity, ChevronLeft, ChevronRight, Copy, X, Mic } from 'lucide-react';
import { cn } from '@/shared/lib/utils';
import { VoxOrb } from '@/shared/components/home';
import {
  WELCOME_SUBSTEPS,
  WELCOME_FEATURE_CARDS,
  WELCOME_TOOLTIPS,
  WELCOME_DEMO_DEFAULT,
  WIZARD_CTA_LABELS,
} from '@/data/welcomeCopy';
import { TRAY_COPY } from '@/data/trayCopy';

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
    <div className="flex flex-col h-full relative" ref={containerRef}>
      <header className="mb-8">
        <div className="flex items-center justify-between mb-2">
          <div className="flex items-center gap-4">
            <div className="h-[1px] w-8 bg-[rgb(var(--accent))]/30" />
            <span className="text-[12px] font-black tracking-[0.4em] text-[rgb(var(--accent))] uppercase">Step 1 of 6 · Getting Started</span>
          </div>
          
            {/* Sub-step Navigation Controls */}
          <div className="flex items-center gap-2">
            <button 
              onClick={prevSubStep}
              disabled={subStep === 1}
              className="p-2 rounded-full glass border border-[rgba(var(--border),0.06)] hover:bg-[rgba(var(--foreground),0.1)] disabled:opacity-20 transition-all"
            >
              <ChevronLeft className="w-4 h-4 text-[rgb(var(--foreground-muted))]" />
            </button>
            <button 
              onClick={nextSubStep}
              disabled={subStep === 3}
              className="p-2 rounded-full glass border border-[rgba(var(--border),0.06)] hover:bg-[rgba(var(--foreground),0.1)] disabled:opacity-20 transition-all"
            >
              <ChevronRight className="w-4 h-4 text-[rgb(var(--foreground-muted))]" />
            </button>
          </div>
        </div>

        <AnimatePresence mode="wait">
          {subStep === 1 && (
            <motion.div
              key="header1"
              initial={{ opacity: 0, x: 10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -10 }}
            >
              <h1 className="text-4xl font-display font-black text-[rgb(var(--foreground))] tracking-tighter uppercase mb-4">
                {WELCOME_SUBSTEPS[0].title}
              </h1>
              <p className="text-[rgb(var(--foreground-muted))] text-sm leading-relaxed max-w-md">
                {WELCOME_SUBSTEPS[0].tagline}
              </p>
            </motion.div>
          )}
          {subStep === 2 && (
            <motion.div
              key="header2"
              initial={{ opacity: 0, x: 10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -10 }}
            >
              <h1 className="text-4xl font-display font-black text-[rgb(var(--foreground))] tracking-tighter uppercase mb-4">
                {WELCOME_SUBSTEPS[1].title}
              </h1>
              <p className="text-[rgb(var(--foreground-muted))] text-sm leading-relaxed max-w-md">
                {WELCOME_SUBSTEPS[1].tagline}
              </p>
            </motion.div>
          )}
          {subStep === 3 && (
            <motion.div
              key="header3"
              initial={{ opacity: 0, x: 10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -10 }}
            >
              <h1 className="text-4xl font-display font-black text-[rgb(var(--foreground))] tracking-tighter uppercase mb-4">
                {WELCOME_SUBSTEPS[2].title}
              </h1>
              <p className="text-[rgb(var(--foreground-muted))] text-sm leading-relaxed max-w-md">
                {WELCOME_SUBSTEPS[2].tagline}
              </p>
            </motion.div>
          )}
        </AnimatePresence>
      </header>

      <div className="flex-1 flex flex-col min-h-0">
        <AnimatePresence mode="wait">
          {subStep === 1 && (
            <motion.div 
              key="step1"
              initial={{ opacity: 0, scale: 0.98 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.98 }}
              className="flex-1 flex flex-col justify-center"
            >
              <div className="grid grid-cols-2 gap-4">
                {WELCOME_FEATURE_CARDS.map((card, i) => {
                  const Icon = card.icon;
                  return (
                    <FeatureCard
                      key={card.title}
                      icon={<Icon className={FEATURE_CARD_ICON_CLASS[i]} />}
                      title={card.title}
                      desc={card.desc}
                    />
                  );
                })}
              </div>
            </motion.div>
          )}

          {subStep === 2 && (
            <motion.div 
              key="step2"
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 1.05 }}
              className="flex-1 flex flex-col items-center justify-center text-center py-6"
            >
              <div className="relative w-64 h-64 mb-8 flex items-center justify-center">
                <div className="w-full h-full">
                   <VoxOrb interactionState="Thinking" />
                </div>
                <div className="absolute inset-0 bg-[rgb(var(--accent))]/10 blur-[100px] rounded-full pointer-events-none" />
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
            >
              <div className="flex-1 flex flex-col items-center justify-center relative">
                <div className="w-[400px] h-[200px] glass-card rounded-2xl border-[rgba(var(--accent),0.15)] overflow-hidden flex flex-col text-left relative z-10">
                  <div className="px-6 py-4 flex items-center justify-between relative z-10 border-b border-[rgba(var(--foreground),0.08)]">
                    <div 
                      className="flex items-center gap-3 cursor-help group/status"
                      onMouseEnter={() => setHoveredElement('status')}
                      onMouseLeave={() => setHoveredElement(null)}
                      id="step3-status"
                    >
                      <div className="relative flex items-center justify-center">
                        <div className="absolute w-5 h-5 rounded-full bg-[rgb(var(--accent))] blur-md opacity-40 animate-pulse" />
                        <div className="w-2.5 h-2.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.8)] z-10" />
                      </div>
                      <span className="text-[12px] font-black tracking-[0.4em] text-[rgb(var(--foreground))]/70 uppercase">
                        {TRAY_COPY.brand} <span className="text-[rgb(var(--accent))]">{TRAY_COPY.live}</span>
                      </span>
                    </div>
                    <div className="flex items-center gap-2">
                       <div 
                         className="p-2 rounded-lg bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] cursor-help"
                         onMouseEnter={() => setHoveredElement('mic')}
                         onMouseLeave={() => setHoveredElement(null)}
                         id="step3-mic"
                       >
                          <Mic size={16} />
                       </div>
                       <Copy 
                         className="w-4 h-4 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-colors cursor-help p-2 box-content rounded-lg hover:bg-[rgba(var(--foreground),0.05)]" 
                         onMouseEnter={() => setHoveredElement('copy')}
                         onMouseLeave={() => setHoveredElement(null)}
                         id="step3-copy"
                       />
                       <X className="w-4 h-4 text-[rgb(var(--foreground-muted))]/40 p-2 box-content" />
                    </div>
                  </div>

                  <div 
                    className="flex-1 px-5 py-4 cursor-help flex flex-col items-center justify-center text-center"
                    onMouseEnter={() => setHoveredElement('renderer')}
                    onMouseLeave={() => setHoveredElement(null)}
                    id="step3-renderer"
                  >
                    <div className="w-full space-y-2">
<div className="text-[18px] leading-snug font-medium tracking-tight text-[rgb(var(--foreground))]/90">
                           {WELCOME_DEMO_DEFAULT.listeningHint}
                           <motion.span
                            animate={{ opacity: [0, 1, 0] }}
                            transition={{ repeat: Infinity, duration: 0.8 }}
                            className="inline-block w-[2px] h-[1em] ml-1 align-middle bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.8)]"
                          />
                       </div>
                    </div>
                  </div>

                  <div 
                    className="px-7 py-4 bg-[rgba(var(--foreground),0.04)] border-t border-[rgba(var(--foreground),0.08)] flex items-center justify-between"
                    onMouseEnter={() => setHoveredElement('history')}
                    onMouseLeave={() => setHoveredElement(null)}
                    id="step3-history"
                  >
                     <div className="flex items-center gap-6 opacity-60">
                        <div className="flex items-center gap-2">
                           <Activity size={12} className="text-[rgb(var(--accent))]" />
                           <span className="text-[12px] font-mono text-[rgb(var(--foreground-muted))]/80 font-bold uppercase tracking-widest">{WELCOME_DEMO_DEFAULT.statsActive}</span>
                        </div>
                        <div className="flex items-center gap-2">
                           <Zap size={12} className="text-[rgb(var(--accent))]" />
                           <span className="text-[12px] font-mono text-[rgb(var(--foreground-muted))]/80 font-bold">42MB</span>
                        </div>
                     </div>
                     <div className="flex items-center gap-1">
                        <div className="p-1.5 rounded-md bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--foreground),0.08)] flex items-center justify-center">
                           <ChevronLeft className="w-3.5 h-3.5 text-[rgb(var(--accent))]" />
                        </div>
                        <div className="p-1.5 rounded-md bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--foreground),0.08)] flex items-center justify-center">
                           <ChevronRight className="w-3.5 h-3.5 text-[rgb(var(--accent))]" />
                        </div>
                     </div>
                  </div>
                </div>

                <div className="absolute inset-0 pointer-events-none z-0">
                  <CalloutLine active={hoveredElement === 'status'} fromId="step3-status" containerRef={containerRef} />
                  <CalloutLine active={hoveredElement === 'mic'} fromId="step3-mic" containerRef={containerRef} />
                  <CalloutLine active={hoveredElement === 'copy'} fromId="step3-copy" containerRef={containerRef} />
                  <CalloutLine active={hoveredElement === 'renderer'} fromId="step3-renderer" containerRef={containerRef} />
                  <CalloutLine active={hoveredElement === 'history'} fromId="step3-history" containerRef={containerRef} />
                </div>
              </div>

              <div className="mt-4 pt-4 min-h-[80px] flex flex-col items-center text-center">
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
                         <div className="h-[1px] w-4 bg-[rgb(var(--accent))]" />
                          <h3 className="text-[rgb(var(--accent))] text-xs font-black uppercase tracking-[0.2em]">
                             {WELCOME_TOOLTIPS[hoveredElement as keyof typeof WELCOME_TOOLTIPS].title}
                          </h3>
                         <div className="h-[1px] w-4 bg-[rgb(var(--accent))]" />
                      </div>
<p className="text-[rgb(var(--foreground-muted))]/80 text-[14px] leading-relaxed font-medium">
                           {WELCOME_TOOLTIPS[hoveredElement as keyof typeof WELCOME_TOOLTIPS].desc}
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
                         <div className="h-[1px] w-4 bg-[rgb(var(--accent))]" />
                          <h3 className="text-[rgb(var(--accent))] text-sm font-black uppercase tracking-[0.2em]">
                             {WELCOME_DEMO_DEFAULT.title}
                          </h3>
                         <div className="h-[1px] w-4 bg-[rgb(var(--accent))]" />
                      </div>
<p className="text-[rgb(var(--foreground-muted))]/70 text-[12px] italic">
                           {WELCOME_DEMO_DEFAULT.desc}
                       </p>
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      <div className="mt-auto space-y-4">
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
                    backgroundColor: subStep === i ? 'rgb(var(--accent))' : 'rgba(var(--foreground),0.15)'
                }}
                className={cn(
                    "h-2 rounded-full transition-all duration-500 ease-out",
                    subStep === i && "shadow-[0_0_15px_rgba(var(--accent),0.5)]"
                )}
              />
              {subStep === i && (
                  <motion.div 
                    layoutId="dot-glow"
                    className="absolute inset-0 bg-[rgb(var(--accent))]/20 blur-md rounded-full"
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
            className="group relative w-full py-5 text-[rgb(var(--foreground))] font-black rounded-2xl overflow-hidden border transition-all active:scale-[0.98] glass-card hover:border-[rgb(var(--accent))]/70"
          >
            <div className="absolute inset-0 bg-gradient-to-r from-[rgb(var(--accent))]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
            <span className="relative z-10 flex items-center justify-center gap-4 uppercase tracking-[0.4em] text-[12px]">
              {WIZARD_CTA_LABELS.beginSetup}
              <ArrowRight className="w-4 h-4 transition-transform group-hover:translate-x-1 text-[rgb(var(--accent))]" />
            </span>
          </button>
        </motion.div>
      </div>
    </div>
  );
};

const FEATURE_CARD_ICON_CLASS = [
  "w-4 h-4 text-[rgb(var(--foreground-muted))]/70",
  "w-4 h-4 text-[rgb(var(--accent))]",
  "w-4 h-4 text-[rgb(var(--accent))]",
  "w-4 h-4",
];

const FeatureCard = ({ icon, title, desc }: { icon: React.ReactNode, title: string, desc: string }) => (
  <div className="glass px-5 py-5 group transition-all duration-500 hover:bg-[rgba(var(--foreground),0.06)]">
    <div className="mb-3 w-8 h-8 rounded-lg bg-[rgba(var(--foreground),0.05)] flex items-center justify-center group-hover:bg-[rgba(var(--foreground),0.1)] transition-colors">
      {icon}
    </div>
    <div className="text-[12px] font-bold text-[rgb(var(--foreground-muted))] tracking-widest uppercase mb-1">{title}</div>
    <div className="text-[rgb(var(--foreground))] text-sm font-medium">{desc}</div>
  </div>
);

const CalloutLine = ({ active, fromId, containerRef }: { active: boolean, fromId: string, containerRef: React.RefObject<HTMLDivElement | null> }) => {
  const [coords, setCoords] = useState<{ x1: number, y1: number, x2: number, y2: number } | null>(null);

  React.useEffect(() => {
    if (active && containerRef.current) {
      const fromEl = document.getElementById(fromId);
      const containerRect = containerRef.current.getBoundingClientRect();
      
      if (fromEl) {
        const fromRect = fromEl.getBoundingClientRect();
        
        setCoords({
          x1: fromRect.left + fromRect.width / 2 - containerRect.left,
          y1: fromRect.top + fromRect.height / 2 - containerRect.top,
          x2: containerRect.width / 2,
          y2: containerRect.height - 110
        });
      }
    } else {
      setCoords(null);
    }
  }, [active, fromId, containerRef]);

  if (!coords) return null;

  return (
    <svg className="absolute inset-0 w-full h-full text-[rgb(var(--accent))]">
      <motion.path
        d={`M ${coords.x1} ${coords.y1} L ${coords.x2} ${coords.y2}`}
        stroke="currentColor"
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
        fill="currentColor"
        initial={{ scale: 0 }}
        animate={{ scale: 1 }}
      />
      <motion.circle
        cx={coords.x2}
        cy={coords.y2}
        r="3"
        fill="currentColor"
        initial={{ scale: 0 }}
        animate={{ scale: 1 }}
      />
    </svg>
  );
};
