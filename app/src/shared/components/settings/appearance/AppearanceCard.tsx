import { memo, useState, useEffect, useCallback, useRef } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import {
  Palette,
  Copy,
  Check,
  SlidersHorizontal,
  ChevronRight,
  ChevronLeft,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, Tooltip } from "@/shared/ui";
import { CardTitleBar } from "../shared/CardTitleBar";
import {
  PRESET_THEMES,
  CURATED_ACCENT_PRESETS,
  DARK_BG_PRESETS,
  LIGHT_BG_PRESETS,
  getActiveAccent,
  getActiveBg,
  previewAccent,
  previewBackground,
  beginAccentPreview,
  endAccentPreview,
  beginBackgroundPreview,
  endBackgroundPreview,
  DEFAULT_DARK_ACCENT,
  DEFAULT_LIGHT_ACCENT,
  DEFAULT_DARK_BG,
  DEFAULT_LIGHT_BG,
} from "@/shared/theme";
import { APPEARANCE_COPY } from "@/data/settingsCopy";
import { copyToClipboard } from "@/shared/lib/clipboard";
import { hexToRgb } from "@/shared/lib/utils";
import { OklchDonutPicker } from "./OklchDonutPicker";

interface AppearanceCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

type View = "presets" | "custom";
type CustomMode = "accent" | "background";

