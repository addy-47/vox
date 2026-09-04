import { memo, useEffect, useMemo, useRef, useState } from "react";
import { HelpToc } from "./HelpToc";
import { HelpArticle } from "./HelpArticle";
import { HelpTierBadge } from "./HelpTierBadge";
import { HelpPinnedCrumb } from "./HelpPinnedCrumb";
import { HelpEmptyState } from "./HelpEmptyState";
import { HELP_ARTICLES, type HelpArticle as HelpArticleT } from "@/data/helpCopy";
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

const HelpContentInner = memo(({ deepLink, onClose }: HelpContentProps) => {
  const [tier, setTier] = useState<ReturnType<typeof deriveTier>>("1A");
  const [activeArticleId, setActiveArticleId] = useState<string | null>(null);
  const [pinned, setPinned] = useState<string | null>(deepLink);
  const articleRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const scrollerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setTier(deriveTier());
  }, []);

  useEffect(() => {
    setPinned(deepLink);
  }, [deepLink]);

  const articles = useMemo(() => filterArticles(tier), [tier]);

  const initialArticleId = useMemo<string | null>(() => {
    if (deepLink && articles.some((a) => a.id === deepLink)) return deepLink;
    return articles[0]?.id ?? null;
  }, [deepLink, articles]);

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
