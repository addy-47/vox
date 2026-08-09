import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { HexColorPicker } from "react-colorful";
import { Palette, Sun, Moon } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, SegmentedControl } from "@/shared/ui";

interface AppearanceCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

const THEME_OPTIONS = [
  { id: "dark", icon: Moon, title: "Dark Mode" },
  { id: "light", icon: Sun, title: "Light Mode" },
];

export const AppearanceCard = memo(({ layoutMode = "full-max" }: AppearanceCardProps) => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  if (!draftSettings) return null;
  const { ui } = draftSettings;

  const isSmall = layoutMode === "small";
  const isMin = layoutMode === "full-min";

  return (
    <Card 
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between select-none",
        !isSmall && cn(
          "p-5 min-h-[180px] h-full",
          isMin ? "lg:w-[240px] xl:w-[260px] 2xl:w-[280px]" : "lg:w-[290px] xl:w-[310px]"
        )
      )}
    >
      {/* Top Row: Header Title & Simple Theme Mode Switcher side-by-side */}
      <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
        <div className="flex items-center gap-2">
          <Palette className="text-[rgb(var(--accent))]" size={16} />
          <span className="text-[12px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
            Appearance Settings
          </span>
        </div>

        {/* Theme Mode Switcher */}
        <SegmentedControl
          options={THEME_OPTIONS}
          value={ui.theme}
          onChange={(theme) => updateDraft("ui", "theme", theme)}
          size="sm"
        />
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
    </Card>
  );
});

AppearanceCard.displayName = "AppearanceCard";