export const AppearanceCard = memo(({ layoutMode = "full-max" }: AppearanceCardProps) => {
  const appearance = useSettingsStore((s) => s.draftSettings?.appearance);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const [view, setView] = useState<View>("presets");
  const [customMode, setCustomMode] = useState<CustomMode>("accent");

  const isLight = appearance?.theme === "light";
  const activeAccent = appearance
    ? getActiveAccent(appearance)
    : isLight
    ? DEFAULT_LIGHT_ACCENT
    : DEFAULT_DARK_ACCENT;
  const activeBg = appearance
    ? getActiveBg(appearance)
    : isLight
    ? DEFAULT_LIGHT_BG
    : DEFAULT_DARK_BG;

  const [localAccent, setLocalAccent] = useState(activeAccent);
  const [localBg, setLocalBg] = useState(activeBg);
  const [copied, setCopied] = useState(false);

  const draggingRef = useRef(false);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    setLocalAccent(activeAccent);
  }, [activeAccent]);

  useEffect(() => {
    setLocalBg(activeBg);
  }, [activeBg]);

  useEffect(() => {
    return () => {
      if (copyTimerRef.current !== null) clearTimeout(copyTimerRef.current);
    };
  }, []);

  const handleCopyCurrentHex = useCallback(async () => {
    const val = customMode === "accent" ? localAccent : localBg;
    const success = await copyToClipboard(val);
    if (success) {
      setCopied(true);
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
      copyTimerRef.current = setTimeout(() => {
        copyTimerRef.current = null;
        setCopied(false);
      }, 1500);
    }
  }, [customMode, localAccent, localBg]);

  // ── Preset Selection ──
  const handleSelectTheme = useCallback(
    (themeId: string) => {
      const match = PRESET_THEMES.find((p) => p.id === themeId);
      if (!match) return;
      updateDraft("appearance", "preset", match.id);
      updateDraft("appearance", "dark_accent", match.dark.accent);
      updateDraft("appearance", "light_accent", match.light.accent);
      updateDraft("appearance", "dark_bg", match.dark.bg);
      updateDraft("appearance", "light_bg", match.light.bg);
      const nextAccent = isLight ? match.light.accent : match.dark.accent;
      const nextBg = isLight ? match.light.bg : match.dark.bg;
      setLocalAccent(nextAccent);
      setLocalBg(nextBg);
      previewAccent(nextAccent);
      previewBackground(nextBg);
    },
    [updateDraft, isLight]
  );

  // ── Accent Color Handlers ──
  const handleAccentChange = useCallback(
    (hex: string) => {
      const normalized = hex.toUpperCase();
      setLocalAccent(normalized);
      if (!draggingRef.current) {
        draggingRef.current = true;
        beginAccentPreview();
      }
      previewAccent(normalized);
    },
    []
  );

  const commitAccent = useCallback(() => {
    if (draggingRef.current) {
      draggingRef.current = false;
      endAccentPreview();
    }
    updateDraft("appearance", "preset", "custom");
    updateDraft("appearance", isLight ? "light_accent" : "dark_accent", localAccent);
  }, [updateDraft, isLight, localAccent]);

  const handleAccentSwatchSelect = useCallback(
    (hex: string) => {
      const normalized = hex.toUpperCase();
      setLocalAccent(normalized);
      updateDraft("appearance", "preset", "custom");
      updateDraft("appearance", isLight ? "light_accent" : "dark_accent", normalized);
      previewAccent(normalized);
    },
    [updateDraft, isLight]
  );

  // ── Background Color Handlers ──
  const handleBgChange = useCallback(
    (hex: string) => {
      const normalized = hex.toUpperCase();
      setLocalBg(normalized);
      if (!draggingRef.current) {
        draggingRef.current = true;
        beginBackgroundPreview();
      }
      previewBackground(normalized);
    },
    []
  );

  const commitBg = useCallback(() => {
    if (draggingRef.current) {
      draggingRef.current = false;
      endBackgroundPreview();
    }
    updateDraft("appearance", "preset", "custom");
    updateDraft("appearance", isLight ? "light_bg" : "dark_bg", localBg);
  }, [updateDraft, isLight, localBg]);

  const handleBgPresetSelect = useCallback(
    (hex: string) => {
      const normalized = hex.toUpperCase();
      setLocalBg(normalized);
      updateDraft("appearance", "preset", "custom");
      updateDraft("appearance", isLight ? "light_bg" : "dark_bg", normalized);
      previewBackground(normalized);
    },
    [updateDraft, isLight]
  );

  const toggleMode = useCallback(() => {
    setCustomMode((prev) => (prev === "accent" ? "background" : "accent"));
  }, []);

  // Cleanup preview gates if unmounted during drag
  useEffect(() => {
    return () => {
      if (draggingRef.current) {
        draggingRef.current = false;
        endAccentPreview();
        endBackgroundPreview();
      }
    };
  }, []);

  if (!appearance) return null;

  const isSmall = layoutMode === "small";
  const bgPresets = isLight ? LIGHT_BG_PRESETS : DARK_BG_PRESETS;
  const currentPresetId = appearance.preset || "vox";
  const isCustom = currentPresetId === "custom";

  return (
    <Card
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between select-none",
        isSmall
          ? "w-full h-auto p-3.5 gap-3"
          : cn(
              "p-4 sm:p-5",
              layoutMode === "full-max"
                ? "h-[280px] max-h-[280px]"
                : "h-[230px] max-h-[230px]"
            )
      )}
    >
      {/* ── Quiet Breadcrumb Title Bar ── */}
      <CardTitleBar
        icon={<Palette size={17} />}
        title={view === "custom" ? APPEARANCE_COPY.customTitle : APPEARANCE_COPY.cardTitle}
        onBack={view === "custom" ? () => setView("presets") : undefined}
        action={
          view === "custom" ? (
            <div className="flex items-center gap-1.5 shrink-0">
              <Tooltip
                label={copied ? APPEARANCE_COPY.copiedHex : APPEARANCE_COPY.copyHex}
                side="bottom"
                asChild
              >
                <button
                  type="button"
                  onClick={handleCopyCurrentHex}
                  className="flex items-center gap-1.5 px-2 py-1 rounded-lg bg-[rgba(var(--card),0.6)] border border-[rgba(var(--border),0.1)] hover:border-[rgba(var(--accent),0.4)] transition-all cursor-pointer group shrink-0"
                  aria-label={copied ? APPEARANCE_COPY.copiedHex : APPEARANCE_COPY.copyHex}
                >
                  <div
                    className="w-2.5 h-2.5 rounded-full border border-[rgba(var(--border),0.2)] shrink-0 shadow-xs"
                    style={{
                      backgroundColor: customMode === "accent" ? localAccent : localBg,
                    }}
                  />
                  <span className="text-[10px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80 select-all">
                    {customMode === "accent" ? localAccent : localBg}
                  </span>
                  {copied ? (
                    <Check size={10} className="text-[rgb(var(--accent))]" />
                  ) : (
                    <Copy
                      size={10}
                    className="text-[rgb(var(--foreground-muted))]/50 group-hover:text-[rgb(var(--accent))] transition-colors"
                    />
                  )}
                </button>
              </Tooltip>
            </div>
          ) : undefined
        }
        actionLabel={view === "presets" ? APPEARANCE_COPY.customise : undefined}
        onAction={view === "presets" ? () => setView("custom") : undefined}
      />

      {/* ── View 1: Studio Presets Grid ── */}
      {view === "presets" ? (
        <div className="grid grid-cols-2 gap-2 flex-1 min-h-0 overflow-y-auto custom-scrollbar pr-0.5">
          {PRESET_THEMES.map((theme) => {
            const isActive = currentPresetId === theme.id;
            const target = isLight ? theme.light : theme.dark;
            return (
              <button
                key={theme.id}
                type="button"
                onClick={() => handleSelectTheme(theme.id)}
                className={cn(
                  "flex items-center justify-between p-2 rounded-xl border text-left transition-all cursor-pointer group focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
                  isActive
                    ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.08)]"
                    : "border-[rgba(var(--border),0.08)] bg-[rgba(var(--card),0.35)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--card),0.55)]"
                )}
              >
                <div className="flex flex-col min-w-0 pr-1.5">
                  <span
                    className={cn(
                      "text-[11.5px] font-bold tracking-tight truncate",
                      isActive ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]"
                    )}
                  >
                    {theme.label}
                  </span>
                  <span className="text-[9.5px] truncate text-[rgb(var(--foreground-muted))]">
                    {target.name}
                  </span>
                </div>

                {/* Tactile Palette Swatch Dots */}
                <div className="flex items-center gap-1.5 shrink-0">
                  <div
                    className="w-4 h-4 rounded-full border border-[rgba(var(--border),0.15)] shadow-xs flex items-center justify-center"
                    style={{ backgroundColor: target.bg }}
                  >
                    <div
                      className="w-1.5 h-1.5 rounded-full"
                      style={{ backgroundColor: target.accent }}
                    />
                  </div>
                  <div
                    className="w-2.5 h-2.5 rounded-full shadow-xs"
                    style={{ backgroundColor: target.accentComp }}
                  />
                  {isActive ? (
                    <Check
                      size={12}
                      className="text-[rgb(var(--accent))] shrink-0 ml-0.5"
                    />
                  ) : (
                    <div className="w-[12px] ml-0.5" />
                  )}
                </div>
              </button>
            );
          })}

          {/* Custom Palette Tile */}
          <button
            type="button"
            onClick={() => setView("custom")}
            className={cn(
              "flex items-center justify-between p-2 rounded-xl border text-left transition-all cursor-pointer group focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
              isCustom
                ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.08)]"
                : "border-[rgba(var(--border),0.08)] bg-[rgba(var(--card),0.35)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--card),0.55)]"
            )}
          >
            <div className="flex flex-col min-w-0 pr-1.5">
              <span
                className={cn(
                  "text-[11.5px] font-bold tracking-tight truncate",
                  isCustom ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]"
                )}
              >
                {APPEARANCE_COPY.customThemeLabel}
              </span>
              <span className="text-[9.5px] text-[rgb(var(--foreground-muted))] truncate">
                {APPEARANCE_COPY.customThemeSubtitle}
              </span>
            </div>

            <div className="flex items-center gap-1.5 shrink-0">
              <div
                className="w-4 h-4 rounded-full border border-[rgba(var(--border),0.15)] shadow-xs flex items-center justify-center"
                style={{ backgroundColor: localBg }}
              >
                <div
                  className="w-1.5 h-1.5 rounded-full"
                  style={{ backgroundColor: localAccent }}
                />
              </div>
              <SlidersHorizontal
                size={11}
                className="text-[rgb(var(--foreground-muted))]/70 group-hover:text-[rgb(var(--accent))] transition-colors"
              />
              {isCustom ? (
                <Check
                  size={12}
                  className="text-[rgb(var(--accent))] shrink-0 ml-0.5"
                />
              ) : (
                <ChevronRight
                  size={12}
                  className="text-[rgb(var(--foreground-muted))]/40 ml-0.5 group-hover:text-[rgb(var(--accent))] transition-colors"
                />
              )}
            </div>
          </button>
        </div>
      ) : (
        /* ── View 2: Custom Color Studio with Two-Ring Donut Carousel + 4 Presets ── */
        <div
          className={cn(
            "flex-1 min-h-0 flex gap-3 pt-1 items-stretch",
            isSmall ? "flex-col overflow-y-auto" : "flex-row"
          )}
        >
          {/* Left Column: Two-Ring Donut + Carousel Navigation with Chevrons Below */}
          <div
            className={cn(
              "flex flex-col items-center justify-between shrink-0",
              isSmall ? "w-full gap-2.5" : "w-[170px] sm:w-[185px] h-full"
            )}
          >
            {/* Standard Two-Ring Donut Color Picker */}
            <OklchDonutPicker
              mode={customMode}
              hex={customMode === "accent" ? localAccent : localBg}
              onChange={customMode === "accent" ? handleAccentChange : handleBgChange}
              onCommit={customMode === "accent" ? commitAccent : commitBg}
              isLight={isLight}
              size={layoutMode === "full-min" ? 116 : 126}
            />

            {/* Carousel Navigation with Chevrons and Simple Text Label Below Donut */}
            <div className="flex items-center justify-center gap-1.5 w-full pt-1">
              <button
                type="button"
                onClick={toggleMode}
                className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-colors cursor-pointer"
                aria-label="Previous mode"
              >
                <ChevronLeft size={13} />
              </button>

              <div className="flex items-center gap-1.5 text-[11px] font-bold tracking-tight">
                <button
                  type="button"
                  onClick={() => setCustomMode("accent")}
                  className={cn(
                    "cursor-pointer transition-colors focus-visible:outline focus-visible:outline-1 focus-visible:outline-[rgb(var(--accent))] rounded px-1",
                    customMode === "accent"
                      ? "text-[rgb(var(--accent))]"
                      : "text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  {APPEARANCE_COPY.segmentedAccent}
                </button>
                <span className="text-[rgb(var(--foreground-muted))]/30 text-[9px]">/</span>
                <button
                  type="button"
                  onClick={() => setCustomMode("background")}
                  className={cn(
                    "cursor-pointer transition-colors focus-visible:outline focus-visible:outline-1 focus-visible:outline-[rgb(var(--accent))] rounded px-1",
                    customMode === "background"
                      ? "text-[rgb(var(--accent))]"
                      : "text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  {APPEARANCE_COPY.segmentedBackground}
                </button>
              </div>

              <button
                type="button"
                onClick={toggleMode}
                className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-colors cursor-pointer"
                aria-label="Next mode"
              >
                <ChevronRight size={13} />
              </button>
            </div>
          </div>

          {/* Right Column: 4 Clean Preset Rows */}
          <div className="flex-1 min-w-0 flex flex-col justify-between h-full pl-2 sm:pl-3 border-l border-[rgba(var(--border),0.06)] gap-1.5">
            {customMode === "accent"
              ? CURATED_ACCENT_PRESETS.map((preset) => {
                  const hex = isLight ? preset.lightHex : preset.darkHex;
                  const isSelected = localAccent.toUpperCase() === hex.toUpperCase();
                  return (
                    <button
                      key={preset.id}
                      type="button"
                      onClick={() => handleAccentSwatchSelect(hex)}
                      className={cn(
                        "flex items-center justify-between px-2.5 py-1.5 rounded-xl border text-left transition-all cursor-pointer group focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
                        isSelected
                          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.08)] shadow-2xs"
                          : "border-[rgba(var(--border),0.08)] bg-[rgba(var(--card),0.35)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--card),0.55)]"
                      )}
                      aria-label={`Select accent ${hex}`}
                    >
                      <div className="flex items-center gap-2 min-w-0">
                        <div
                          className={cn(
                            "w-4 h-4 rounded-full border border-[rgba(var(--border),0.2)] shadow-xs shrink-0 flex items-center justify-center transition-transform",
                            isSelected && "scale-105"
                          )}
                          style={{ backgroundColor: hex }}
                        >
                          {isSelected && (
                            <div className="w-1.5 h-1.5 rounded-full bg-white shadow-xs" />
                          )}
                        </div>
                        <span
                          className={cn(
                            "text-[11px] font-bold tracking-tight truncate",
                            isSelected
                              ? "text-[rgb(var(--accent))]"
                              : "text-[rgb(var(--foreground))]"
                          )}
                        >
                          {preset.name}
                        </span>
                      </div>

                      <div className="flex items-center gap-1.5 shrink-0">
                        <span className="text-[9.5px] font-mono text-[rgb(var(--foreground-muted))]">
                          {hex}
                        </span>
                        {isSelected ? (
                          <Check size={11} className="text-[rgb(var(--accent))]" />
                        ) : (
                          <div className="w-[11px]" />
                        )}
                      </div>
                    </button>
                  );
                })
              : bgPresets.map((preset) => {
                  const isSelected = localBg.toUpperCase() === preset.hex.toUpperCase();
                  return (
                    <button
                      key={preset.hex}
                      type="button"
                      onClick={() => handleBgPresetSelect(preset.hex)}
                      className={cn(
                        "flex items-center justify-between px-2.5 py-1.5 rounded-xl border text-left transition-all cursor-pointer group focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
                        isSelected
                          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.08)] shadow-2xs"
                          : "border-[rgba(var(--border),0.08)] bg-[rgba(var(--card),0.35)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--card),0.55)]"
                      )}
                      aria-label={preset.label}
                    >
                      <div className="flex items-center gap-2 min-w-0">
                        <div
                          className="w-4 h-4 rounded-md border flex items-center justify-center shrink-0 shadow-xs"
                          style={{
                            backgroundColor: preset.hex,
                            borderColor: isSelected
                              ? "rgb(var(--accent))"
                              : `rgba(${hexToRgb(preset.fg)}, 0.3)`,
                          }}
                        >
                          <div
                            className="w-2.5 h-2.5 rounded-sm flex items-center justify-center"
                            style={{
                              backgroundColor: `rgba(${hexToRgb(preset.fg)}, 0.18)`,
                            }}
                          >
                            <div
                              className="w-1.5 h-1.5 rounded-full"
                              style={{ backgroundColor: localAccent }}
                            />
                          </div>
                        </div>
                        <span
                          className={cn(
                            "text-[11px] font-bold tracking-tight truncate",
                            isSelected
                              ? "text-[rgb(var(--accent))]"
                              : "text-[rgb(var(--foreground))]"
                          )}
                        >
                          {preset.label}
                        </span>
                      </div>

                      <div className="flex items-center gap-1.5 shrink-0">
                        <span className="text-[9.5px] font-mono text-[rgb(var(--foreground-muted))]">
                          {preset.hex}
                        </span>
                        {isSelected ? (
                          <Check size={11} className="text-[rgb(var(--accent))]" />
                        ) : (
                          <div className="w-[11px]" />
                        )}
                      </div>
                    </button>
                  );
                })}
          </div>
        </div>
      )}
    </Card>
  );
});

AppearanceCard.displayName = "AppearanceCard";
