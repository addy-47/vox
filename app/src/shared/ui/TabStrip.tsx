import React, { useRef } from "react";
import { cn } from "@/shared/lib/utils";

export interface TabItem {
  id: string;
  label: string;
  icon?: React.ComponentType<{ size?: number; className?: string }>;
  count?: number;
  disabled?: boolean;
  indicator?: boolean;
}

export interface TabStripProps {
  tabs: TabItem[];
  activeTab: string;
  onChange: (id: string) => void;
  variant?: "underline" | "pill";
  size?: "sm" | "md";
  ariaLabel: string;
  className?: string;
}

export const TabStrip = ({
  tabs,
  activeTab,
  onChange,
  variant = "underline",
  size = "md",
  ariaLabel,
  className,
}: TabStripProps) => {
  const ref = useRef<HTMLDivElement>(null);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    e.preventDefault();
    const container = ref.current;
    if (!container) return;
    const buttons = Array.from(container.querySelectorAll<HTMLButtonElement>('button[role="tab"]'));
    const enabled = buttons.filter((b) => !b.disabled);
    if (enabled.length === 0) return;
    const current = enabled.findIndex((b) => b === document.activeElement);
    const idx = current < 0 ? 0 : current;
    const next =
      e.key === "ArrowRight"
        ? (idx + 1) % enabled.length
        : (idx - 1 + enabled.length) % enabled.length;
    enabled[next]?.focus();
    const nextId = enabled[next]?.getAttribute("data-tab-id");
    if (nextId) onChange(nextId);
  };

  const sizeClasses =
    size === "sm"
      ? "text-[11px] px-2 py-1"
      : "text-[12px] px-3 py-1.5";

  return (
    <div
      ref={ref}
      role="tablist"
      aria-label={ariaLabel}
      className={cn(
        "flex items-center",
        variant === "underline" ? "gap-0 border-b border-[rgba(var(--accent),0.08)]" : "gap-1",
        className
      )}
      onKeyDown={handleKeyDown}
    >
      {tabs.map((tab) => {
        const Icon = tab.icon;
        const isActive = activeTab === tab.id;
        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            data-tab-id={tab.id}
            aria-selected={isActive}
            tabIndex={isActive ? 0 : -1}
            disabled={tab.disabled}
            onClick={() => {
              if (!tab.disabled) onChange(tab.id);
            }}
            className={cn(
              "flex items-center gap-1.5 font-mono font-bold uppercase tracking-[0.06em] transition-colors duration-150 cursor-pointer shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
              sizeClasses,
              variant === "underline"
                ? cn(
                    "border-b-2",
                    isActive
                      ? "border-[rgb(var(--accent))] text-[rgb(var(--accent))]"
                      : "border-transparent text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))]"
                  )
                : cn(
                    "rounded-xl border",
                    isActive
                      ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] border-[rgb(var(--accent))]/30 shadow-sm"
                      : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)] border-transparent"
                  ),
              tab.disabled && "opacity-40 cursor-not-allowed"
            )}
          >
            {Icon && <Icon size={size === "sm" ? 12 : 14} className="shrink-0" />}
            <span className="truncate">{tab.label}</span>
            {typeof tab.count === "number" && tab.count > 0 && (
              <span className="font-mono tabular-nums opacity-70">{tab.count}</span>
            )}
            {tab.indicator && (
              <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--success))] shadow-[0_0_6px_rgba(var(--success),0.8)] animate-pulse shrink-0" />
            )}
          </button>
        );
      })}
    </div>
  );
};

TabStrip.displayName = "TabStrip";