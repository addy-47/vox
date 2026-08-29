import { memo } from "react";
import { Brain, Server, Cloud, ChevronRight } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui";

export interface ProviderOption {
  id: "local" | "remote" | "cloud";
  label: string;
  sublabel: string;
  icon: typeof Brain;
}

interface ProviderSelectorViewProps {
  activeCategory: "STT" | "LLM" | "TTS";
  activePill: "local" | "remote" | "cloud";
  onSelectProvider: (pill: "local" | "remote" | "cloud") => void;
  layoutMode?: "full-max" | "full-min" | "small";
}

const PROVIDER_OPTIONS: Record<"STT" | "LLM" | "TTS", ProviderOption[]> = {
  STT: [
    {
      id: "local",
      label: "Embedded",
      sublabel: "Local Inference",
      icon: Brain,
    },
    {
      id: "remote",
      label: "Server",
      sublabel: "Remote Server",
      icon: Server,
    },
    {
      id: "cloud",
      label: "Cloud",
      sublabel: "Speech API",
      icon: Cloud,
    },
  ],
  LLM: [
    {
      id: "local",
      label: "Embedded",
      sublabel: "On-Device GGUF",
      icon: Brain,
    },
    {
      id: "remote",
      label: "Server",
      sublabel: "Ollama / Remote",
      icon: Server,
    },
    {
      id: "cloud",
      label: "Cloud",
      sublabel: "Direct Provider API",
      icon: Cloud,
    },
  ],
  TTS: [
    {
      id: "local",
      label: "Embedded",
      sublabel: "Neural Voice",
      icon: Brain,
    },
    {
      id: "remote",
      label: "Server",
      sublabel: "Chatterbox GPU",
      icon: Server,
    },
    {
      id: "cloud",
      label: "Cloud",
      sublabel: "Cloud Voices",
      icon: Cloud,
    },
  ],
};

export const ProviderSelectorView = memo(
  ({
    activeCategory,
    activePill,
    onSelectProvider,
    layoutMode,
  }: ProviderSelectorViewProps) => {
    const options = PROVIDER_OPTIONS[activeCategory];

    return (
      <div
        className="grid grid-cols-3 gap-2 sm:gap-2.5 w-full my-auto py-1 px-0.5 items-stretch animate-fade-in"
      >
        {options.map((option) => {
          const isActive = activePill === option.id;
          const IconComponent = option.icon;

          return (
            <Tooltip
              key={option.id}
              label={
                isActive
                  ? "Active provider — Click to configure parameters"
                  : `Click to select and configure ${option.label}`
              }
              side="top"
              wrapperClassName="w-full flex-1"
            >
              <button
                type="button"
                onClick={() => onSelectProvider(option.id)}
                className={cn(
                  "w-full rounded-xl border text-center transition-all duration-200 cursor-pointer flex flex-col items-center justify-center group relative overflow-hidden",
                  layoutMode === "small" ? "py-2 px-1 gap-0.5" : "py-2.5 px-2 gap-1",
                  isActive
                    ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30 shadow-[0_0_12px_rgba(var(--accent),0.12)]"
                    : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.08)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.03)]"
                )}
              >
                {/* Top Right Corner Drilldown Arrow (Colored when active) */}
                <ChevronRight
                  size={layoutMode === "small" ? 11 : 12}
                  className={cn(
                    "absolute top-1.5 right-1.5 transition-all shrink-0",
                    isActive
                      ? "text-[rgb(var(--accent))]"
                      : "text-[rgb(var(--foreground-muted))]/30 group-hover:text-[rgb(var(--accent))] group-hover:translate-x-0.5"
                  )}
                />

                {/* Center 1: Icon */}
                <div
                  className={cn(
                    "rounded-md flex items-center justify-center transition-colors shrink-0",
                    layoutMode === "small" ? "w-5 h-5" : "w-6 h-6",
                    isActive
                      ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))]"
                      : "bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--accent))] group-hover:bg-[rgb(var(--accent))]/10"
                  )}
                >
                  <IconComponent size={layoutMode === "small" ? 11.5 : 13} />
                </div>

                {/* Center 2: Title */}
                <span
                  className={cn(
                    "font-display font-bold text-[rgb(var(--foreground))] truncate w-full px-0.5 leading-tight",
                    layoutMode === "small" ? "text-[10.5px]" : "text-[11.5px] sm:text-[12px]"
                  )}
                >
                  {option.label}
                </span>

                {/* Center 3: Subtext (hidden in small layout) */}
                {layoutMode !== "small" && (
                  <span className="font-medium text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--foreground))]/80 transition-colors truncate w-full px-0.5 leading-tight text-[9.5px] sm:text-[10px]">
                    {option.sublabel}
                  </span>
                )}
              </button>
            </Tooltip>
          );
        })}
      </div>
    );
  }
);

ProviderSelectorView.displayName = "ProviderSelectorView";
