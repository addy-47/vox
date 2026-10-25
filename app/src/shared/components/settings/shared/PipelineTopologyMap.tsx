import { memo, useCallback, useMemo } from "react";
import type { ReactElement } from "react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { DIRTY_STATE_COPY } from "@/data/settingsCopy";

/**
 * The card-level category strip. Generic by design: the speech card passes its
 * three stages, the language card will pass its own.
 */

export interface PipelineTopologyNode<T extends string = string> {
  id: T;
  label: string;
  shortLabel?: string;
  /** One-line explainer under the label, so the stage says what it does. */
  caption?: string;
  Icon: LucideIcon;
  disabled?: boolean;
  isDirty?: boolean;
}

export interface PipelineTopologyMapProps<T extends string = string> {
  activeTab: T;
  onChangeTab: (id: T) => void;
  nodes: PipelineTopologyNode<T>[];
  layoutMode?: "full-max" | "full-min" | "small";
  ariaLabel?: string;
}

function PipelineTopologyMapInner<T extends string = string>({
  activeTab,
  onChangeTab,
  nodes,
  layoutMode,
  ariaLabel,
}: PipelineTopologyMapProps<T>) {
  const ids = useMemo(() => nodes.map((n) => n.id), [nodes]);
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const currentIdx = ids.indexOf(activeTab);
      if (e.key === "ArrowRight") {
        e.preventDefault();
        onChangeTab(ids[(currentIdx + 1) % ids.length]);
      } else if (e.key === "ArrowLeft") {
        e.preventDefault();
        onChangeTab(ids[(currentIdx - 1 + ids.length) % ids.length]);
      }
    },
    [activeTab, onChangeTab, ids]
  );

  return (
    <div
      className={cn(
        "gap-1 shrink-0 p-1 rounded-xl border border-[rgba(var(--foreground),0.06)] overflow-visible mb-3 bg-[rgba(var(--foreground),0.03)]",
        layoutMode === "small"
          ? "flex overflow-x-auto snap-x no-scrollbar scrollbar-none w-full scroll-smooth p-0.5 mb-2 bg-transparent border-transparent"
          : "flex items-center justify-around"
      )}
      role="tablist"
      aria-label={ariaLabel ?? "Settings topology"}
    >
      {nodes.map(({ id, label, shortLabel, caption, Icon, disabled, isDirty }) => {
        const isActive = activeTab === id;

        return (
          <button
            key={id}
            type="button"
            disabled={disabled}
            role="tab"
            aria-selected={isActive}
            tabIndex={isActive ? 0 : -1}
            data-arrow-nav
            onKeyDown={handleKeyDown}
            onClick={() => !disabled && onChangeTab(id)}
            className={cn(
              // Stage tiles: glyph, name, and what the stage does. Tall enough
              // to carry copy, so the strip reads as the card's spine.
              "rounded-lg flex flex-col items-center justify-center border text-center transition-colors duration-200 relative group overflow-hidden cursor-pointer flex-1 min-w-0",
              isActive
                ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.4)]"
                : "bg-transparent border-transparent hover:bg-[rgba(var(--foreground),0.04)]",
              disabled && "opacity-40 cursor-not-allowed",
              layoutMode === "small"
                ? "min-h-0 gap-0.5 px-2 py-2"
                : "h-[94px] min-h-[94px] gap-1 px-3 py-2",
              // Phones get a compact strip: the same three stages, one line
              // each, scrolled sideways. The caption needs a card, not a
              // 360px viewport.
              layoutMode === "small" &&
                "min-w-[104px] snap-center"
            )}
          >
            {isDirty && (
              <Tooltip label={DIRTY_STATE_COPY.category} side="top">
                <span
                  className="absolute top-2 right-2 w-1.5 h-1.5 rounded-full bg-[rgb(var(--warn-soft))] shadow-[0_0_6px_rgba(var(--warn-soft),0.8)] shrink-0"
                />
              </Tooltip>
            )}
            <Icon
              size={layoutMode === "small" ? 16 : 20}
              className={cn(
                "transition-colors shrink-0",
                isActive
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]"
              )}
            />
            <div className="flex max-w-full flex-col items-center gap-0">
              <span
                className={cn(
                  "font-semibold tracking-tight truncate max-w-full leading-tight select-none",
                  layoutMode === "small" ? "text-[11px]" : "text-[12px]",
                  isActive
                    ? "text-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--foreground))]"
                )}
              >
                {layoutMode === "small" && shortLabel ? shortLabel : label}
              </span>
              {caption && layoutMode !== "small" && (
                <span
                  className={cn(
                    "line-clamp-2 max-w-full text-[10px] leading-snug select-none",
                    isActive
                      ? "text-[rgb(var(--accent))]/75"
                      : "text-[rgb(var(--foreground-muted))]/60"
                  )}
                >
                  {caption}
                </span>
              )}
            </div>
          </button>
        );
      })}
    </div>
  );
}

export const PipelineTopologyMap = memo(PipelineTopologyMapInner) as <T extends string = string>(
  props: PipelineTopologyMapProps<T>
) => ReactElement | null;
