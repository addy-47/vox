import { memo } from "react";
import { ChevronLeft, ChevronRight, Brain, Server, Cloud } from "lucide-react";
import { cn } from "@/shared/lib/utils";

interface CategorySelectorProps {
  activeCategory: "STT" | "LLM" | "TTS";
  activePill: "local" | "remote" | "cloud";
  onCycleCategory: () => void;
  onSetCategory?: (category: "STT" | "LLM" | "TTS") => void;
  onPillChange: (pill: "local" | "remote" | "cloud") => void;
  layoutMode?: "full-max" | "full-min" | "small";
}

const CATEGORIES: Array<{ id: "STT" | "LLM" | "TTS"; label: string }> = [
  { id: "STT", label: "Speech to Text" },
  { id: "LLM", label: "Reasoning" },
  { id: "TTS", label: "Speech" },
];

const PILLS = [
  { id: "local" as const, label: "Embedded", icon: Brain },
  { id: "remote" as const, label: "Server", icon: Server },
  { id: "cloud" as const, label: "Cloud", icon: Cloud },
];

export const CategorySelector = memo(
  ({
    activeCategory,
    activePill,
    onCycleCategory,
    onSetCategory,
    onPillChange,
    layoutMode,
  }: CategorySelectorProps) => {
    const handlePrev = (e: React.MouseEvent) => {
      e.stopPropagation();
      const currentIndex = CATEGORIES.findIndex((c) => c.id === activeCategory);
      const prevIndex = (currentIndex - 1 + CATEGORIES.length) % CATEGORIES.length;
      if (onSetCategory) {
        onSetCategory(CATEGORIES[prevIndex].id);
      } else {
        onCycleCategory();
      }
    };

    const handleNext = (e: React.MouseEvent) => {
      e.stopPropagation();
      const currentIndex = CATEGORIES.findIndex((c) => c.id === activeCategory);
      const nextIndex = (currentIndex + 1) % CATEGORIES.length;
      if (onSetCategory) {
        onSetCategory(CATEGORIES[nextIndex].id);
      } else {
        onCycleCategory();
      }
    };

    const currentCatObj = CATEGORIES.find((c) => c.id === activeCategory) || CATEGORIES[0];

    return (
      <div className="shrink-0 flex flex-col w-full">
        <div className="flex items-center justify-between gap-x-1 w-full pt-1 pb-2 shrink-0 px-0.5">
          {/* Left: Interactive Category Carousel with Left & Right Chevrons */}
          <div className="flex items-center gap-1 shrink-0 pr-0.5 sm:pr-1 select-none">
            <button
              type="button"
              onClick={handlePrev}
              className="p-1 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-colors cursor-pointer flex items-center justify-center"
              title="Previous category"
              aria-label="Previous category"
            >
              <ChevronLeft size={13} className="shrink-0" />
            </button>

            <span
              key={activeCategory}
              className="text-[12px] sm:text-[13px] font-black tracking-wider uppercase text-[rgb(var(--accent))] transition-opacity duration-150 animate-fade-in px-1"
            >
              {currentCatObj.label}
            </span>

            <button
              type="button"
              onClick={handleNext}
              className="p-1 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-colors cursor-pointer flex items-center justify-center"
              title="Next category"
              aria-label="Next category"
            >
              <ChevronRight size={13} className="shrink-0" />
            </button>
          </div>

          {/* Center Connector: Crisp, clean straight arrow extending across the remaining space */}
          <div className="flex flex-1 items-center px-1 min-w-[8px] pointer-events-none select-none overflow-hidden">
            <svg
              className="w-full h-2.5 sm:h-3 text-[rgb(var(--accent))]/50 overflow-visible"
              viewBox="0 0 100 12"
              preserveAspectRatio="none"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <line
                x1="0"
                y1="6"
                x2="97"
                y2="6"
                stroke="currentColor"
                strokeWidth="1.25"
                strokeLinecap="round"
                vectorEffect="non-scaling-stroke"
              />
              <path
                d="M 92 2.5 L 98.5 6 L 92 9.5"
                stroke="currentColor"
                strokeWidth="1.25"
                strokeLinecap="round"
                strokeLinejoin="round"
                vectorEffect="non-scaling-stroke"
              />
            </svg>
          </div>

          {/* Right: Local | Remote | Cloud Pills */}
          <div className="flex items-center gap-1.5 sm:gap-2.5 shrink-0 pl-0.5 sm:pl-1">
            {PILLS.map((mode, idx, arr) => {
              const isActive = activePill === mode.id;
              const IconComponent = mode.icon;
              return (
                <div key={mode.id} className="flex items-center gap-1.5 sm:gap-2.5">
                  <button
                    type="button"
                    onClick={() => onPillChange(mode.id)}
                    className={cn(
                      "flex items-center justify-center gap-1 pb-0.5 sm:pb-1 border-b-2 transition-all duration-200 bg-transparent text-[11px] sm:text-[12px] font-black uppercase tracking-[0.08em] sm:tracking-[0.12em] outline-none cursor-pointer",
                      isActive
                        ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                        : "text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    <span>{mode.label}</span>
                    {layoutMode !== "small" && <IconComponent size={10} className="shrink-0 hidden md:inline-block" />}
                  </button>
                  {idx < arr.length - 1 && (
                    <span className="text-[11px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/25 font-light select-none pb-0.5 sm:pb-1">
                      |
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      </div>
    );
  }
);

CategorySelector.displayName = "CategorySelector";
