import { memo } from "react";
import { Brain, Server, Cloud } from "lucide-react";
import { cn } from "@/shared/lib/utils";

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
      <div className="shrink-0 flex flex-col gap-2 w-full mt-3">
        <div className="flex items-center gap-3 w-full pb-2 pt-1 shrink-0 mt-2 px-3">
          {/* Cycling Category Button */}
          <button
            type="button"
            onClick={onCycleCategory}
            className="flex items-center gap-1 pb-2 -mb-[13px] bg-transparent transition-all duration-200 text-[13px] font-black tracking-wider uppercase text-[rgb(var(--accent))] border-b-2 border-transparent outline-none active:scale-95 select-none cursor-pointer"
          >
            <span>{activeCategory}</span>
          </button>

          {/* Arrow separator */}
          <span className="text-[13px] text-[rgb(var(--accent))]/70 font-black select-none tracking-tighter pb-2 -mb-[10px]">
            ───&gt;
          </span>

          {/* Local | Remote | Cloud Pills */}
          <div className="flex items-center gap-3">
            {PILLS.map((mode, idx, arr) => {
              const isActive = activePill === mode.id;
              const IconComponent = mode.icon;
              return (
                <div key={mode.id} className="flex items-center gap-3">
                  <button
                    type="button"
                    onClick={() => onPillChange(mode.id)}
                    className={cn(
                      "flex items-center justify-center gap-1.5 pb-2 -mb-[10px] border-b-2 transition-all duration-200 bg-transparent text-[10px] font-black uppercase tracking-[0.15em] outline-none cursor-pointer",
                      isActive
                        ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                        : "text-[rgb(var(--foreground-muted))]/50 border-transparent hover:text-[rgb(var(--foreground-muted))]/80"
                    )}
                  >
                    <span>{mode.label}</span>
                    {layoutMode !== "small" && <IconComponent size={11} className="shrink-0" />}
                  </button>
                  {idx < arr.length - 1 && (
                    <span className="text-[10px] text-[rgb(var(--foreground-muted))]/20 font-light select-none pb-2 -mb-[10px]">
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
