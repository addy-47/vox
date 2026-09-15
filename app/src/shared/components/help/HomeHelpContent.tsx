import { memo } from "react";
import { HOME_PAGE_HELP } from "@/data/helpCopy";
import { HelpControlCard } from "./HelpControlCard";
import { HelpOrbVisualizer } from "./HelpOrbVisualizer";
import { HelpPttDiagram } from "./HelpPttDiagram";
import { Lightbulb } from "lucide-react";

export const HomeHelpContent = memo(() => {
  return (
    <div className="flex flex-col gap-5 select-none font-sans">
      {/* ── Subtitle intro ── */}
      <p className="text-[13px] leading-relaxed text-[rgb(var(--foreground-muted))]">
        {HOME_PAGE_HELP.subtitle}
      </p>

      {/* ── Visual Simulator: Live Orb Moods ── */}
      <HelpOrbVisualizer />

      {/* ── Push-To-Talk Workflow ── */}
      <HelpPttDiagram />

      {/* ── Sections & Controls ── */}
      {HOME_PAGE_HELP.sections.map((section, idx) => (
        <div key={idx} className="flex flex-col gap-2.5">
          <h3 className="text-[11.5px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-2">
            <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
            {section.heading}
          </h3>
          {section.description && (
            <p className="text-[12px] text-[rgb(var(--foreground-muted))] -mt-1 leading-relaxed">
              {section.description}
            </p>
          )}

          <div className="flex flex-col gap-2">
            {section.controls?.map((item) => (
              <HelpControlCard key={item.name} item={item} />
            ))}
          </div>

          {/* Quick Tips */}
          {section.tips && section.tips.length > 0 && (
            <div className="mt-1 p-3 rounded-xl border border-[rgba(var(--border),0.1)] bg-[rgba(var(--foreground),0.02)] flex items-start gap-2.5">
              <Lightbulb size={15} className="text-[rgb(var(--accent))] shrink-0 mt-0.5" />
              <div className="flex flex-col gap-1 text-[12px] text-[rgb(var(--foreground-muted))]">
                {section.tips.map((t, i) => (
                  <p key={i}>• {t}</p>
                ))}
              </div>
            </div>
          )}
        </div>
      ))}
    </div>
  );
});

HomeHelpContent.displayName = "HomeHelpContent";
