import { memo, useState, useEffect, useCallback, useRef } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { HexColorPicker } from "react-colorful";
import { Palette, Sun, Moon } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, SegmentedControl } from "@/shared/ui";
import { beginAccentPreview, endAccentPreview, previewAccent } from "@/shared/theme";
import { APPEARANCE_COPY } from "@/data/settingsCopy";

interface AppearanceCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

const THEME_OPTIONS = [
  { id: "dark", icon: Moon, title: APPEARANCE_COPY.darkMode },
  { id: "light", icon: Sun, title: APPEARANCE_COPY.lightMode },
];

export const AppearanceCard = memo(({ layoutMode = "full-max" }: AppearanceCardProps) => {
  const appearance = useSettingsStore((s) => s.draftSettings?.appearance);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const [localColor, setLocalColor] = useState(appearance?.accent_seed || "#00dbe9");
  const draggingRef = useRef(false);
  // Live refs: the release listener must read the newest colour without being
  // re-attached on every drag frame.
  const localColorRef = useRef(localColor);
  localColorRef.current = localColor;
  const pendingSeedRef = useRef(appearance?.accent_seed ?? null);

  useEffect(() => {
    if (appearance?.accent_seed && appearance.accent_seed !== localColor) {
      setLocalColor(appearance.accent_seed);
    }
  }, [appearance?.accent_seed]);

  /**
   * Ends the gesture: commit, then release the gate. Committing first lets the
   * theme module's `applyTheme` run while the preview still holds the gate, so
   * the attribute is written once for the whole gesture instead of being closed
   * and reopened (two extra full-document style recalcs).
   */
  const commitAccent = useCallback(() => {
    if (!draggingRef.current) return;
    draggingRef.current = false;
    window.removeEventListener("pointerup", commitAccent);
    window.removeEventListener("mouseup", commitAccent);
    window.removeEventListener("pointercancel", commitAccent);
    const next = localColorRef.current;
    if (pendingSeedRef.current && next !== pendingSeedRef.current) {
      updateDraft("appearance", "accent_seed", next);
    }
    endAccentPreview();
  }, [updateDraft]);

  // The preview gate must not outlive the card, or a drag interrupted by a
  // route change would leave every transition and backdrop-filter in the app
  // suspended indefinitely.
  useEffect(
    () => () => {
      draggingRef.current = false;
      endAccentPreview();
    },
    []
  );

  /**
   * Live accent preview. `previewAccent` is rAF-coalesced and holds the repaint
   * gate open for the duration of the drag, so the write costs one document
   * style recalc per frame instead of one per mousemove event.
   *
   * This used to be a bare `documentElement.style.setProperty` on every event:
   * unthrottled, outside the theme module, and with no notification-token sync,
   * so the seven `--notif-*` colours sat frozen at the old accent for the whole
   * drag and then jumped.
   *
   * The release is bound to `window`, not just the wrapper's `onPointerUp`:
   * react-colorful tracks the drag with a `document` mouseup listener, so a
   * gesture that ends outside the card never fires the wrapper handler — and an
   * unclosed gate leaves all 237 `transition-all` sites and every
   * `backdrop-filter` region suspended app-wide.
   */
  const handleColorChange = useCallback((color: string) => {
    setLocalColor(color);
    if (!draggingRef.current) {
      draggingRef.current = true;
      beginAccentPreview();
      window.addEventListener("pointerup", commitAccent);
      window.addEventListener("mouseup", commitAccent);
      window.addEventListener("pointercancel", commitAccent);
    }
    previewAccent(color);
  }, [commitAccent]);

  const handleThemeChange = useCallback(
    (theme: string) => {
      updateDraft("appearance", "theme", theme);
    },
    [updateDraft]
  );

  if (!appearance) return null;

  const isSmall = layoutMode === "small";
  const isMin = layoutMode === "full-min";

  return (
    <Card 
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between select-none",
        isSmall
          ? "w-full h-auto gap-3.5"
          : cn(
              "p-5 min-h-[180px] h-full",
              isMin ? "lg:w-[240px] xl:w-[260px] 2xl:w-[280px]" : "lg:w-[290px] xl:w-[310px]"
            )
      )}
    >
      {/* Top Row: Header Title & Simple Theme Mode Switcher side-by-side */}
      <div className="flex items-center justify-between mb-2 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
        <div className="flex items-center gap-2">
          <Palette className="text-[rgb(var(--accent))]" size={17} />
          <span className="font-display text-[13px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
            {APPEARANCE_COPY.cardTitle}
          </span>
        </div>

        {/* Theme Mode Switcher */}
        <SegmentedControl
          options={THEME_OPTIONS}
          value={appearance.theme}
          onChange={handleThemeChange}
          size="sm"
        />
      </div>

      {/* Bottom Row: Full card width color picker */}
      <div
        className="w-full flex items-center justify-center pt-1 pb-1"
        onPointerUp={commitAccent}
      >
        <HexColorPicker
          color={localColor}
          onChange={handleColorChange}
          className="custom-color-picker w-full"
          style={{ width: "100%", height: isSmall ? "130px" : "92px" }}
        />
      </div>
    </Card>
  );
});

AppearanceCard.displayName = "AppearanceCard";

