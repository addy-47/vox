import { memo } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { HexColorPicker } from "react-colorful";
import { Palette, Sun, Moon } from "lucide-react";
import { cn } from "@/shared/lib/utils";

interface AppearanceCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const AppearanceCard = memo(({ layoutMode = "full-max" }: AppearanceCardProps) => {
  const { draftSettings, updateDraft } = useSettings();

  if (!draftSettings) return null;
  const { ui } = draftSettings;

  const isSmall = layoutMode === "small";
  const isMin = layoutMode === "full-min";

  return (
    <div 
      className={cn(
        "text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between",
        isSmall
          ? "w-full bg-transparent p-0"
          : cn(
              "w-full glass-card glass-base p-5",
              layoutMode === "full-min" ? "lg:w-[240px] xl:w-[280px] 2xl:w-[320px]" : "lg:w-[320px]"
            )
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between mb-4 shrink-0">
        {!isSmall ? (
          <div className="flex items-center gap-2">
            <Palette className="text-[rgb(var(--accent))]" size={16} />
            <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/80">
              Appearance
            </span>
          </div>
        ) : (
          <div /> // spacing helper
        )}
        
        {/* Dark/Light Theme toggle */}
        <div className="flex bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.08)] p-0.5 rounded-xl">
          {[
            { id: "dark", icon: Moon },
            { id: "light", icon: Sun },
          ].map((t) => (
            <button
              key={t.id}
              onClick={() => updateDraft("ui", "theme", t.id)}
              className={cn(
                "p-1.5 rounded-lg transition-all duration-300",
                ui.theme === t.id
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md"
                  : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
              aria-label={`Switch to ${t.id} theme`}
            >
              <t.icon size={13} />
            </button>
          ))}
        </div>
      </div>

      {/* HexColorPicker filling card width */}
      <div className={cn("flex-1 flex items-center justify-center py-1", isMin ? "min-h-[125px]" : "min-h-[180px]")}>
        <div className={cn("w-full flex justify-center", !isMin && "custom-color-picker-v2")}>
          <HexColorPicker
            color={ui.accent_seed}
            onChange={(color) => updateDraft("ui", "accent_seed", color)}
            className={cn("custom-color-picker", isMin && "custom-color-picker-min")}
            style={{ 
              width: "100%", 
              height: isMin ? "115px" : (isSmall ? "150px" : "140px") 
            }}
          />
        </div>
      </div>

      {/* Hex value display */}
      <div className="mt-4 p-2.5 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.08)] flex items-center justify-between font-mono text-[11px] shrink-0">
        <span className="text-[rgb(var(--foreground-muted))]/70">ACCENT HEX</span>
        <span className="text-[rgb(var(--accent))] font-bold uppercase">{ui.accent_seed}</span>
      </div>
    </div>
  );
});

AppearanceCard.displayName = "AppearanceCard";
