import { memo } from "react";
import type { LucideIcon } from "lucide-react";

export interface HelpControlItem {
  icon: LucideIcon;
  name: string;
  badge?: string;
  action: string;
  outcome: string;
  shortcut?: string;
  tip?: string;
}

export const HelpControlCard = memo(({ item }: { item: HelpControlItem }) => {
  const Icon = item.icon;

  return (
    <div className="rounded-xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--foreground),0.025)] p-3 transition-all duration-200 hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--foreground),0.045)] flex flex-col gap-2">
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
