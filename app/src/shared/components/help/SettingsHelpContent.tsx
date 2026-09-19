import { useState, useMemo, memo, lazy, Suspense } from "react";
import { SETTINGS_PAGE_HELP, type SettingsCardId } from "@/data/helpCopy";
import { HelpControlCard } from "./HelpControlCard";
import { cn } from "@/shared/lib/utils";
import { ErrorBoundary } from "@/shared/components/common";

const HelpPipelineDiagram = lazy(() => import("./HelpPipelineDiagram").then((m) => ({ default: m.HelpPipelineDiagram })));
const HelpInteractionDiagram = lazy(() => import("./HelpInteractionDiagram").then((m) => ({ default: m.HelpInteractionDiagram })));
const HelpMemoryKnobsDiagram = lazy(() => import("./HelpMemoryKnobsDiagram").then((m) => ({ default: m.HelpMemoryKnobsDiagram })));

interface SettingsHelpContentProps {
  initialCardId?: SettingsCardId;
}

export const SettingsHelpContent = memo(({ initialCardId = "models" }: SettingsHelpContentProps) => {
  const [selectedCardId, setSelectedCardId] = useState<SettingsCardId>(initialCardId);

  const activeCard = useMemo(() => {
    return SETTINGS_PAGE_HELP.cards.find((c) => c.id === selectedCardId) ?? SETTINGS_PAGE_HELP.cards[0];
  }, [selectedCardId]);

  const CardIcon = activeCard.icon;

  return (
    <div className="flex flex-col gap-4 select-none font-sans">
      {/* ── Subtitle intro ── */}
      <p className="text-[13px] leading-relaxed text-[rgb(var(--foreground-muted))]">
        {SETTINGS_PAGE_HELP.subtitle}
      </p>

      {/* ── Settings Category Tabs (Responsive, full labels) ── */}
      <div className="flex flex-col gap-1.5 pt-0.5">
        <span className="text-[10.5px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70">
          Categories
        </span>
        <div className="flex flex-wrap gap-1.5">
          {SETTINGS_PAGE_HELP.cards.map((card) => {
            const Icon = card.icon;
            const isSelected = card.id === selectedCardId;

            return (
              <button
                key={card.id}
                type="button"
                onClick={() => setSelectedCardId(card.id)}
                className={cn(
                  "flex items-center gap-1.5 py-1.5 px-2.5 rounded-lg text-[12px] font-medium transition-colors cursor-pointer border",
                  isSelected
                    ? "bg-[rgba(var(--accent),0.10)] text-[rgb(var(--accent))] border-[rgba(var(--accent),0.3)] font-semibold shadow-xs"
                    : "border-[rgba(var(--border),0.12)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--border),0.25)]"
                )}
              >
                <Icon size={13} className={isSelected ? "text-[rgb(var(--accent))]" : "opacity-60"} />
                <span>{card.label}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* ── Accent-tinted Section Divider (separating Category selector from Active Section) ── */}
      <div className="h-px bg-gradient-to-r from-[rgba(var(--accent),0.45)] via-[rgba(var(--accent),0.2)] to-transparent my-0.5" />

      {/* ── Main Section Container (Subtle minimal border providing structure) ── */}
      <div className="rounded-xl border border-[rgba(var(--border),0.14)] bg-[rgba(var(--foreground),0.015)] p-3.5 flex flex-col gap-3.5">
        {/* ── Active Category Overview ── */}
        <div className="flex flex-col gap-1">
          <div className="flex items-baseline justify-between gap-2 flex-wrap">
            <h3 className="font-display text-[15px] font-bold text-[rgb(var(--foreground))] tracking-tight flex items-center gap-2">
              <CardIcon size={16} className="text-[rgb(var(--accent))]" />
              {activeCard.label}
            </h3>
            <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60">
              {activeCard.badge}
            </span>
          </div>
          <p className="text-[12.5px] leading-relaxed text-[rgb(var(--foreground-muted))] mt-0.5">
            {activeCard.overview}
          </p>
        </div>

        {/* ── Visual Diagrams for Key Categories (Lazy Loaded with internal grey divider) ── */}
        {(selectedCardId === "models" || selectedCardId === "interaction" || selectedCardId === "memory") && (
          <>
            <div className="h-px bg-[rgba(var(--border),0.10)]" />
            <ErrorBoundary name={`SettingsDiagram:${selectedCardId}`}>
              <Suspense fallback={null}>
                {selectedCardId === "models" && <HelpPipelineDiagram />}
                {selectedCardId === "interaction" && <HelpInteractionDiagram />}
                {selectedCardId === "memory" && <HelpMemoryKnobsDiagram />}
              </Suspense>
            </ErrorBoundary>
          </>
        )}

        {/* ── Internal Grey Divider before Controls ── */}
        <div className="h-px bg-[rgba(var(--border),0.10)]" />

        {/* ── Controls List (Connected by vertical spine) ── */}
        <div className="flex flex-col gap-1.5">
          <h4 className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] flex items-center gap-1.5">
            <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
            Controls & Options
          </h4>

          <div className="flex flex-col pt-4">
            {activeCard.controls.map((item, idx) => (
              <HelpControlCard
                key={item.name}
                item={item}
                variant="minimal"
                isLast={idx === activeCard.controls.length - 1}
              />
            ))}
          </div>
        </div>

        {/* ── Internal Grey Divider before Tips ── */}
        {activeCard.tips && activeCard.tips.length > 0 && (
          <>
            <div className="h-px bg-[rgba(var(--border),0.10)]" />
            <div className="border-l-2 border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.04)] pl-3.5 pr-3 py-2 rounded-r-xl flex flex-col gap-1 text-[12px] text-[rgb(var(--foreground-muted))] border-y border-r border-[rgba(var(--border),0.08)]">
              {activeCard.tips.map((tip) => (
                <p key={tip} className="leading-relaxed">
                  💡 {tip}
                </p>
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
});

SettingsHelpContent.displayName = "SettingsHelpContent";
