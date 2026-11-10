import React, { memo, useMemo } from "react";
import { Pin, Calendar, Code2, Trash2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { resolveSessionTitle } from "@/services/persistenceService";
import { formatCardDate } from "./recallDateUtils";
import { RECALL_CARD_WIDTH, RECALL_CARD_HEIGHT } from "./recallArcMath";
import { Tooltip } from "@/shared/ui/Tooltip";
import { isBoundaryCard, isSkeletonCard, type CarouselItemData } from "./recallTypes";
import { SESSION_COPY } from "@/data/sessionCopy";

export interface RecallSessionCardProps {
  item: CarouselItemData;
  isActive: boolean;
  projectName?: string;
  onSelect: (item: CarouselItemData) => void;
  onDelete?: (id: number) => void;
}

export const RecallSessionCard = memo(({
  item,
  isActive,
  projectName,
  onSelect,
  onDelete,
}: RecallSessionCardProps) => {
  const isBoundary = isBoundaryCard(item);
  const isSkeleton = isSkeletonCard(item);

  const title = useMemo(() => {
    if (isSkeleton) return "";
    if (isBoundary) return item.label;
    return resolveSessionTitle(item);
  }, [isBoundary, isSkeleton, item]);

  const dateText = useMemo(() => {
    if (isBoundary || isSkeleton) return "";
    return formatCardDate(item.created_at);
  }, [isBoundary, isSkeleton, item]);

  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!isSkeleton) {
      onSelect(item);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.target !== e.currentTarget || isSkeleton) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onSelect(item);
    }
  };

  if (isSkeleton) {
    return (
      <div
        style={{ width: RECALL_CARD_WIDTH, height: RECALL_CARD_HEIGHT }}
        className="rounded-2xl p-4 flex flex-col justify-between select-none pointer-events-none text-left glass-card bg-[rgb(var(--card))]/25 border border-dashed border-[rgba(var(--accent),0.2)] backdrop-blur-xs opacity-45"
      >
        <div className="flex items-center justify-between gap-2 shrink-0">
          <div className="flex items-center gap-1.5 min-w-0">
            <span className="w-1.5 h-1.5 rounded-full border border-dashed border-[rgb(var(--foreground-muted))]/40 shrink-0" />
            <div className="h-2.5 w-12 rounded-full bg-[rgba(var(--foreground-muted),0.12)] animate-pulse" />
          </div>
        </div>
        <div className="flex flex-col gap-2 my-2">
          <div className="h-3 w-3/4 rounded-full bg-[rgba(var(--foreground-muted),0.14)] animate-pulse" />
          <div className="h-3 w-1/2 rounded-full bg-[rgba(var(--foreground-muted),0.08)] animate-pulse" />
        </div>
        <div className="shrink-0 pt-1 flex items-center justify-between">
          <div className="h-2 w-10 rounded-full bg-[rgba(var(--foreground-muted),0.08)]" />
        </div>
      </div>
    );
  }

  if (isBoundary) {
    return (
      <div
        role="button"
        tabIndex={0}
        onClick={handleClick}
        onKeyDown={handleKeyDown}
        style={{ width: RECALL_CARD_WIDTH, height: RECALL_CARD_HEIGHT }}
        className={cn(
          "rounded-2xl p-4 flex flex-col justify-between select-none cursor-pointer text-left transition-colors",
          isActive ? "glass-card backdrop-blur-md" : "bg-[rgb(var(--card))]/80",
          "border",
          isActive
            ? "border-[rgba(var(--accent),0.6)] shadow-[0_0_25px_rgba(var(--accent),0.28)]"
            : "border-[rgba(var(--border),0.08)] hover:border-[rgba(var(--accent),0.3)] hover:shadow-lg"
        )}
      >
        <div className="w-8 h-8 rounded-xl bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] flex items-center justify-center text-[rgb(var(--accent))]">
          <Calendar size={16} />
        </div>
        <div className="flex flex-col gap-1">
          <h3 className="text-[15px] font-display font-semibold text-[rgb(var(--foreground))]">
            {item.label}
          </h3>
          <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
            {item.count} conversations
          </p>
        </div>
      </div>
    );
  }

  const session = item;

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      style={{ width: RECALL_CARD_WIDTH, height: RECALL_CARD_HEIGHT }}
      className={cn(
        "group relative rounded-2xl p-4 flex flex-col justify-between select-none cursor-pointer text-left transition-[border-color,box-shadow,background-color] duration-150",
        isActive ? "bg-[rgb(var(--card))] backdrop-blur-md" : "bg-[rgb(var(--card))]/85",
        "border shadow-md",
        isActive
          ? "border-[rgba(var(--accent),0.75)] shadow-[0_0_28px_rgba(var(--accent),0.35)] ring-1 ring-[rgba(var(--accent),0.4)]"
          : "border-[rgba(var(--border),0.08)] hover:border-[rgba(var(--border),0.2)]"
      )}
    >
      {/* Top Header: Dot indicator + Date, and Pin / Actions */}
      <div className="flex items-center justify-between gap-2 shrink-0">
        <div className="flex items-center gap-1.5 min-w-0">
          <span
            className={cn(
              "w-1.5 h-1.5 rounded-full shrink-0",
              isActive
                ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.9)]"
                : "bg-[rgb(var(--foreground-muted))]/40"
            )}
          />
          <span className="text-[11px] font-sans font-medium text-[rgb(var(--foreground-muted))] truncate">
            {dateText}
          </span>
        </div>

        <div className="flex items-center gap-1 shrink-0">
          {session.is_pinned && (
            <Pin size={12} className="text-[rgb(var(--accent))] fill-[rgba(var(--accent),0.4)] shrink-0" />
          )}
          {onDelete && (
            <div className="opacity-0 group-hover:opacity-100 transition-opacity">
              <Tooltip label={SESSION_COPY.deleteSessionAriaLabel} side="top">
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete(session.id);
                  }}
                  className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.1)] transition-colors cursor-pointer"
                  aria-label={SESSION_COPY.deleteSessionAriaLabel}
                >
                  <Trash2 size={12} />
                </button>
              </Tooltip>
            </div>
          )}
        </div>
      </div>

      {/* Middle: Title & Project Chip */}
      <div className="flex flex-col gap-2 min-h-0 my-1">
        <h3 className="text-[14px] sm:text-[14.5px] font-semibold text-[rgb(var(--foreground))] line-clamp-2 leading-snug break-words">
          {title}
        </h3>

        {(projectName || session.project_id) && (
          <div className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.08)] w-fit text-[11px] font-mono text-[rgb(var(--foreground-muted))] leading-none">
            <Code2 size={11} className="text-[rgb(var(--accent))] shrink-0" />
            <span className="truncate max-w-[120px]">
              {projectName || session.project_id}
            </span>
          </div>
        )}
      </div>

      {/* Bottom: Turn count */}
      <div className="shrink-0 pt-1 flex items-center justify-between text-[11px] font-mono text-[rgb(var(--foreground-muted))]/70">
        <span>
          {session.turn_count} {session.turn_count === 1 ? SESSION_COPY.turnSingular : SESSION_COPY.turnPlural}
        </span>
      </div>
    </div>
  );
});

RecallSessionCard.displayName = "RecallSessionCard";
