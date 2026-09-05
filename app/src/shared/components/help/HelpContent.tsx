import { memo, useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown } from "lucide-react";
import { HelpToc } from "./HelpToc";
import { HelpArticle } from "./HelpArticle";
import { HelpTierBadge } from "./HelpTierBadge";
import { HelpPinnedCrumb } from "./HelpPinnedCrumb";
import { HelpEmptyState } from "./HelpEmptyState";
import {
  HELP_ARTICLES,
  HELP_DRAWER_COPY,
  type HelpArticle as HelpArticleT,
} from "@/data/helpCopy";
import { deriveTier } from "@/services/helpService";
import { cn } from "@/shared/lib/utils";

interface HelpContentProps {
  deepLink: string | null;
  onClose: () => void;
}

const filterArticles = (tier: ReturnType<typeof deriveTier>): HelpArticleT[] => {
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
  tiered: readonly HelpArticleT[],
  deepLink: string | null
): { scoped: HelpArticleT[]; rest: HelpArticleT[] } {
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

const HelpContentInner = memo(({ deepLink, onClose }: HelpContentProps) => {
  const [tier, setTier] = useState<ReturnType<typeof deriveTier>>("1A");
  const [activeArticleId, setActiveArticleId] = useState<string | null>(null);
  const [pinned, setPinned] = useState<string | null>(deepLink);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [guidesOpen, setGuidesOpen] = useState(false);
  const articleRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const scrollerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setTier(deriveTier());
  }, []);

  useEffect(() => {
    setPinned(deepLink);
    setExpandedId(null);
    setGuidesOpen(false);
  }, [deepLink]);

  const tiered = useMemo(() => filterArticles(tier), [tier]);
  const { scoped, rest } = useMemo(() => scopeArticles(tiered, deepLink), [tiered, deepLink]);

  const articles = useMemo(() => {
    if (!expandedId) return scoped;
    const extra = tiered.find((a) => a.id === expandedId);
    if (!extra || scoped.some((a) => a.id === expandedId)) return scoped;
    return [...scoped, extra];
  }, [scoped, tiered, expandedId]);

  const initialArticleId = useMemo<string | null>(() => {
    if (expandedId && articles.some((a) => a.id === expandedId)) return expandedId;
    if (deepLink && articles.some((a) => a.id === deepLink)) return deepLink;
    return articles[0]?.id ?? null;
  }, [deepLink, articles, expandedId]);

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

  if (articles.length === 0) {
    return <HelpEmptyState onClose={onClose} />;
  }

  return (
    <div className="flex flex-col h-full min-h-0">
      <div className="flex items-center justify-between gap-3 px-5 sm:px-7 pt-3 pb-2 shrink-0 border-b border-[rgba(var(--accent),0.08)]">
        <div className="flex items-center gap-2 flex-wrap min-w-0">
          <HelpTierBadge tier={tier} />
          {pinnedArticle && <HelpPinnedCrumb article={pinnedArticle} onClear={handleUnpin} />}
        </div>
      </div>
      <div className="flex-1 min-h-0 flex flex-col lg:flex-row">
        <div
          ref={scrollerRef}
          className={cn(
            "flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar",
            "px-5 sm:px-7 py-5"
          )}
        >
          <div className="max-w-3xl mx-auto space-y-10">
            {articles.map((article) => (
              <HelpArticle
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
                  className="w-full flex items-center justify-between gap-2 px-4 py-3 text-[12px] font-bold uppercase tracking-[0.16em] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                >
                  <span>{HELP_DRAWER_COPY.allGuidesHeading}</span>
                  <ChevronDown
                    size={15}
                    className={cn("transition-transform", guidesOpen && "rotate-180")}
                  />
                </button>
                {guidesOpen && (
                  <div className="border-t border-[rgba(var(--border),0.1)] max-h-64 overflow-y-auto custom-scrollbar">
                    <HelpToc
                      articles={rest}
                      activeId={expandedId}
                      onSelect={handleSelectFromRest}
                    />
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
        <HelpToc
          articles={articles}
          activeId={activeArticleId}
          onSelect={handleSelect}
        />
      </div>
    </div>
  );
});
HelpContentInner.displayName = "HelpContent";

export const HelpContent = HelpContentInner;
