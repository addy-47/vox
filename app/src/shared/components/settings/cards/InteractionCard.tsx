import { memo } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { Sliders, Lock } from "lucide-react";
import { cn } from "@/shared/lib/utils";

interface InteractionCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const InteractionCard = memo(({ layoutMode = "full-max" }: InteractionCardProps) => {
  const { draftSettings, updateDraft } = useSettings();

  if (!draftSettings) return null;
  const { interaction } = draftSettings;

  const isSmall = layoutMode === "small";
  const isMin = layoutMode === "full-min";

  return (
    <div 
      className={cn(
        "text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85",
        isSmall
          ? "w-full bg-transparent p-0"
          : "w-full lg:w-[340px] bg-transparent lg:bg-black/15 lg:backdrop-blur-md border-0 lg:border border-[rgba(var(--accent),0.10)] rounded-none lg:rounded-2xl p-0 lg:p-5 shadow-none lg:shadow-xl shadow-black/30"
      )}
    >
      {/* Header */}
      {!isSmall && (
        <div className="flex items-center gap-2 mb-4 shrink-0">
          <Sliders className="text-[rgb(var(--accent))]" size={16} />
          <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/80">
            Interaction
          </span>
        </div>
      )}

      <div className="space-y-4">
        {/* Toggle Main App Mode */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col">
            <span className="font-bold text-[rgb(var(--foreground))]/80">App Mode</span>
            {!isMin && (
              <span className="text-[11px] text-[rgb(var(--foreground-muted))]/70">
                {interaction.main_app_mode === "Passive" ? "Continuous listening" : "Push-to-talk"}
              </span>
            )}
          </div>
          <div className="flex p-0.5 bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.08)] rounded-xl">
            <button
              onClick={() => updateDraft("interaction", "main_app_mode", "Passive")}
              className={cn(
                "px-2.5 py-1 rounded-lg text-[11px] font-bold uppercase transition-all duration-300",
                interaction.main_app_mode === "Passive"
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md"
                  : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
            >
              Passive
            </button>
            <button
              onClick={() => updateDraft("interaction", "main_app_mode", "PTT")}
              className={cn(
                "px-2.5 py-1 rounded-lg text-[11px] font-bold uppercase transition-all duration-300",
                interaction.main_app_mode === "PTT"
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md"
                  : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
            >
              PTT
            </button>
          </div>
        </div>

        <div className="border-t border-[rgba(var(--accent),0.06)] my-2" />

        {/* Stub 1: Local / Cloud Mode Selection */}
        <div className="flex items-center justify-between opacity-65 select-none">
          <div className="flex flex-col">
            <div className="flex items-center gap-1.5">
              <span className="font-bold text-[rgb(var(--foreground))]/80">Processing Location</span>
              <span className="text-[11px] font-mono font-black bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] px-1 py-0.5 rounded tracking-wide leading-none">
                PHASE 9
              </span>
            </div>
            {!isMin && (
              <span className="text-[11px] text-[rgb(var(--foreground-muted))]/70">Switch to Cloud API</span>
            )}
          </div>
          <div className="flex p-0.5 bg-[rgba(var(--foreground),0.03)] border border-transparent rounded-xl cursor-not-allowed">
            <span className="px-2.5 py-1 rounded-lg text-[11px] font-bold uppercase bg-[rgba(var(--foreground),0.1)] text-[rgb(var(--foreground-muted))] shadow-sm flex items-center gap-1">
              <Lock size={11} /> Local
            </span>
          </div>
        </div>

        {/* Stub 2: Cloud API Key input */}
        <div className="space-y-1 opacity-65 select-none">
          <div className="flex justify-between items-center">
            <div className="flex items-center gap-1.5">
              <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/80">
                Cloud API Key
              </span>
              <span className="text-[11px] font-mono font-black bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] px-1 py-0.5 rounded tracking-wide leading-none">
                PHASE 9
              </span>
            </div>
          </div>
          <div className="relative">
            <input
              type="password"
              disabled
              value="••••••••••••••••"
              className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.12)] rounded-xl px-3 py-2 text-[12px] text-[rgb(var(--foreground-muted))]/80 font-mono focus:outline-none cursor-not-allowed"
            />
          </div>
        </div>
      </div>
    </div>
  );
});

InteractionCard.displayName = "InteractionCard";
