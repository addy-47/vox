import { memo } from "react";
import { Target, Plus, Minus, RefreshCw, MousePointerClick } from "lucide-react";
import { Tooltip } from "@/shared/ui/Tooltip";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";

interface GraphControlDockProps {
  onRecenter: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onRefresh: () => void;
  onFocusCore?: () => void;
  refreshing?: boolean;
  selectModeEnabled?: boolean;
  onToggleSelectMode?: () => void;
}

export const GraphControlDock = memo(
  ({
    onRecenter,
    onZoomIn,
    onZoomOut,
    onRefresh,
    refreshing = false,
    selectModeEnabled = false,
    onToggleSelectMode,
  }: GraphControlDockProps) => {
    return (
      <aside
        aria-label="Graph Navigation Controls"
        className="fixed right-2 sm:right-5 top-1/2 -translate-y-1/2 z-30 flex flex-col items-center gap-1 sm:gap-1.5 p-1 sm:p-1.5 rounded-xl sm:rounded-2xl bg-[rgba(var(--card),0.75)] backdrop-blur-2xl border border-[rgba(var(--border),0.14)] shadow-2xl pointer-events-auto transition-all"
      >
        {/* Recenter View */}
        <Tooltip label={MEMORY_COPY.recenterView} shortcutId="memory.recenter" side="left">
          <button
            type="button"
            onClick={onRecenter}
            aria-label={MEMORY_COPY.recenterView}
            className="p-1.5 sm:p-2.5 rounded-lg sm:rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] active:scale-95 transition-all cursor-pointer"
          >
            <Target className="w-3.5 h-3.5 sm:w-4 sm:h-4" />
          </button>
        </Tooltip>

        {/* Toggle Node Selection Mode */}
        {onToggleSelectMode && (
          <Tooltip
            label={selectModeEnabled ? MEMORY_COPY.selectModeActive : MEMORY_COPY.selectModeInactive}
            side="left"
          >
            <button
              type="button"
              onClick={onToggleSelectMode}
              aria-label={selectModeEnabled ? MEMORY_COPY.selectModeActive : MEMORY_COPY.selectModeInactive}
              aria-pressed={selectModeEnabled}
              className={cn(
                "p-1.5 sm:p-2.5 rounded-lg sm:rounded-xl transition-all cursor-pointer active:scale-95",
                selectModeEnabled
                  ? "bg-[rgba(var(--accent),0.2)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.45)] shadow-[0_0_12px_rgba(var(--accent),0.3)]"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)]"
              )}
            >
              <MousePointerClick className="w-3.5 h-3.5 sm:w-4 sm:h-4" />
            </button>
          </Tooltip>
        )}
        {/* Refresh Facts */}
        <Tooltip label={MEMORY_COPY.refresh} side="left">
          <button
            type="button"
            onClick={onRefresh}
            disabled={refreshing}
            aria-label={MEMORY_COPY.refresh}
            className={cn(
              "p-1.5 sm:p-2.5 rounded-lg sm:rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] active:scale-95 transition-all cursor-pointer disabled:opacity-50",
              refreshing && "cursor-wait"
            )}
          >
            <RefreshCw
              className={cn("w-3.5 h-3.5 sm:w-4 sm:h-4", refreshing && "animate-spin text-[rgb(var(--accent))]")}
            />
          </button>
        </Tooltip>
    
        {/* Subtle Divider */}
        <div className="w-4 sm:w-5 h-[1px] bg-[rgba(var(--border),0.12)] my-0.5" />

        {/* Zoom In */}
        <Tooltip label={MEMORY_COPY.zoomIn} shortcutId="memory.zoomIn" side="left">
          <button
            type="button"
            onClick={onZoomIn}
            aria-label={MEMORY_COPY.zoomIn}
            className="p-1.5 sm:p-2.5 rounded-lg sm:rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] active:scale-95 transition-all cursor-pointer"
          >
            <Plus className="w-3.5 h-3.5 sm:w-4 sm:h-4" />
          </button>
        </Tooltip>

        {/* Zoom Out */}
        <Tooltip label={MEMORY_COPY.zoomOut} shortcutId="memory.zoomOut" side="left">
          <button
            type="button"
            onClick={onZoomOut}
            aria-label={MEMORY_COPY.zoomOut}
            className="p-1.5 sm:p-2.5 rounded-lg sm:rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] active:scale-95 transition-all cursor-pointer"
          >
            <Minus className="w-3.5 h-3.5 sm:w-4 sm:h-4" />
          </button>
        </Tooltip>

      </aside>
    );
  }
);
GraphControlDock.displayName = "GraphControlDock";
