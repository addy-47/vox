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
        "text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between select-none",
        isSmall
          ? "w-full bg-transparent p-0"
          : cn(
              "w-full glass-card p-5 min-h-[180px] h-full",
              isMin ? "lg:w-[240px] xl:w-[260px] 2xl:w-[280px]" : "lg:w-[290px] xl:w-[310px]"
            )
      )}
    >
      {/* Top Row: Header Title & Simple Theme Mode Switcher side-by-side */}
      <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
        <div className="flex items-center gap-2">
          <Palette className="text-[rgb(var(--accent))]" size={16} />
          <span className="text-[11px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
            Appearance Settings
          </span>
        </div>

        {/* Simple Sun/Moon Icon Toggle Capsule */}
        <div className="flex bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.08)] p-0.5 rounded-xl gap-0.5">
          {[
            { id: "dark", icon: Moon, desc: "Dark Mode" },
            { id: "light", icon: Sun, desc: "Light Mode" },
          ].map((t) => (
            <button
              key={t.id}
              onClick={() => updateDraft("ui", "theme", t.id)}
              className={cn(
                "w-7.5 h-7.5 rounded-lg transition-all duration-300 cursor-pointer border flex items-center justify-center",
                ui.theme === t.id
                  ? "bg-[rgba(var(--accent),0.15)] border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.1)]"
                  : "bg-transparent border-transparent text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
              aria-label={t.desc}
              title={t.desc}
            >
              <t.icon size={14} />
            </button>
          ))}
        </div>
      </div>

      {/* Bottom Row: Full card width color picker */}
      <div className="flex-1 flex items-center justify-center min-h-0 pt-0.5">
        <HexColorPicker
          color={ui.accent_seed}
          onChange={(color) => updateDraft("ui", "accent_seed", color)}
          className="custom-color-picker w-full"
          style={{ width: "100%", height: "92px" }}
        />
      </div>
    </div>
  );
});

AppearanceCard.displayName = "AppearanceCard";
