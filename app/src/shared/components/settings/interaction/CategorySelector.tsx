import { memo, useCallback } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";

interface CategorySelectorProps {
  activeCategory: "STT" | "LLM" | "TTS";
  onSetCategory: (category: "STT" | "LLM" | "TTS") => void;
  layoutMode?: "full-max" | "full-min" | "small";
}

const STAGES: Array<{
  id: "STT" | "LLM" | "TTS";
  label: string;
  sublabel: string;
}> = [
  { id: "STT", label: "Listening", sublabel: "Speech Recognition" },
  { id: "LLM", label: "Reasoning", sublabel: "Language Model" },
  { id: "TTS", label: "Speaking", sublabel: "Voice Synthesis" },
];

export const CategorySelector = memo(
  ({ activeCategory, onSetCategory }: CategorySelectorProps) => {
    const isCategoryDirty = useSettingsStore((s) => s.isCategoryDirty);

    const currentIndex = STAGES.findIndex((s) => s.id === activeCategory);
    const currentStage = STAGES[currentIndex >= 0 ? currentIndex : 1];
    const isDirty = isCategoryDirty(currentStage.id.toLowerCase());

    const handlePrev = useCallback(() => {
      const prevIdx = (currentIndex - 1 + STAGES.length) % STAGES.length;
      onSetCategory(STAGES[prevIdx].id);
    }, [currentIndex, onSetCategory]);

    const handleNext = useCallback(() => {
      const nextIdx = (currentIndex + 1) % STAGES.length;
      onSetCategory(STAGES[nextIdx].id);
    }, [currentIndex, onSetCategory]);

    return (
      <div className="w-full flex items-center justify-between py-1 px-0.5 select-none mb-2.5 shrink-0">
        <button
          type="button"
          onClick={handlePrev}
          className="p-1 rounded-lg text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-colors active:scale-90 cursor-pointer flex items-center justify-center shrink-0"
          title="Previous stage"
          aria-label="Previous stage"
        >
          <ChevronLeft size={16} />
        </button>

        <div className="flex items-center justify-center gap-1.5 sm:gap-2 min-w-0 px-2 flex-1 animate-fade-in leading-none">
          <span className="font-display font-black text-[12px] sm:text-[12.5px] uppercase tracking-[0.15em] text-[rgb(var(--foreground))] leading-none">
            {currentStage.label}
          </span>
          <span className="text-[10.5px] sm:text-[11px] font-medium text-[rgb(var(--foreground-muted))]/65 leading-none truncate -translate-y-[0.5px]">
            ({currentStage.sublabel})
          </span>
          {isDirty && (
            <span
              title="Unsaved changes in this stage"
              className="w-1.5 h-1.5 rounded-full bg-amber-400 shadow-[0_0_6px_rgba(251,191,36,0.8)] shrink-0 ml-0.5"
            />
          )}
        </div>

        <button
          type="button"
          onClick={handleNext}
          className="p-1 rounded-lg text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-colors active:scale-90 cursor-pointer flex items-center justify-center shrink-0"
          title="Next stage"
          aria-label="Next stage"
        >
          <ChevronRight size={16} />
        </button>
      </div>
    );
  }
);

CategorySelector.displayName = "CategorySelector";
