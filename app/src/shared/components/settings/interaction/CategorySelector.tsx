import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";
import { CATEGORY_SWITCH_COPY } from "@/data/settingsCopy";

interface CategorySelectorProps {
  activeCategory: "STT" | "LLM" | "TTS";
  onSetCategory: (category: "STT" | "LLM" | "TTS") => void;
  layoutMode?: "full-max" | "full-min" | "small";
}

const TABS: Array<{
  id: "STT" | "LLM" | "TTS";
  labelKey: keyof typeof CATEGORY_SWITCH_COPY.tabs;
}> = [
  { id: "STT", labelKey: "stt" },
  { id: "LLM", labelKey: "llm" },
  { id: "TTS", labelKey: "tts" },
];

export const CategorySelector = memo(
  ({ activeCategory, onSetCategory }: CategorySelectorProps) => {
    const isCategoryDirty = useSettingsStore((s) => s.isCategoryDirty);

    return (
      <div className="w-full flex items-center justify-between pt-0.5 pb-1 shrink-0 border-b border-[rgba(var(--accent),0.08)] mb-2 px-0.5 select-none overflow-x-auto no-scrollbar">
        {TABS.map((tab, idx, arr) => {
          const isActive = activeCategory === tab.id;
          const isDirty = isCategoryDirty(tab.id.toLowerCase());
          const label = CATEGORY_SWITCH_COPY.tabs[tab.labelKey];

          return (
            <div key={tab.id} className="flex-1 min-w-0 flex items-center justify-center">
              <button
                type="button"
                onClick={() => onSetCategory(tab.id)}
                className={cn(
                  "w-full flex items-center justify-center gap-1.5 pb-1 border-b-2 transition-all duration-200 bg-transparent text-[9.5px] sm:text-[10.5px] xl:text-[11px] font-black uppercase tracking-[0.04em] sm:tracking-[0.08em] outline-none cursor-pointer text-center truncate px-0.5",
                  isActive
                    ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]"
                )}
              >
                <span className="truncate">{label}</span>
                {isDirty && (
                  <span
                    title="Unsaved changes in this stage"
                    className="w-1.5 h-1.5 rounded-full bg-amber-400 shadow-[0_0_6px_rgba(251,191,36,0.8)] shrink-0"
                  />
                )}
              </button>
              {idx < arr.length - 1 && (
                <span className="text-[10px] text-[rgb(var(--foreground-muted))]/25 font-light select-none pb-1 shrink-0 px-0.5 sm:px-1">
                  |
                </span>
              )}
            </div>
          );
        })}
      </div>
    );
  }
);

CategorySelector.displayName = "CategorySelector";
