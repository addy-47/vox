import { memo } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { HexColorPicker } from "react-colorful";
import { Palette, Sun, Moon } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export const AppearanceCard = memo(() => {
  const { draftSettings, updateDraft } = useSettings();

  if (!draftSettings) return null;
  const { ui } = draftSettings;

  return (
    <div className="w-full lg:w-[320px] bg-transparent lg:bg-black/15 lg:backdrop-blur-md border-0 lg:border border-[rgba(var(--accent),0.10)] rounded-none lg:rounded-2xl p-0 lg:p-5 shadow-none lg:shadow-xl shadow-black/30 text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between">
      {/* Header */}
      <div className="flex items-center justify-between mb-4 shrink-0">
        <div className="flex items-center gap-2">
          <Palette className="text-[rgb(var(--accent))]" size={16} />
          <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/80">
            Appearance
          </span>
        </div>
        
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
      <div className="flex-1 flex items-center justify-center py-2 min-h-[190px]">
        <div className="w-full flex justify-center custom-color-picker-v2">
          <HexColorPicker
            color={ui.accent_seed}
            onChange={(color) => updateDraft("ui", "accent_seed", color)}
            style={{ width: "100%", height: "150px" }}
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
