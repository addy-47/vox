import { useState, memo } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { UserCircle } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import ReactMarkdown from "react-markdown";
import { Card, SegmentedControl } from "@/shared/ui";

interface PersonaCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

const INSTRUCTION_TABS = [
  { id: "modular" as const, label: "Modular" },
  { id: "realtime" as const, label: "Realtime" },
];

const VIEW_TABS = [
  { id: "edit" as const, label: "Edit" },
  { id: "preview" as const, label: "Preview" },
];

const MarkdownComponents = {
  h1: ({node, ...props}: any) => <h1 className="text-[14px] font-bold mt-2 mb-1 text-[rgb(var(--accent))]" {...props} />,
  h2: ({node, ...props}: any) => <h2 className="text-[13px] font-bold mt-2 mb-1 text-[rgb(var(--accent))]" {...props} />,
  h3: ({node, ...props}: any) => <h3 className="text-[12px] font-bold mt-1.5 mb-1 text-[rgb(var(--accent))]" {...props} />,
  p: ({node, ...props}: any) => <p className="mb-2 last:mb-0 w-full" {...props} />,
  ul: ({node, ...props}: any) => <ul className="list-disc list-inside mb-2 pl-2 space-y-0.5" {...props} />,
  ol: ({node, ...props}: any) => <ol className="list-decimal list-inside mb-2 pl-2 space-y-0.5" {...props} />,
  li: ({node, ...props}: any) => <li className="ml-1" {...props} />,
  code: ({node, ...props}: any) => <code className="bg-[rgba(var(--foreground),0.06)] px-1 py-0.5 rounded font-mono text-[11px]" {...props} />,
  pre: ({node, ...props}: any) => <pre className="bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--accent),0.1)] rounded-lg p-2 font-mono text-[11px] overflow-x-auto my-2 w-full" {...props} />,
};

export const PersonaCard = memo(({ layoutMode = "full-max" }: PersonaCardProps) => {
  const { draftSettings, updateDraft } = useSettings();
  const [activeTab, setActiveTab] = useState<"modular" | "realtime">("modular");
  const [viewMode, setViewMode] = useState<"edit" | "preview">("edit");

  if (!draftSettings) return null;

  const isSmall = layoutMode === "small";

  return (
    <Card 
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85",
        !isSmall && cn(
          "p-5",
          layoutMode === "full-min" ? "lg:w-[320px] xl:w-[380px] 2xl:w-[460px]" : "lg:w-[460px]"
        )
      )}
    >
      {/* Header & Tabs */}
      {isSmall ? (
        <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full gap-2">
          <span className="text-[10px] font-semibold tracking-wider text-[rgb(var(--foreground-muted))]/70">Instruction Mode</span>
          <div className="flex items-center gap-2">
            <SegmentedControl options={INSTRUCTION_TABS} value={activeTab} onChange={setActiveTab} size="sm" />
            <SegmentedControl options={VIEW_TABS} value={viewMode} onChange={setViewMode} size="sm" />
          </div>
        </div>
      ) : (
        <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full gap-2">
          <div className="flex items-center gap-2">
            <UserCircle className="text-[rgb(var(--accent))]" size={18} />
            <span className="text-[12px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
              Persona Settings
            </span>
          </div>
          
          <div className="flex items-center gap-3">
            <SegmentedControl options={INSTRUCTION_TABS} value={activeTab} onChange={setActiveTab} size="sm" />
            <SegmentedControl options={VIEW_TABS} value={viewMode} onChange={setViewMode} size="sm" />
          </div>
        </div>
      )}

      {/* Text Area Content */}
      <div className={cn("pt-2 flex flex-col justify-between", isSmall ? "min-h-[200px]" : "min-h-[140px]")}>
        {activeTab === "modular" && (
          <div className="space-y-2 flex-1 flex flex-col justify-between">
            {viewMode === "edit" ? (
              <textarea
                value={draftSettings.assistant.modular_prompt}
                onChange={(e) => updateDraft("assistant", "modular_prompt", e.target.value)}
                rows={layoutMode === "full-max" ? 6 : isSmall ? 8 : 4}
                className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.12)] rounded-xl px-3 py-2 text-[12px] text-[rgb(var(--foreground))]/80 font-mono leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.35)] transition-colors flex-1"
                placeholder="Modular instruction prompt..."
                spellCheck={false}
              />
            ) : (
              <div 
                className={cn(
                  "w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.12)] rounded-xl px-3 py-2 text-[12px] text-[rgb(var(--foreground))]/80 leading-relaxed font-sans overflow-y-auto select-text prose prose-invert max-w-none text-left",
                  layoutMode === "full-max" ? "h-[154px]" : isSmall ? "h-[194px]" : "h-[104px]"
                )}
              >
                <ReactMarkdown components={MarkdownComponents}>{draftSettings.assistant.modular_prompt}</ReactMarkdown>
              </div>
            )}
            <p className="text-[10px] text-[rgb(var(--foreground-muted))]/60 leading-normal font-semibold uppercase tracking-wide">
              Supports <code className="text-[rgb(var(--accent))] font-mono">&lt;lang&gt;</code> and <code className="text-[rgb(var(--accent))] font-mono">&lt;script&gt;</code> template variables, dynamically resolved based on user speech language.
            </p>
          </div>
        )}

        {activeTab === "realtime" && (
          <div className="space-y-2 flex-1 flex flex-col justify-between">
            {viewMode === "edit" ? (
              <textarea
                value={draftSettings.assistant.realtime_prompt}
                onChange={(e) => updateDraft("assistant", "realtime_prompt", e.target.value)}
                rows={layoutMode === "full-max" ? 6 : isSmall ? 8 : 4}
                className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.12)] rounded-xl px-3 py-2 text-[12px] text-[rgb(var(--foreground))]/80 font-mono leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.35)] transition-colors flex-1"
                placeholder="Realtime instruction prompt..."
                spellCheck={false}
              />
            ) : (
              <div 
                className={cn(
                  "w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.12)] rounded-xl px-3 py-2 text-[12px] text-[rgb(var(--foreground))]/80 leading-relaxed font-sans overflow-y-auto select-text prose prose-invert max-w-none text-left",
                  layoutMode === "full-max" ? "h-[154px]" : isSmall ? "h-[194px]" : "h-[104px]"
                )}
              >
                <ReactMarkdown components={MarkdownComponents}>{draftSettings.assistant.realtime_prompt}</ReactMarkdown>
              </div>
            )}
            <p className="text-[10px] text-[rgb(var(--foreground-muted))]/60 leading-normal font-semibold uppercase tracking-wide">
              Instructions supplied to duplex cloud speech-to-speech models (e.g. Gemini Live).
            </p>
          </div>
        )}
      </div>
    </Card>
  );
});

PersonaCard.displayName = "PersonaCard";
