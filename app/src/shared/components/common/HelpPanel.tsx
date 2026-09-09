import { memo, useEffect, useMemo, useRef, useState } from "react";
import { useLocation } from "react-router-dom";
import { ChevronDown, Compass, Cpu, Keyboard, Lightbulb, Pin, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import {
  HELP_ARTICLES,
  HELP_DRAWER_COPY,
  HELP_TOC_GROUPS,
  type HelpArticle as HelpArticleData,
  type HelpTier,
} from "@/data/helpCopy";
import { deriveTier } from "@/services/helpService";

interface HelpPanelProps {
  /** Explicit article scope. Defaults by route: settings → settings:overview, else global. */
  deepLink?: string | null;
  onClose: () => void;
}

type Tier = ReturnType<typeof deriveTier>;

const filterArticles = (tier: Tier): HelpArticleData[] => {
  return HELP_ARTICLES.filter((a) => {
    if (!a.visibleOnTiers) return true;
    return a.visibleOnTiers.includes(tier);
  });
};

/**
 * Scope rule: an exact deepLink shows that article only; a settings-domain
 * link without its own article falls back to the settings group; no link
 * shows everything (global `?` entry point).
 */
export function scopeArticles(
  tiered: readonly HelpArticleData[],
  deepLink: string | null
): { scoped: HelpArticleData[]; rest: HelpArticleData[] } {
  if (!deepLink) return { scoped: [...tiered], rest: [] };
  const exact = tiered.find((a) => a.id === deepLink);
  if (exact) {
    return { scoped: [exact], rest: tiered.filter((a) => a.id !== deepLink) };
  }
  if (deepLink.startsWith("settings:")) {
    const group = tiered.filter((a) => a.group === "settings");
    if (group.length > 0) {
      return { scoped: group, rest: tiered.filter((a) => a.group !== "settings") };
    }
  }
  return { scoped: [...tiered], rest: [] };
}

const tierPalette: Record<HelpTier, { ring: string; text: string; bg: string; label: string }> = {
  "1A": {
    ring: "border-[rgba(var(--muted),0.4)]",
    text: "text-[rgb(var(--muted))]",
    bg: "bg-[rgba(var(--muted),0.08)]",
    label: "1A · CPU only",
  },
  "1B": {
    ring: "border-[rgba(var(--accent),0.35)]",
    text: "text-[rgb(var(--accent))]",
    bg: "bg-[rgba(var(--accent),0.1)]",
    label: "1B · Local + GPU",
  },
  "2A": {
    ring: "border-[rgba(var(--info),0.35)]",
    text: "text-[rgb(var(--info))]",
    bg: "bg-[rgba(var(--info),0.1)]",
    label: "2A · Remote LLM",
  },
  "2B": {
    ring: "border-[rgba(var(--success),0.35)]",
    text: "text-[rgb(var(--success))]",
    bg: "bg-[rgba(var(--success),0.1)]",
    label: "2B · Cloud LLM",
  },
  "3": {
    ring: "border-[rgba(var(--violet),0.35)]",
    text: "text-[rgb(var(--violet))]",
    bg: "bg-[rgba(var(--violet),0.1)]",
    label: "3 · Realtime",
  },
};

const TierBadge = memo(({ tier }: { tier: HelpTier }) => {
  const palette = tierPalette[tier];
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 px-2.5 py-1 rounded-xl border text-[11px] font-mono font-bold uppercase tracking-wider",
        palette.ring,
        palette.text,
        palette.bg
      )}
    >
      <Cpu size={12} className="shrink-0" />
      <span>
        {HELP_DRAWER_COPY.tierBadgePrefix}: {palette.label}
      </span>
    </span>
  );
});
TierBadge.displayName = "TierBadge";

const PinnedCrumb = memo(({ article, onClear }: { article: HelpArticleData; onClear: () => void }) => {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 px-2.5 py-1 rounded-xl border text-[11px] font-mono font-bold uppercase tracking-wider",
        "border-[rgba(var(--foreground),0.15)] text-[rgb(var(--foreground))]",
        "bg-[rgba(var(--foreground),0.04)]"
      )}
    >
      <Pin size={12} className="shrink-0" />
      <span>
        {HELP_DRAWER_COPY.pinnedCrumbPrefix}: {article.pinnedFrom ?? article.title}
      </span>
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
PinnedCrumb.displayName = "PinnedCrumb";

