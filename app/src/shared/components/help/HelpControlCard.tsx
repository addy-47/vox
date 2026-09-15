import { memo } from "react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export interface HelpControlItem {
  icon: LucideIcon;
  name: string;
  badge?: string;
  action: string;
  outcome: string;
  shortcut?: string;
  tip?: string;
}

export interface HelpControlCardProps {
  item: HelpControlItem;
  variant?: "card" | "minimal";
  isLast?: boolean;
}

export const HelpControlCard = memo(({ item, variant = "card", isLast = false }: HelpControlCardProps) => {
  const Icon = item.icon;

  if (variant === "minimal") {
    return (
      <div className="flex gap-3 relative group">
        {/* Left Column: Icon Node + Connecting Vertical Spine */}
        <div className="flex flex-col items-center shrink-0 w-5">
          <div className="w-5 h-5 rounded-md bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.25)] group-hover:border-[rgba(var(--accent),0.5)] group-hover:bg-[rgba(var(--accent),0.14)] transition-colors flex items-center justify-center shrink-0 text-[rgb(var(--accent))]">
            <Icon size={12} strokeWidth={2.2} />
          </div>

          {!isLast && (
            <div className="w-px flex-1 my-1.5 bg-[rgba(var(--border),0.25)]" />
          )}
        </div>

        {/* Right Column: Content */}
        <div className={cn("flex-1 min-w-0 flex flex-col gap-0.5 text-left", isLast ? "pb-1" : "pb-3.5")}>
          <div className="flex items-baseline justify-between gap-2">
            <span className="font-semibold text-[13px] text-[rgb(var(--foreground))] group-hover:text-[rgb(var(--accent))] transition-colors">
              {item.name}
            </span>
            <div className="flex items-center gap-1.5 shrink-0">
              {item.shortcut && (
                <kbd className="font-mono text-[10px] px-1.5 py-0.5 rounded bg-[rgba(var(--accent),0.1)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.2)] font-semibold">
                  {item.shortcut}
                </kbd>
              )}
              {item.badge && !item.shortcut && (
                <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60">
                  {item.badge}
                </span>
              )}
            </div>
          </div>

          <p className="text-[12.5px] leading-relaxed text-[rgb(var(--foreground-muted))]">
            {item.outcome}
          </p>

          {item.tip && (
            <p className="text-[11.5px] text-[rgb(var(--foreground-muted))]/60 italic pt-0.5">
              Tip: {item.tip}
            </p>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="rounded-xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--foreground),0.025)] p-3 transition-colors duration-200 hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--foreground),0.045)] flex flex-col gap-2">
      {/* Header: Icon + Friendly Title + Badge / Shortcut */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2.5 min-w-0">
          <span className="w-7 h-7 rounded-lg border border-[rgba(var(--accent),0.25)] bg-[rgba(var(--accent),0.1)] text-[rgb(var(--accent))] flex items-center justify-center shrink-0">
            <Icon size={14} strokeWidth={2.2} />
          </span>
          <span className="font-bold text-[13px] text-[rgb(var(--foreground))] truncate">
            {item.name}
          </span>
        </div>

        <div className="flex items-center gap-1.5 shrink-0">
          {item.shortcut && (
            <kbd className="font-mono text-[10.5px] px-2 py-0.5 rounded-md bg-[rgba(var(--accent),0.1)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.25)] font-semibold">
              {item.shortcut}
            </kbd>
          )}
          {item.badge && !item.shortcut && (
            <span className="px-2 py-0.5 rounded-md bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.1)] text-[10px] font-mono font-medium text-[rgb(var(--foreground-muted))] uppercase tracking-wider">
              {item.badge}
            </span>
          )}
        </div>
      </div>

      {/* Description: Human, clear 1-2 sentence explanation */}
      <p className="text-[12.5px] leading-relaxed text-[rgb(var(--foreground))]/85 pl-0.5">
        {item.outcome}
      </p>

      {/* Pro tip if present */}
      {item.tip && (
        <div className="pt-1.5 border-t border-[rgba(var(--border),0.08)] text-[11.5px] text-[rgb(var(--foreground-muted))] italic pl-0.5">
          💡 {item.tip}
        </div>
      )}
    </div>
  );
});

HelpControlCard.displayName = "HelpControlCard";
