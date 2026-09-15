import { useState, useMemo, memo } from "react";
import { SETTINGS_PAGE_HELP, type SettingsCardId } from "@/data/helpCopy";
import { HelpControlCard } from "./HelpControlCard";
import { HelpPipelineDiagram } from "./HelpPipelineDiagram";
import { HelpInteractionDiagram } from "./HelpInteractionDiagram";
import { HelpMemoryKnobsDiagram } from "./HelpMemoryKnobsDiagram";
import { cn } from "@/shared/lib/utils";

interface SettingsHelpContentProps {
  initialCardId?: SettingsCardId;
}

export const SettingsHelpContent = memo(({ initialCardId = "models" }: SettingsHelpContentProps) => {
  const [selectedCardId, setSelectedCardId] = useState<SettingsCardId>(initialCardId);

  const activeCard = useMemo(() => {
    return SETTINGS_PAGE_HELP.cards.find((c) => c.id === selectedCardId) ?? SETTINGS_PAGE_HELP.cards[0];
  }, [selectedCardId]);

  return (
    <div className="flex flex-col gap-5 select-none font-sans">
      {/* ── Subtitle intro ── */}
      <p className="text-[13px] leading-relaxed text-[rgb(var(--foreground-muted))]">
        {SETTINGS_PAGE_HELP.subtitle}
      </p>

      {/* ── Settings Category Tabs ── */}
      <div className="flex flex-col gap-1.5">
        <span className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))]">
          Settings Categories
        </span>
        <div className="grid grid-cols-3 gap-1.5 p-1 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)]">
          {SETTINGS_PAGE_HELP.cards.map((card) => {
            const Icon = card.icon;
            const isSelected = card.id === selectedCardId;

            return (
              <button
                key={card.id}
                onClick={() => setSelectedCardId(card.id)}
                className={cn(
                  "flex items-center gap-2 py-2 px-2.5 rounded-lg text-[11.5px] font-medium transition-all cursor-pointer truncate",
                  isSelected
                    ? "bg-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground))] shadow-xs border border-[rgba(var(--accent),0.3)]"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)]"
                )}
              >
                <Icon size={14} className={cn("shrink-0", isSelected ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]")} />
                <span className="truncate">{card.label}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* ── Active Category Overview ── */}
      <div className="rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--card),0.75)] p-4 flex flex-col gap-2.5 backdrop-blur-md">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <span className="w-8 h-8 rounded-lg border border-[rgba(var(--accent),0.25)] bg-[rgba(var(--accent),0.1)] text-[rgb(var(--accent))] flex items-center justify-center shrink-0">
              <activeCard.icon size={16} />
            </span>
            <span className="font-display text-[14px] font-bold tracking-wide text-[rgb(var(--foreground))]">
              {activeCard.label}
            </span>
          </div>
          <span className="px-2 py-0.5 rounded-md bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.2)] text-[10.5px] font-mono text-[rgb(var(--accent))] uppercase tracking-wider shrink-0">
            {activeCard.badge}
          </span>
        </div>

        <p className="text-[12.5px] leading-relaxed text-[rgb(var(--foreground))]/85">
          {activeCard.overview}
        </p>
      </div>

      {/* ── Visual Diagrams for Key Categories ── */}
      {selectedCardId === "models" && <HelpPipelineDiagram />}
      {selectedCardId === "interaction" && <HelpInteractionDiagram />}
      {selectedCardId === "memory" && <HelpMemoryKnobsDiagram />}

      {/* ── Controls List ── */}
      <div className="flex flex-col gap-2.5">
        <h3 className="text-[11.5px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-2">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          What You Can Change
        </h3>

        <div className="flex flex-col gap-2">
          {activeCard.controls.map((item) => (
            <HelpControlCard key={item.name} item={item} />
          ))}
        </div>
      </div>

      {/* ── Helpful Tips ── */}
      {activeCard.tips && activeCard.tips.length > 0 && (
        <div className="p-3 rounded-xl border border-[rgba(var(--border),0.1)] bg-[rgba(var(--foreground),0.02)] flex flex-col gap-1.5 text-[12px] text-[rgb(var(--foreground-muted))]">
          {activeCard.tips.map((tip, idx) => (
            <p key={idx} className="flex items-start gap-2">
              <span className="text-[rgb(var(--accent))]">💡</span>
              <span>{tip}</span>
            </p>
          ))}
        </div>
      )}
    </div>
  );
});

SettingsHelpContent.displayName = "SettingsHelpContent";
