import React, { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ArrowRight } from 'lucide-react';
import { VoxOrb } from '@/shared/components/home';
import { MOBILE_ONBOARDING_COPY, WIZARD_CTA_LABELS } from '@/data/welcomeCopy';

interface MobileHeroStepProps {
  onNext: () => void;
}

export const MobileHeroStep: React.FC<MobileHeroStepProps> = ({ onNext }) => {
  const [slideIndex, setSlideIndex] = useState(0);

  const copy = MOBILE_ONBOARDING_COPY.hero;

  const slides = [
    {
      title: copy.slide1Title,
      subtitle: copy.slide1Body,
      orbState: 'Thinking' as const,
    },
    {
      title: copy.slide2Title,
      subtitle: copy.slide2Body,
      orbState: 'Ready' as const,
    },
    {
      title: copy.slide3Title,
      subtitle: copy.slide3Body,
      orbState: 'Speaking' as const,
    },
  ];

  const currentSlide = slides[slideIndex];

  const handleAdvance = () => {
    if (slideIndex < slides.length - 1) {
      setSlideIndex((prev) => prev + 1);
    } else {
      onNext();
    }
  };

  return (
    <div className="flex flex-col h-full overflow-hidden select-none">
      {/* Top Header */}
      <div className="px-5 pt-3 pb-1 shrink-0 flex items-center justify-between">
        <span className="text-[11px] font-mono font-semibold tracking-wider text-[rgb(var(--accent))] uppercase">
          {copy.kicker}
        </span>
        <button
          onClick={onNext}
          className="text-xs font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] uppercase tracking-wider py-1 px-2 focus:outline-none"
        >
          {WIZARD_CTA_LABELS.skip}
        </button>
      </div>

      {/* Scrollable Center Body */}
      <div className="flex-1 min-h-0 overflow-y-auto px-5 py-2 flex flex-col items-center justify-center space-y-4">
        {/* Dynamic Responsive Orb */}
        <div className="relative w-40 h-40 sm:w-48 sm:h-48 flex items-center justify-center shrink-0">
          <div className="w-full h-full">
            <VoxOrb interactionState={currentSlide.orbState} />
          </div>
          <div className="absolute inset-0 bg-[rgb(var(--accent))]/15 blur-[60px] rounded-full pointer-events-none" />
        </div>

        {/* Text Story Section */}
        <div className="w-full max-w-xs text-center space-y-2 min-h-[90px] flex flex-col justify-center">
          <AnimatePresence mode="wait">
            <motion.div
              key={slideIndex}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={{ duration: 0.2 }}
              className="space-y-1.5"
            >
              <h2 className="text-xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
                {currentSlide.title}
              </h2>
              <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed">
                {currentSlide.subtitle}
              </p>
            </motion.div>
          </AnimatePresence>
        </div>

        {/* Slide Indicators */}
        <div className="flex items-center justify-center gap-1.5 pt-1">
          {slides.map((_, i) => (
            <button
              key={i}
              onClick={() => setSlideIndex(i)}
              className="p-1 focus:outline-none"
              aria-label={`Go to slide ${i + 1}`}
            >
              <div
                className={`h-1 rounded-full transition-all duration-300 ${
                  slideIndex === i
                    ? 'w-5 bg-[rgb(var(--accent))]'
                    : 'w-1.5 bg-[rgba(var(--foreground),0.15)]'
                }`}
              />
            </button>
          ))}
        </div>
      </div>

      {/* Sticky Bottom Actions */}
      <div className="px-5 py-4 shrink-0 border-t border-[rgba(var(--foreground),0.06)] bg-[rgb(var(--background))]/95 backdrop-blur-md">
        <button
          onClick={handleAdvance}
          className="w-full py-3.5 min-h-[48px] bg-[rgb(var(--accent))] text-black font-display font-bold text-xs uppercase tracking-wider rounded-xl flex items-center justify-center gap-2 shadow-[0_4px_20px_rgba(var(--accent),0.3)] active:scale-[0.98] transition-transform"
        >
          {slideIndex < slides.length - 1 ? WIZARD_CTA_LABELS.proceed : copy.getStarted}
          <ArrowRight className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
};
