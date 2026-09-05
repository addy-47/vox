import { memo } from "react";
import { Keyboard, Lightbulb } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { type HelpArticle as HelpArticleData, type HelpTier } from "@/data/helpCopy";

interface HelpArticleProps {
  refCb: (el: HTMLDivElement | null) => void;
  article: HelpArticleData;
  tier: HelpTier;
  isActive: boolean;
}

const visibleTips = (article: HelpArticleData, tier: HelpTier) => {
  if (!article.tips) return [];
  return article.tips.filter((t) => t.tier === tier);
};

const HelpArticleInner = memo(
  ({ refCb, article, tier, isActive }: HelpArticleProps) => {
    const tips = visibleTips(article, tier);
    return (
      <section
        ref={refCb}
        data-article-id={article.id}
        className={cn(
          "scroll-mt-4 transition-opacity duration-300",
          isActive ? "opacity-100" : "opacity-90"
        )}
      >
        <header className="mb-3 pb-2 border-b border-[rgba(var(--accent),0.10)]">
          <h2 className="font-display text-[20px] sm:text-[22px] font-black tracking-wide text-[rgb(var(--foreground))]">
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
                <p
                  key={`${article.id}-p-${idx}-${i}`}
                  className="text-[13.5px] leading-[1.65] text-[rgb(var(--foreground))]/90"
                >
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
                <ul className="grid gap-1.5 sm:grid-cols-2">
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
                          <span className="block text-[12.5px] font-bold text-[rgb(var(--foreground))]">
                            {c.name}
                          </span>
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
                    <p className="text-[12.5px] leading-[1.55] text-[rgb(var(--foreground))]/85">
                      {section.tip.body}
                    </p>
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
                    <p className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--violet))]">
                      {t.title}
                    </p>
                    <p className="text-[12.5px] leading-[1.55] text-[rgb(var(--foreground))]/85">
                      {t.body}
                    </p>
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
HelpArticleInner.displayName = "HelpArticle";

export const HelpArticle = HelpArticleInner;
