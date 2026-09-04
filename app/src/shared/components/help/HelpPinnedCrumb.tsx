import { memo } from "react";
import { X, Pin } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { HELP_DRAWER_COPY, type HelpArticle as HelpArticleData } from "@/data/helpCopy";

interface HelpPinnedCrumbProps {
  article: HelpArticleData;
  onClear: () => void;
}

const HelpPinnedCrumbInner = memo(({ article, onClear }: HelpPinnedCrumbProps) => {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 px-2.5 py-1 rounded-xl border text-[11px] font-mono font-bold uppercase tracking-wider",
        "border-[rgba(var(--foreground),0.15)] text-[rgb(var(--foreground))]",
        "bg-[rgba(var(--foreground),0.04)]"
      )}
    >
      <Pin size={12} className="shrink-0" />
      <span>{HELP_DRAWER_COPY.pinnedCrumbPrefix}: {article.pinnedFrom ?? article.title}</span>
      <button
        onClick={onClear}
        className="ml-0.5 -mr-1 inline-flex items-center justify-center w-4 h-4 rounded-full hover:bg-[rgba(var(--foreground),0.08)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
        aria-label={HELP_DRAWER_COPY.pinnedCrumbClear}
      >
        <X size={10} />
      </button>
    </span>
  );
});
HelpPinnedCrumbInner.displayName = "HelpPinnedCrumb";

export const HelpPinnedCrumb = HelpPinnedCrumbInner;
