import { memo, useState, useEffect, useCallback } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { HexColorPicker } from "react-colorful";
import { Palette, Sun, Moon } from "lucide-react";
import { cn, hexToRgb } from "@/shared/lib/utils";
import { Card, SegmentedControl } from "@/shared/ui";

interface AppearanceCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

const THEME_OPTIONS = [
  { id: "dark", icon: Moon, title: "Dark Mode" },
  { id: "light", icon: Sun, title: "Light Mode" },
];

export const AppearanceCard = memo(({ layoutMode = "full-max" }: AppearanceCardProps) => {
  const appearance = useSettingsStore((s) => s.draftSettings?.appearance);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const [localColor, setLocalColor] = useState(appearance?.accent_seed || "#00dbe9");

  useEffect(() => {
    if (appearance?.accent_seed && appearance.accent_seed !== localColor) {
      setLocalColor(appearance.accent_seed);
    }
  }, [appearance?.accent_seed]);

  const handleColorChange = useCallback((color: string) => {
    setLocalColor(color);
    if (typeof document !== "undefined") {
      document.documentElement.style.setProperty("--accent", hexToRgb(color));
    }
  }, []);

  const handlePointerUp = useCallback(() => {
    if (appearance && localColor !== appearance.accent_seed) {
      updateDraft("appearance", "accent_seed", localColor);
    }
  }, [appearance, localColor, updateDraft]);

  if (!appearance) return null;

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
          <span className="font-display text-[12px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
            Appearance
          </span>
        </div>

        {/* Theme Mode Switcher */}
        <SegmentedControl
          options={THEME_OPTIONS}
          value={appearance.theme}
          onChange={(theme) => updateDraft("appearance", "theme", theme)}
          size="sm"
        />
      </div>

      {/* Bottom Row: Full card width color picker */}
      <div
        className="flex-1 flex items-center justify-center min-h-0 pt-0.5"
        onPointerUp={handlePointerUp}
      >
        <HexColorPicker
          color={localColor}
          onChange={handleColorChange}
          className="custom-color-picker w-full"
          style={{ width: "100%", height: "92px" }}
        />
      </div>
    </Card>
  );
});

AppearanceCard.displayName = "AppearanceCard";

