import { useState, memo } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { UserCircle } from "lucide-react";
import { cn } from "@/shared/lib/utils";

interface PersonaCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const PersonaCard = memo(({ layoutMode = "full-max" }: PersonaCardProps) => {
  const { draftSettings, updateDraft } = useSettings();
  const [activeTab, setActiveTab] = useState<"hindi" | "english">("hindi");

  if (!draftSettings) return null;

  const isSmall = layoutMode === "small";

  return (
    <div 
      className={cn(
        "text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85",
        isSmall
          ? "w-full bg-transparent p-0"
          : "w-full lg:w-[460px] glass-card glass-base p-5"
      )}
    >
      {/* Header & Tabs */}
      <div className="flex items-center justify-between mb-4 shrink-0">
        {!isSmall ? (
          <div className="flex items-center gap-2">
            <UserCircle className="text-[rgb(var(--accent))]" size={16} />
            <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/80">
              Persona Settings
            </span>
          </div>
        ) : (
          <div /> // spacing helper
        )}
        
        {/* Simple hi/eng tabs */}
        <div className="flex items-center bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.08)] p-0.5 rounded-lg">
          <button
            onClick={() => setActiveTab("hindi")}
            className={cn(
              "px-2 py-0.5 rounded text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
              activeTab === "hindi"
                ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-sm"
                : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
            )}
          >
            hi
          </button>
          <button
            onClick={() => setActiveTab("english")}
            className={cn(
              "px-2 py-0.5 rounded text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
              activeTab === "english"
                ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-sm"
                : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
            )}
          >
            eng
          </button>
        </div>
      </div>

      {/* Text Area Content */}
      <div className={cn("pt-2 flex flex-col justify-between", isSmall ? "min-h-[200px]" : "min-h-[140px]")}>
        {activeTab === "hindi" && (
          <div className="space-y-2.5">
            <textarea
              value={draftSettings.assistant.hindi_prompt}
              onChange={(e) => updateDraft("assistant", "hindi_prompt", e.target.value)}
              rows={layoutMode === "full-max" ? 7 : isSmall ? 8 : 5}
              className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.12)] rounded-xl px-3 py-2 text-[12px] text-[rgb(var(--foreground))]/80 font-mono leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.35)] transition-colors"
              placeholder="Hindi prompt..."
              spellCheck={false}
            />
          </div>
        )}

        {activeTab === "english" && (
          <div className="space-y-2.5">
            <textarea
              value={draftSettings.assistant.english_prompt}
              onChange={(e) => updateDraft("assistant", "english_prompt", e.target.value)}
              rows={layoutMode === "full-max" ? 7 : isSmall ? 8 : 5}
              className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.12)] rounded-xl px-3 py-2 text-[12px] text-[rgb(var(--foreground))]/80 font-mono leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.35)] transition-colors"
              placeholder="English prompt..."
              spellCheck={false}
            />
          </div>
        )}
      </div>
    </div>
  );
});

PersonaCard.displayName = "PersonaCard";