const EmptyState = memo(({ onClose }: { onClose: () => void }) => {
  return (
    <div className="flex-1 flex flex-col items-center justify-center text-center px-6 py-12 gap-3">
      <div className="p-3 rounded-2xl bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))]">
        <Compass size={28} />
      </div>
      <h3 className="font-display text-[16px] font-black uppercase tracking-[0.16em] text-[rgb(var(--foreground))]">
        {HELP_DRAWER_COPY.emptyStateTitle}
      </h3>
      <p className="text-[13px] leading-[1.6] text-[rgb(var(--foreground-muted))]">
        {HELP_DRAWER_COPY.emptyStateBody}
      </p>
      <button
        onClick={onClose}
        className="mt-2 px-3.5 py-1.5 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[12px] font-bold uppercase tracking-wider hover:brightness-110 transition-all cursor-pointer"
      >
        {HELP_DRAWER_COPY.emptyClose}
      </button>
    </div>
  );
});
EmptyState.displayName = "EmptyState";

const visibleTips = (article: HelpArticleData, tier: HelpTier) => {
  if (!article.tips) return [];
  return article.tips.filter((t) => t.tier === tier);
};

const ArticleView = memo(
  ({
    refCb,
    article,
    tier,
    isActive,
  }: {
    refCb: (el: HTMLDivElement | null) => void;
    article: HelpArticleData;
    tier: HelpTier;
    isActive: boolean;
  }) => {
    const tips = visibleTips(article, tier);
    return (
      <section
        ref={refCb}
        data-article-id={article.id}
        className={cn("scroll-mt-4 transition-opacity duration-300", isActive ? "opacity-100" : "opacity-90")}
      >
        <header className="mb-3 pb-2 border-b border-[rgba(var(--accent),0.10)]">
          <h2 className="font-display text-[17px] font-black tracking-wide text-[rgb(var(--foreground))]">
            {article.title}
          </h2>
          {article.pinnedFrom && (
            <p className="text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--accent))] mt-0.5">
              {article.pinnedFrom}
            </p>
          )}
        </header>
        <div className="space-y-5">
          {article.sections.map((section, idx) => (
            <div key={`${article.id}-section-${idx}`} className="space-y-2">
              <h3 className="font-display text-[13px] font-bold uppercase tracking-[0.16em] text-[rgb(var(--accent))]">
                {section.heading}
              </h3>
              {section.paragraphs?.map((p, i) => (
                <p key={`${article.id}-p-${idx}-${i}`} className="text-[13.5px] leading-[1.65] text-[rgb(var(--foreground))]/90">
                  {p}
                </p>
              ))}
              {section.bullets && (
                <ul className="space-y-1.5 pl-1">
                  {section.bullets.map((b, i) => (
                    <li
                      key={`${article.id}-b-${idx}-${i}`}
                      className="flex items-start gap-2 text-[13.5px] leading-[1.55] text-[rgb(var(--foreground))]/90"
                    >
                      <span className="mt-2 w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shrink-0" />
                      <span>{b}</span>
                    </li>
                  ))}
                </ul>
              )}
              {section.controls && (
                <ul className="grid gap-1.5">
                  {section.controls.map((c, i) => {
                    const ControlIcon = c.icon;
                    return (
                      <li
                        key={`${article.id}-c-${idx}-${i}`}
                        className="flex items-start gap-2.5 rounded-xl border border-[rgba(var(--border),0.1)] bg-[rgba(var(--card),0.5)] p-2.5"
                      >
                        <span className="w-8 h-8 rounded-lg border border-[rgba(var(--accent),0.25)] bg-[rgba(var(--accent),0.08)] text-[rgb(var(--accent))] flex items-center justify-center shrink-0">
                          <ControlIcon size={15} strokeWidth={1.75} />
                        </span>
                        <span className="min-w-0">
                          <span className="block text-[12.5px] font-bold text-[rgb(var(--foreground))]">{c.name}</span>
                          <span className="block text-[12px] leading-[1.5] text-[rgb(var(--foreground-muted))]">
                            {c.body}
                          </span>
                        </span>
                      </li>
                    );
                  })}
                </ul>
              )}
              {section.shortcuts && (
                <div className="rounded-xl glass-whisper p-3 space-y-1.5">
                  {section.shortcuts.map((s, i) => (
                    <div
                      key={`${article.id}-k-${idx}-${i}`}
                      className="flex items-center justify-between gap-3 text-[12.5px]"
                    >
                      <span className="text-[rgb(var(--foreground-muted))]">{s.label}</span>
                      <kbd className="font-mono text-[11px] px-2 py-0.5 rounded-md bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.25)]">
                        {s.keys}
                      </kbd>
                    </div>
                  ))}
                </div>
              )}
              {section.tip && (
                <div className="rounded-xl border border-[rgba(var(--accent),0.2)] bg-[rgba(var(--accent),0.06)] p-3 flex gap-2.5">
                  <Lightbulb size={16} className="text-[rgb(var(--accent))] mt-0.5 shrink-0" />
                  <div className="space-y-0.5">
                    <p className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--accent))]">
                      {section.tip.title}
                    </p>
                    <p className="text-[12.5px] leading-[1.55] text-[rgb(var(--foreground))]/85">{section.tip.body}</p>
                  </div>
                </div>
              )}
            </div>
          ))}
          {tips.length > 0 && (
            <div className="space-y-2 pt-2 border-t border-dashed border-[rgba(var(--accent),0.15)]">
              {tips.map((t, i) => (
                <div
                  key={`${article.id}-tier-tip-${i}`}
                  className="rounded-xl border border-[rgba(var(--violet),0.3)] bg-[rgba(var(--violet),0.08)] p-3 flex gap-2.5"
                >
                  <Keyboard size={16} className="text-[rgb(var(--violet))] mt-0.5 shrink-0" />
                  <div className="space-y-0.5">
                    <p className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--violet))]">{t.title}</p>
                    <p className="text-[12.5px] leading-[1.55] text-[rgb(var(--foreground))]/85">{t.body}</p>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </section>
    );
  }
);
ArticleView.displayName = "ArticleView";

