import { memo } from "react";
import { ArrowRight, Plus } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export interface AddCardProps {
  title: string;
  /** Short line under the title. Ignored by the row variant. */
  description?: string;
  onClick: () => void;
  /**
   * `row` matches `CatalogRowCard` and sits in a compact list.
   * `tile` matches `SelectableCard` (188px) inside an expanded catalog.
   */
  variant?: "row" | "tile";
  /** Bottom-left slot caption for the tile variant. */
  footnote?: string;
  /** Bottom-right call to action for the tile variant. */
  actionLabel?: string;
  className?: string;
}

/**
 * The dashed "nothing here yet" affordance. It only opens the surface that
 * owns creation; it never embeds a form of its own, so a compact list cannot
 * turn into an editor by accident.
 */
export const AddCard = memo(({
  title,
  description,
  onClick,
  variant = "row",
  footnote,
  actionLabel = "+ Configure",
  className,
}: AddCardProps) => {
  if (variant === "row") {
    return (
      <button
        type="button"
        onClick={onClick}
        className={cn(
          "group flex w-full items-center gap-2.5 rounded-xl border border-dashed",
          "min-h-[64px] py-2.5 pl-2.5 pr-2.5 text-left transition-colors duration-200",
          "border-[rgba(var(--foreground),0.2)] bg-transparent",
          "hover:border-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.03)] cursor-pointer",
          className
        )}
      >
        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-dashed border-[rgba(var(--foreground),0.25)] text-[rgb(var(--foreground-muted))] transition-all group-hover:border-[rgb(var(--accent))] group-hover:text-[rgb(var(--accent))]">
          <Plus size={16} strokeWidth={2.25} />
        </span>
        <span className="min-w-0 flex-1 text-[13px] font-semibold leading-tight tracking-tight text-[rgb(var(--foreground-muted))] transition-colors group-hover:text-[rgb(var(--accent))]">
          {title}
        </span>
        <ArrowRight
          size={13}
          strokeWidth={2}
          className="shrink-0 text-[rgb(var(--foreground-muted))]/50 transition-all group-hover:translate-x-0.5 group-hover:text-[rgb(var(--accent))]"
        />
      </button>
    );
  }

  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "relative flex h-[188px] w-full flex-col justify-between rounded-xl border border-dashed p-4.5 text-left",
        "border-[rgba(var(--foreground),0.2)] bg-transparent transition-all duration-200 select-none",
        "hover:border-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.03)] group cursor-pointer transform-gpu",
        className
      )}
    >
      <span className="my-auto flex flex-col items-center px-1 pt-1 text-center">
        <span className="flex h-12 w-12 items-center justify-center rounded-2xl border border-dashed border-[rgba(var(--foreground),0.25)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))] transition-all group-hover:scale-105 group-hover:border-[rgb(var(--accent))] group-hover:text-[rgb(var(--accent))]">
          <Plus size={20} strokeWidth={2.25} />
        </span>
        <span className="mt-2.5 w-full truncate text-center text-[14.5px] font-bold leading-tight tracking-tight text-[rgb(var(--foreground))] transition-colors group-hover:text-[rgb(var(--accent))]">
          {title}
        </span>
        {description && (
          <span className="mt-1 line-clamp-2 h-[36px] px-1 text-center text-[12px] leading-relaxed text-[rgb(var(--foreground-muted))]">
            {description}
          </span>
        )}
      </span>
      <span className="mt-auto flex h-[28px] items-center justify-between border-t border-dashed border-[rgba(var(--foreground),0.12)] pt-2.5 font-mono text-[11px]">
        <span className="text-[rgb(var(--foreground-muted))]/60">{footnote}</span>
        <span className="flex items-center gap-1 font-medium text-[rgb(var(--accent))] transition-transform group-hover:translate-x-0.5">
          {actionLabel} <ArrowRight size={11} />
        </span>
      </span>
    </button>
  );
});

AddCard.displayName = "AddCard";
