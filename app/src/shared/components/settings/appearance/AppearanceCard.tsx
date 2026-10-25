import { memo, useState, useEffect, useCallback, useRef } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { HexColorPicker } from "react-colorful";
import { Palette, Copy, Check } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, Tooltip } from "@/shared/ui";
import { beginAccentPreview, endAccentPreview, previewAccent } from "@/shared/theme";
import { APPEARANCE_COPY } from "@/data/settingsCopy";
import { copyToClipboard } from "@/shared/lib/clipboard";

interface AppearanceCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const AppearanceCard = memo(({ layoutMode = "full-max" }: AppearanceCardProps) => {
  const appearance = useSettingsStore((s) => s.draftSettings?.appearance);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const [localColor, setLocalColor] = useState(appearance?.accent_seed || "#00dbe9");
  const [copied, setCopied] = useState(false);
  const draggingRef = useRef(false);
  const colorRafRef = useRef<number | null>(null);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Live refs: the release listener must read the newest colour without being
  // re-attached on every drag frame.
  const localColorRef = useRef(localColor);
  localColorRef.current = localColor;
  const pendingSeedRef = useRef(appearance?.accent_seed ?? null);

  useEffect(() => {
    if (draggingRef.current) return;
    if (appearance?.accent_seed && appearance.accent_seed !== localColorRef.current) {
      setLocalColor(appearance.accent_seed);
      pendingSeedRef.current = appearance.accent_seed;
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
    if (colorRafRef.current !== null) {
      cancelAnimationFrame(colorRafRef.current);
      colorRafRef.current = null;
    }
    window.removeEventListener("pointerup", commitAccent);
    window.removeEventListener("mouseup", commitAccent);
    window.removeEventListener("pointercancel", commitAccent);
    const next = localColorRef.current;
    setLocalColor(next);
    if (next && next !== pendingSeedRef.current) {
      pendingSeedRef.current = next;
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
      if (colorRafRef.current !== null) {
        cancelAnimationFrame(colorRafRef.current);
      }
      if (copyTimerRef.current !== null) {
        clearTimeout(copyTimerRef.current);
      }
      window.removeEventListener("pointerup", commitAccent);
      window.removeEventListener("mouseup", commitAccent);
      window.removeEventListener("pointercancel", commitAccent);
      endAccentPreview();
    },
    [commitAccent]
  );

  /**
   * Live accent preview. `previewAccent` is rAF-coalesced and holds the repaint
   * gate open for the duration of the drag, so the write costs one document
   * style recalc per frame instead of one per mousemove event.
   */
  const handleColorChange = useCallback((color: string) => {
    localColorRef.current = color;
    if (!draggingRef.current) {
      draggingRef.current = true;
      pendingSeedRef.current =
        useSettingsStore.getState().draftSettings?.appearance?.accent_seed ?? localColorRef.current;
      beginAccentPreview();
      window.addEventListener("pointerup", commitAccent);
      window.addEventListener("mouseup", commitAccent);
      window.addEventListener("pointercancel", commitAccent);
    }
    if (colorRafRef.current === null) {
      colorRafRef.current = requestAnimationFrame(() => {
        colorRafRef.current = null;
        setLocalColor(localColorRef.current);
      });
    }
    previewAccent(color);
  }, [commitAccent]);

  const handleCopy = useCallback(async () => {
    const success = await copyToClipboard(localColor);
    if (success) {
      setCopied(true);
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
      copyTimerRef.current = setTimeout(() => {
        copyTimerRef.current = null;
        setCopied(false);
      }, 1500);
    }
  }, [localColor]);

  if (!appearance) return null;

  const isSmall = layoutMode === "small";

  return (
    <Card 
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between select-none",
        isSmall
          ? "w-full h-auto gap-3.5"
          : "p-5 min-h-[180px] h-full"
      )}
    >
      {/* Top Row: Header Title + Hex Copy */}
      <div className="flex items-center justify-between mb-2 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
        <div className="flex items-center gap-2">
          <Palette className="text-[rgb(var(--accent))]" size={17} />
          <span className="font-display text-[13px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
            {APPEARANCE_COPY.cardTitle}
          </span>
        </div>
        <Tooltip
          label={copied ? APPEARANCE_COPY.copiedHex : APPEARANCE_COPY.copyHex}
          side="bottom"
          asChild
        >
          <button
            type="button"
            onClick={handleCopy}
            className="flex items-center gap-1.5 text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground))]/60 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer bg-transparent border-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))] p-0"
          >
            <span>{localColor}</span>
            {copied ? (
              <Check size={12} className="text-[rgb(var(--accent))]" />
            ) : (
              <Copy size={12} />
            )}
          </button>
        </Tooltip>
      </div>

      {/* Bottom Row: Full card width color picker */}
      <div
        className="w-full flex items-center justify-center px-1 pt-1 pb-1"
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