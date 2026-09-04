import { memo, useMemo } from "react";
import { cn } from "@/shared/lib/utils";
import { HELP_TOC_GROUPS, type HelpArticle as HelpArticleT, HELP_DRAWER_COPY } from "@/data/helpCopy";

interface HelpTocProps {
  articles: HelpArticleT[];
  activeId: string | null;
  onSelect: (id: string) => void;
}

const tierClass = (isActive: boolean) =>
  cn(
    "w-full text-left px-3 py-1.5 rounded-lg text-[12.5px] transition-colors cursor-pointer",
    "flex items-center gap-2 min-w-0",
    isActive
      ? "bg-[rgba(var(--accent),0.14)] text-[rgb(var(--accent))] font-semibold"
      : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)]"
  );

const HelpTocInner = memo(({ articles, activeId, onSelect }: HelpTocProps) => {
  const grouped = useMemo(() => {
    const byGroup: Record<string, HelpArticleT[]> = {};
    for (const a of articles) {
      if (!byGroup[a.group]) byGroup[a.group] = [];
      byGroup[a.group].push(a);
    }
    return HELP_TOC_GROUPS.map((g) => ({
      label: g.label,
      id: g.id,
      items: byGroup[g.id] ?? [],
    })).filter((g) => g.items.length > 0);
  }, [articles]);

  return (
    <nav
      aria-label={HELP_DRAWER_COPY.tocHeading}
      className={cn(
        "shrink-0 border-t lg:border-t-0 lg:border-l border-[rgba(var(--accent),0.08)]",
        "lg:w-72 lg:max-h-full",
        "bg-[rgba(var(--foreground),0.015)]"
      )}
    >
      <div className="px-5 py-3 lg:py-5 lg:px-4 lg:h-full lg:overflow-y-auto custom-scrollbar">
        <p className="hidden lg:block text-[11px] font-mono font-bold uppercase tracking-[0.18em] text-[rgb(var(--foreground-muted))] mb-3 px-1">
          {HELP_DRAWER_COPY.tocHeading}
        </p>
        <div className="flex lg:flex-col gap-3 lg:gap-5 overflow-x-auto lg:overflow-visible">
          {grouped.map((group) => (
            <div key={group.id} className="space-y-1.5 shrink-0 lg:shrink min-w-0">
              <p className="text-[10.5px] font-mono font-bold uppercase tracking-[0.18em] text-[rgb(var(--accent))]/80 px-1">
                {group.label}
              </p>
              <div className="space-y-0.5">
                {group.items.map((a) => (
                  <button
                    key={a.id}
                    onClick={() => onSelect(a.id)}
                    className={tierClass(activeId === a.id)}
                    aria-current={activeId === a.id ? "true" : undefined}
                  >
                    <span className="truncate">{a.title}</span>
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>
    </nav>
  );
});
HelpTocInner.displayName = "HelpToc";

export const HelpToc = HelpTocInner;
