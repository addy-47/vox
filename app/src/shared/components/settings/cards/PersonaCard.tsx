import { useState, memo } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { UserCircle } from "lucide-react";
import { cn } from "@/shared/lib/utils";

interface PersonaCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const PersonaCard = memo(({ layoutMode = "full-max" }: PersonaCardProps) => {
  const { draftSettings, updateDraft } = useSettings();
  const [activeTab, setActiveTab] = useState<"modular" | "realtime">("modular");

  if (!draftSettings) return null;

  const isSmall = layoutMode === "small";

  return (
    <div 
      className={cn(
        "text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85",
        isSmall
          ? "w-full bg-transparent p-0"
          : cn(
              "w-full glass-card p-5",
              layoutMode === "full-min" ? "lg:w-[320px] xl:w-[380px] 2xl:w-[460px]" : "lg:w-[460px]"
            )
      )}
    >
      {/* Header & Tabs */}
      {isSmall ? (
        <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
          <span className="text-[10px] font-semibold tracking-wider text-[rgb(var(--foreground-muted))]/70 uppercase">INSTRUCTION MODE</span>
          <div className="flex items-center bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.08)] p-0.5 rounded-lg">
            <button
              onClick={() => setActiveTab("modular")}
              className={cn(
                "px-2.5 py-0.5 rounded text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
                activeTab === "modular"
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                  : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
            >
              Modular
            </button>
            <button
              onClick={() => setActiveTab("realtime")}
              className={cn(
                "px-2.5 py-0.5 rounded text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
                activeTab === "realtime"
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                  : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
            >
              Realtime
            </button>
          </div>
        </div>
      ) : (
        <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
          <div className="flex items-center gap-2">
            <UserCircle className="text-[rgb(var(--accent))]" size={18} />
            <span className="text-[12px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
              Persona Settings
            </span>
          </div>
          
          {/* Simple modular/realtime tabs */}
          <div className="flex items-center bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.08)] p-0.5 rounded-lg">
            <button
              onClick={() => setActiveTab("modular")}
              className={cn(
                "px-2.5 py-0.5 rounded text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
                activeTab === "modular"
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                  : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
            >
              Modular
            </button>
            <button
              onClick={() => setActiveTab("realtime")}
              className={cn(
                "px-2.5 py-0.5 rounded text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
                activeTab === "realtime"
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                  : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
            >
              Realtime
            </button>
          </div>
        </div>
      )}

      {/* Text Area Content */}
      <div className={cn("pt-2 flex flex-col justify-between", isSmall ? "min-h-[200px]" : "min-h-[140px]")}>
        {activeTab === "modular" && (
          <div className="space-y-2">
            <textarea
              value={draftSettings.assistant.modular_prompt}
              onChange={(e) => updateDraft("assistant", "modular_prompt", e.target.value)}
              rows={layoutMode === "full-max" ? 6 : isSmall ? 8 : 4}
              className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.12)] rounded-xl px-3 py-2 text-[12px] text-[rgb(var(--foreground))]/80 font-mono leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.35)] transition-colors"
              placeholder="Modular instruction prompt..."
              spellCheck={false}
            />
            <p className="text-[10px] text-[rgb(var(--foreground-muted))]/60 leading-normal font-semibold uppercase tracking-wide">
              Supports <code className="text-[rgb(var(--accent))] font-mono">&lt;lang&gt;</code> and <code className="text-[rgb(var(--accent))] font-mono">&lt;script&gt;</code> template variables, dynamically resolved based on user speech language.
            </p>
          </div>
        )}

        {activeTab === "realtime" && (
          <div className="space-y-2">
            <textarea
              value={draftSettings.assistant.realtime_prompt}
              onChange={(e) => updateDraft("assistant", "realtime_prompt", e.target.value)}
              rows={layoutMode === "full-max" ? 6 : isSmall ? 8 : 4}
              className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.12)] rounded-xl px-3 py-2 text-[12px] text-[rgb(var(--foreground))]/80 font-mono leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.35)] transition-colors"
              placeholder="Realtime instruction prompt..."
              spellCheck={false}
            />
            <p className="text-[10px] text-[rgb(var(--foreground-muted))]/60 leading-normal font-semibold uppercase tracking-wide">
              Instructions supplied to duplex cloud speech-to-speech models (e.g. Gemini Live).
            </p>
          </div>
        )}
      </div>
    </div>
  );
});

PersonaCard.displayName = "PersonaCard";
