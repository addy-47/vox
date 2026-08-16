import { memo } from "react";
import { RotateCw, Brain, Server, Cloud, MoveRight } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";

interface CategorySelectorProps {
  activeCategory: "STT" | "LLM" | "TTS";
  activePill: "local" | "remote" | "cloud";
  onCycleCategory: () => void;
  onPillChange: (pill: "local" | "remote" | "cloud") => void;
  layoutMode?: "full-max" | "full-min" | "small";
}

const PILLS = [
  { id: "local" as const, label: "Embedded", icon: Brain },
  { id: "remote" as const, label: "Server", icon: Server },
  { id: "cloud" as const, label: "Cloud", icon: Cloud },
];

export const CategorySelector = memo(
  ({ activeCategory, activePill, onCycleCategory, onPillChange, layoutMode }: CategorySelectorProps) => {
    return (
      <div className="shrink-0 flex flex-col gap-2 w-full mt-1.5">
        <div className="flex flex-wrap items-center justify-between gap-2 w-full pb-2 pt-1 shrink-0 px-2 sm:px-3">
          <div className="flex items-center gap-2 shrink-0">
            {/* Cycling Category Button with Interactive Rotate Indicator */}
            <Tooltip label="Click to switch between hearing, thinking, and speaking">
              <button
                type="button"
                onClick={onCycleCategory}
                className="flex items-center gap-1.5 pb-1 bg-transparent transition-all duration-200 text-[13px] sm:text-[14px] font-black tracking-wider uppercase text-[rgb(var(--accent))] border-b-2 border-transparent outline-none active:scale-95 select-none cursor-pointer group"
              >
                <div className="p-1 rounded-md bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.2)] text-[rgb(var(--accent))] group-hover:bg-[rgba(var(--accent),0.2)] transition-all flex items-center justify-center">
                  <RotateCw size={12} className="shrink-0 transition-transform duration-300 group-hover:rotate-180" />
                </div>
                <span>{activeCategory}</span>
              </button>
            </Tooltip>

            {/* Clean MoveRight separator icon */}
            <MoveRight size={13} className="text-[rgb(var(--accent))]/70 shrink-0 select-none hidden sm:inline-block" />
          </div>

          {/* Local | Remote | Cloud Pills */}
          <div className="flex flex-wrap items-center gap-2 sm:gap-2.5">
            {PILLS.map((mode, idx, arr) => {
              const isActive = activePill === mode.id;
              const IconComponent = mode.icon;
              return (
                <div key={mode.id} className="flex items-center gap-2 sm:gap-2.5">
                  <button
                    type="button"
                    onClick={() => onPillChange(mode.id)}
                    className={cn(
                      "flex items-center justify-center gap-1 pb-1 border-b-2 transition-all duration-200 bg-transparent text-[11px] sm:text-[11px] font-black uppercase tracking-[0.12em] outline-none cursor-pointer",
                      isActive
                        ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                        : "text-[rgb(var(--foreground-muted))]/50 border-transparent hover:text-[rgb(var(--foreground-muted))]/80"
                    )}
                  >
                    <span>{mode.label}</span>
                    {layoutMode !== "small" && <IconComponent size={10} className="shrink-0 hidden sm:inline-block" />}
                  </button>
                  {idx < arr.length - 1 && (
                    <span className="text-[11px] text-[rgb(var(--foreground-muted))]/20 font-light select-none pb-1">
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
