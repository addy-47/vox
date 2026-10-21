import { memo, type ReactNode } from "react";
import { cn } from "@/shared/lib/utils";
import { ProviderLogo } from "./ProviderLogo";

export interface SelectableCardProps {
  id: string;
  name: string;
  /** Optional brand-name hint forwarded to ProviderLogo for engine-logo inference. */
  providerName?: string;
  tagline?: string;
  logo?: string | null;
  isSelected: boolean;
  isComingSoon?: boolean;
  /** Corner affordance rendered in the card's top-right slot (e.g. breadcrumb navigation). */
  cornerSlot?: ReactNode;
  onSelect: () => void;
  actionSlot?: ReactNode;
  /**
   * Replaces the brand mark. Catalogs whose entity has no brand (engines with a
   * glyph) pass their own tile instead of a logo.
   */
  leading?: ReactNode;
}

/**
 * One tile in a settings catalog modal. The shared geometry for every picker
 * card: a 48px mark, a centered name, an optional two-line tagline, and a
 * trailing action strip. The body selects; corner and action slots stop
 * propagation so their own controls never select the tile.
 */
export const SelectableCard = memo(({
  id,
  name,
  providerName,
  tagline,
  logo,
  isSelected,
  isComingSoon,
  cornerSlot,
  onSelect,
  actionSlot,
  leading,
}: SelectableCardProps) => {
  return (
    <div
      data-card-id={id}
      onClick={isComingSoon ? undefined : onSelect}
      className={cn(
        "relative flex flex-col justify-between p-4.5 rounded-xl border transition-all duration-200 select-none group transform-gpu h-[188px]",
        isComingSoon
          ? "opacity-45 cursor-not-allowed bg-[rgba(var(--card),0.2)] border-[rgba(var(--foreground),0.04)]"
          : isSelected
          ? "bg-[rgba(var(--card),0.7)] border-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.18)] ring-1 ring-[rgb(var(--accent))]/30 cursor-pointer"
          : "bg-[rgba(var(--card),0.4)] border-[rgba(var(--foreground),0.07)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--card),0.65)] cursor-pointer"
      )}
    >
      {cornerSlot && (
        <div className="absolute top-3 right-3 flex items-center gap-1.5 z-10" onClick={(e) => e.stopPropagation()}>
          {cornerSlot}
        </div>
      )}

      {leading ? (
        <div className="flex flex-col items-center text-center my-auto px-1 pt-1 min-h-0">
          {leading}
          <h4 className="font-bold text-[14.5px] text-[rgb(var(--foreground))] tracking-tight truncate w-full text-center mt-2.5 leading-tight group-hover:text-[rgb(var(--accent))] transition-colors">
            {name}
          </h4>
          {tagline && (
            <p className="text-[12px] text-[rgb(var(--foreground-muted))] mt-1 line-clamp-2 leading-relaxed text-center px-1 h-[36px]">
              {tagline}
            </p>
          )}
        </div>
      ) : (
        <div className="flex flex-col items-center text-center my-auto px-1 pt-1">
          <div className={cn(
            "w-12 h-12 rounded-2xl flex items-center justify-center shrink-0 border transition-all",
            isSelected
              ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.3)] shadow-[0_0_12px_rgba(var(--accent),0.15)]"
              : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--foreground),0.07)] group-hover:border-[rgba(var(--accent),0.25)]"
          )}>
            <ProviderLogo providerId={id} providerName={providerName ?? name} logo={logo} size={24} />
          </div>
          <h4 className="font-bold text-[14.5px] text-[rgb(var(--foreground))] tracking-tight truncate w-full text-center mt-2.5 leading-tight group-hover:text-[rgb(var(--accent))] transition-colors">
            {name}
          </h4>
          {tagline && (
            <p className="text-[12px] text-[rgb(var(--foreground-muted))] mt-1 line-clamp-2 leading-relaxed text-center px-1 h-[36px]">
              {tagline}
            </p>
          )}
        </div>
      )}

      {actionSlot && (
        <div className="mt-auto pt-2.5 border-t border-[rgba(var(--foreground),0.06)] h-[28px] flex items-center justify-between" onClick={(e) => e.stopPropagation()}>
          {actionSlot}
        </div>
      )}
    </div>
  );
});
SelectableCard.displayName = "SelectableCard";
