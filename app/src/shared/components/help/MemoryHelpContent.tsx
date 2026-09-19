import { memo } from "react";
import { MEMORY_PAGE_HELP } from "@/data/helpCopy";
import { HelpControlCard } from "./HelpControlCard";
import { HelpMemoryDiagram } from "./HelpMemoryDiagram";
import { ErrorBoundary } from "@/shared/components/common";

export const MemoryHelpContent = memo(() => {
  return (
    <div className="flex flex-col gap-4 select-none font-sans">
      {/* ── Subtitle intro ── */}
      <p className="text-[13px] leading-relaxed text-[rgb(var(--foreground-muted))]">
        {MEMORY_PAGE_HELP.subtitle}
      </p>

      {/* ── Accent-tinted Section Divider ── */}
      <div className="h-px bg-gradient-to-r from-[rgba(var(--accent),0.45)] via-[rgba(var(--accent),0.2)] to-transparent my-0.5" />

      {/* ── Main Section Container (Subtle minimal border providing structure) ── */}
      <div className="rounded-xl border border-[rgba(var(--border),0.14)] bg-[rgba(var(--foreground),0.015)] p-3.5 flex flex-col gap-3.5">
        {/* ── Visual Diagram: Cognitive Memory Architecture ── */}
        <ErrorBoundary name="HelpMemoryDiagram">
          <HelpMemoryDiagram />
        </ErrorBoundary>

        {/* ── Dynamic Sections & Controls ── */}
        {MEMORY_PAGE_HELP.sections.map((section) => (
          <div key={section.heading} className="flex flex-col gap-2 pt-4 border-t border-[rgba(var(--border),0.10)] first:border-t-0 first:pt-0">
            <div className="flex flex-col gap-0.5">
              <h4 className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] flex items-center gap-1.5">
                <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
                {section.heading}
              </h4>
              {section.description && (
                <p className="text-[12.5px] leading-relaxed text-[rgb(var(--foreground-muted))]">
                  {section.description}
                </p>
              )}
            </div>

            {/* Controls List (Connected by vertical spine) */}
            {section.controls && section.controls.length > 0 && (
              <div className="flex flex-col pt-4">
                {section.controls.map((item, idx) => (
                  <HelpControlCard
                    key={item.name}
                    item={item}
                    variant="minimal"
                    isLast={idx === section.controls!.length - 1}
                  />
                ))}
              </div>
            )}

            {/* Tips Callout */}
            {section.tips && section.tips.length > 0 && (
              <div className="border-l-2 border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.04)] pl-3.5 pr-3 py-2 rounded-r-xl flex flex-col gap-1 text-[12px] text-[rgb(var(--foreground-muted))] border-y border-r border-[rgba(var(--border),0.08)] mt-1">
                {section.tips.map((tip) => (
                  <p key={tip} className="leading-relaxed">
                    💡 {tip}
                  </p>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
});

MemoryHelpContent.displayName = "MemoryHelpContent";
