import { memo } from "react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export interface SettingsTopologyNode {
  id: string;
  label: string;
  Icon: LucideIcon;
  disabled?: boolean;
}

interface SettingsTopologyMapProps {
  nodes: SettingsTopologyNode[];
  activeSubTab: string;
  onChangeSubTab: (id: string) => void;
  layoutMode?: "full-max" | "full-min" | "small";
}

export const SettingsTopologyMap = memo(
  ({
    nodes,
    activeSubTab,
    onChangeSubTab,
    layoutMode,
  }: SettingsTopologyMapProps) => {
    return (
      <div
        className={cn(
          "gap-1 shrink-0 p-1 rounded-xl glass overflow-visible mb-2.5 bg-[rgba(var(--foreground),0.02)]",
          layoutMode === "small"
            ? "flex overflow-x-auto snap-x no-scrollbar scrollbar-none w-full scroll-smooth"
            : "flex items-center justify-around"
        )}
      >
        {nodes.map(({ id, label, Icon, disabled }) => {
          const isActive = activeSubTab === id;

          return (
            <button
              key={id}
              type="button"
              disabled={disabled}
              onClick={() => !disabled && onChangeSubTab(id)}
              className={cn(
                "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden cursor-pointer flex-1",
                isActive
                  ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                  : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
                disabled && "opacity-40 cursor-not-allowed",
                layoutMode === "small" && "min-w-[75px] snap-center py-1.5 px-1"
              )}
            >
              <Icon
                size={16}
                className={cn(
                  "transition-colors shrink-0",
                  isActive
                    ? "text-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]"
                )}
              />
              <span
                className={cn(
                  "text-[10.5px] font-bold tracking-tight truncate max-w-full leading-tight select-none",
                  isActive
                    ? "text-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground))] group-hover:text-[rgb(var(--foreground))]"
                )}
              >
                {label}
              </span>
            </button>
          );
        })}
      </div>
    );
  }
);

SettingsTopologyMap.displayName = "SettingsTopologyMap";