/**
 * Help content rendered inside EdgePanel. Rail-first single column: meta row,
 * quick-jump chip strip, one scroll container. All articles, tier filtering,
 * and copy stay in helpCopy.ts / helpService.
 */
export const HelpPanel = memo(({ deepLink, onClose }: HelpPanelProps) => {
  const { pathname } = useLocation();

  const effectiveDeepLink = useMemo<string | null>(() => {
    if (deepLink !== undefined) return deepLink;
    return pathname.startsWith("/settings") ? "settings:overview" : null;
  }, [deepLink, pathname]);

  const [tier, setTier] = useState<Tier>("1A");
  const [activeArticleId, setActiveArticleId] = useState<string | null>(null);
  const [pinned, setPinned] = useState<string | null>(effectiveDeepLink);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [guidesOpen, setGuidesOpen] = useState(false);
  const articleRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const scrollerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setTier(deriveTier());
  }, []);

  useEffect(() => {
    setPinned(effectiveDeepLink);
    setExpandedId(null);
    setGuidesOpen(false);
  }, [effectiveDeepLink]);

  const tiered = useMemo(() => filterArticles(tier), [tier]);
  const { scoped, rest } = useMemo(() => scopeArticles(tiered, effectiveDeepLink), [tiered, effectiveDeepLink]);

  const articles = useMemo(() => {
    if (!expandedId) return scoped;
    const extra = tiered.find((a) => a.id === expandedId);
    if (!extra || scoped.some((a) => a.id === expandedId)) return scoped;
    return [...scoped, extra];
  }, [scoped, tiered, expandedId]);

  const initialArticleId = useMemo<string | null>(() => {
    if (expandedId && articles.some((a) => a.id === expandedId)) return expandedId;
    if (effectiveDeepLink && articles.some((a) => a.id === effectiveDeepLink)) return effectiveDeepLink;
    return articles[0]?.id ?? null;
  }, [effectiveDeepLink, articles, expandedId]);

  useEffect(() => {
    if (!initialArticleId) return;
    setActiveArticleId(initialArticleId);
    const id = window.setTimeout(() => {
      const el = articleRefs.current.get(initialArticleId);
      const scroller = scrollerRef.current;
      if (el && scroller) {
        const top = el.offsetTop - 16;
        scroller.scrollTo({ top, behavior: "smooth" });
      }
    }, 240);
    return () => window.clearTimeout(id);
  }, [initialArticleId]);

  useEffect(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries
          .filter((e) => e.isIntersecting)
          .sort((a, b) => b.intersectionRatio - a.intersectionRatio);
        if (visible[0]) {
          const id = (visible[0].target as HTMLElement).dataset.articleId;
          if (id) setActiveArticleId(id);
        }
      },
      { root: scroller, rootMargin: "0px 0px -60% 0px", threshold: [0.1, 0.5, 0.9] }
    );
    articleRefs.current.forEach((el) => observer.observe(el));
    return () => observer.disconnect();
  }, [articles]);

  const registerRef = (id: string) => (el: HTMLDivElement | null) => {
    if (el) articleRefs.current.set(id, el);
    else articleRefs.current.delete(id);
  };

  const handleSelect = (id: string) => {
    const el = articleRefs.current.get(id);
    const scroller = scrollerRef.current;
    if (el && scroller) {
      const top = el.offsetTop - 16;
      scroller.scrollTo({ top, behavior: "smooth" });
      setActiveArticleId(id);
    }
  };

  const handleSelectFromRest = (id: string) => {
    setExpandedId(id);
    setGuidesOpen(false);
  };

  useEffect(() => {
    if (!expandedId) return;
    const t = window.setTimeout(() => {
      const el = articleRefs.current.get(expandedId);
      const scroller = scrollerRef.current;
      if (el && scroller) {
        scroller.scrollTo({ top: el.offsetTop - 16, behavior: "smooth" });
        setActiveArticleId(expandedId);
      }
    }, 60);
    return () => window.clearTimeout(t);
  }, [expandedId, articles]);

  const handleUnpin = () => setPinned(null);

  const pinnedArticle = useMemo(
    () => (pinned ? articles.find((a) => a.id === pinned) ?? null : null),
    [pinned, articles]
  );

  const restGroups = useMemo(() => {
    const byGroup: Record<string, HelpArticleData[]> = {};
    for (const a of rest) {
      if (!byGroup[a.group]) byGroup[a.group] = [];
      byGroup[a.group].push(a);
    }
    return HELP_TOC_GROUPS.map((g) => ({
      label: g.label,
      id: g.id,
      items: byGroup[g.id] ?? [],
    })).filter((g) => g.items.length > 0);
  }, [rest]);

  if (articles.length === 0) {
    return <EmptyState onClose={onClose} />;
  }

  return (
    <div className="flex flex-col h-full min-h-0">
      <div className="flex items-center gap-2 flex-wrap px-3 pt-1 pb-2 shrink-0 border-b border-[rgba(var(--accent),0.08)]">
        <TierBadge tier={tier} />
        {pinnedArticle && <PinnedCrumb article={pinnedArticle} onClear={handleUnpin} />}
      </div>

      <nav aria-label={HELP_DRAWER_COPY.tocHeading} className="shrink-0 px-3 py-2 border-b border-[rgba(var(--border),0.08)]">
        <div className="flex gap-1.5 overflow-x-auto custom-scrollbar pb-0.5">
          {articles.map((a) => (
            <button
              key={a.id}
              onClick={() => handleSelect(a.id)}
              aria-current={activeArticleId === a.id ? "true" : undefined}
              className={cn(
                "shrink-0 px-2.5 py-1 rounded-full text-[11.5px] font-semibold border transition-colors cursor-pointer whitespace-nowrap",
                activeArticleId === a.id
                  ? "bg-[rgba(var(--accent),0.14)] text-[rgb(var(--accent))] border-[rgba(var(--accent),0.3)]"
                  : "text-[rgb(var(--foreground-muted))] border-[rgba(var(--border),0.12)] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.25)]"
              )}
            >
              {a.title}
            </button>
          ))}
        </div>
      </nav>

      <div ref={scrollerRef} className="flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar px-3.5 pt-3 pb-16">
        <div className="space-y-8">
          {articles.map((article) => (
            <ArticleView
              key={article.id}
              refCb={registerRef(article.id)}
              article={article}
              tier={tier}
              isActive={activeArticleId === article.id}
            />
          ))}
          {rest.length > 0 && (
            <div className="rounded-xl border border-[rgba(var(--border),0.12)] overflow-hidden">
              <button
                type="button"
                onClick={() => setGuidesOpen((v) => !v)}
                aria-expanded={guidesOpen}
                className="w-full flex items-center justify-between gap-2 px-3 py-2.5 text-[12px] font-bold uppercase tracking-[0.16em] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
              >
                <span>{HELP_DRAWER_COPY.allGuidesHeading}</span>
                <ChevronDown size={15} className={cn("transition-transform", guidesOpen && "rotate-180")} />
              </button>
              {guidesOpen && (
                <div className="border-t border-[rgba(var(--border),0.1)] max-h-64 overflow-y-auto custom-scrollbar p-2">
                  {restGroups.map((group) => (
                    <div key={group.id} className="py-1">
                      <p className="text-[10.5px] font-mono font-bold uppercase tracking-[0.18em] text-[rgb(var(--accent))]/80 px-2 pb-1">
                        {group.label}
                      </p>
                      {group.items.map((a) => (
                        <button
                          key={a.id}
                          onClick={() => handleSelectFromRest(a.id)}
                          className="w-full text-left px-2 py-1.5 rounded-lg text-[12.5px] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)] transition-colors cursor-pointer"
                        >
                          <span className="truncate block">{a.title}</span>
                        </button>
                      ))}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
          {/* Bottom spacing cushion so the final section is never clipped */}
          <div className="h-8 shrink-0" aria-hidden="true" />
        </div>
      </div>
    </div>
  );
});
HelpPanel.displayName = "HelpPanel";
