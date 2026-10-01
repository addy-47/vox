import { memo, useState, useEffect, useCallback, useRef } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Clipboard, Layers, Send, Check, X, CheckCircle2, RotateCcw, Timer, VolumeX, Info, Keyboard, Pencil } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { TriangularLoopSelector, TriangularLoopOption, KeyboardHotkeySkeleton } from "@/shared/ui";
import { DICTATION_COPY } from "@/data/settingsCopy";
import { updateSetting } from "@/services/settingsService";
import type { DictationSettings } from "@/store/settingsStore";

interface DictationConfigDeskProps {
  layoutMode?: "full-max" | "full-min" | "small";
  disabled?: boolean;
}

const DEFAULT_HOTKEY = DICTATION_COPY.defaultHotkey; // "Alt+V"

type DictationOutputMode = DictationSettings["output_mode"];

const OUTPUT_OPTIONS: [
  TriangularLoopOption<DictationOutputMode>,
  TriangularLoopOption<DictationOutputMode>,
  TriangularLoopOption<DictationOutputMode>
] = [
  { id: "paste", label: DICTATION_COPY.modePaste },
  { id: "clipboard", label: DICTATION_COPY.modeClipboard },
  { id: "tray", label: DICTATION_COPY.modeTray },
];

type DictationSubTab = "hotkey" | "destination" | "autostop";

const TABS: Array<{ id: DictationSubTab; label: string }> = [
  { id: "hotkey", label: "Hotkey" },
  { id: "destination", label: "Output Mode" },
  { id: "autostop", label: "Auto Silence" },
];

export const DictationConfigDesk = memo(({ layoutMode, disabled = false }: DictationConfigDeskProps) => {
  const [activeSubTab, setActiveSubTab] = useState<DictationSubTab>("hotkey");
  const dictationDraft = useSettingsStore((s) => s.draftSettings?.dictation);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const dictation = dictationDraft ?? {
    enabled: true,
    interaction_mode: "ptt",
    hotkey: DEFAULT_HOTKEY,
    output_mode: "paste",
  };

  const [isEditingHotkey, setIsEditingHotkey] = useState(false);
  const [tempHotkey, setTempHotkey] = useState(dictation.hotkey || DEFAULT_HOTKEY);
  const [savedToast, setSavedToast] = useState(false);
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  // Tracked toast timer (style-guide §4.4 — previously two untracked setTimeouts).
  const toastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    return () => {
      if (toastTimerRef.current) clearTimeout(toastTimerRef.current);
    };
  }, []);
  const flashSavedToast = useCallback(() => {
    setHotkeyError(null);
    setSavedToast(true);
    if (toastTimerRef.current) clearTimeout(toastTimerRef.current);
    toastTimerRef.current = setTimeout(() => setSavedToast(false), 2200);
  }, []);

  const outputMode = dictation.output_mode || "paste";
  const isSmall = layoutMode === "small";

  const autoStopDraft = dictation.silence_auto_stop_ms;
  const isAutoStopEnabled = autoStopDraft !== undefined ? autoStopDraft > 0 : true;
  const autoStopMs = isAutoStopEnabled ? (autoStopDraft ?? 1200) : 1200;

  const [secondsDraft, setSecondsDraft] = useState<string>(
    isAutoStopEnabled ? (autoStopMs / 1000).toFixed(1) : "1.2"
  );
  const secondsInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isAutoStopEnabled && autoStopMs > 0) {
      setSecondsDraft((autoStopMs / 1000).toFixed(1));
    }
  }, [isAutoStopEnabled, autoStopMs]);

  // Draft-only: the store's 600ms debounced autosave persists. The old code also
  // fired updateSetting per interaction — a Tauri IPC round trip per keystroke on
  // a CPU-first box — and a rejected write left this optimistic local state
  // showing a value the backend never accepted.
  const handleAutoStopToggle = useCallback(() => {
    if (isAutoStopEnabled) {
      updateDraft("dictation", "silence_auto_stop_ms", 0);
    } else {
      const num = parseFloat(secondsDraft);
      const fallbackMs = !isNaN(num) && num > 0 ? Math.round(num * 1000) : 1200;
      updateDraft("dictation", "silence_auto_stop_ms", fallbackMs);
    }
  }, [isAutoStopEnabled, secondsDraft, updateDraft]);

  const handleSecondsChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      let val = e.target.value.replace(/[^0-9.]/g, "");
      const parts = val.split(".");
      if (parts.length > 2) {
        val = parts[0] + "." + parts.slice(1).join("");
      }
      setSecondsDraft(val);
      const num = parseFloat(val);
      if (!isNaN(num) && num > 0 && num <= 30) {
        const ms = Math.round(num * 1000);
        updateDraft("dictation", "silence_auto_stop_ms", ms);
      }
    },
    [updateDraft]
  );

  const handleSecondsBlur = useCallback(() => {
    const num = parseFloat(secondsDraft);
    // GUARD: Input should not be 0, negative, or NaN
    if (isNaN(num) || num <= 0) {
      setSecondsDraft("1.2");
      updateDraft("dictation", "silence_auto_stop_ms", 1200);
    } else if (num > 30) {
      setSecondsDraft("30.0");
      updateDraft("dictation", "silence_auto_stop_ms", 30000);
    } else {
      const cleanStr = num.toFixed(1);
      setSecondsDraft(cleanStr);
      const ms = Math.round(num * 1000);
      updateDraft("dictation", "silence_auto_stop_ms", ms);
    }
  }, [secondsDraft, updateDraft]);

  useEffect(() => {
    setTempHotkey(dictation.hotkey || DEFAULT_HOTKEY);
  }, [dictation.hotkey]);

  useEffect(() => {
    if (isEditingHotkey) {
      inputRef.current?.focus();
    }
  }, [isEditingHotkey]);

  const handleHotkeySave = useCallback(async () => {
    const cleanKey = tempHotkey.replace(/\.\.\.$/, "").trim();
    if (cleanKey && cleanKey !== "...") {
      updateDraft("dictation", "hotkey", cleanKey);
      try {
        await updateSetting("dictation", "hotkey", cleanKey);
        flashSavedToast();
      } catch (err) {
        // Visible, not console-only: the field would otherwise keep showing a
        // hotkey the backend rejected.
        setHotkeyError(err instanceof Error ? err.message : String(err));
      }
    }
    setIsEditingHotkey(false);
  }, [tempHotkey, updateDraft, flashSavedToast]);

  const handleHotkeyCancel = useCallback(() => {
    setTempHotkey(dictation.hotkey || DEFAULT_HOTKEY);
    setIsEditingHotkey(false);
  }, [dictation.hotkey]);

  const handleResetDefault = useCallback(async () => {
    setTempHotkey(DEFAULT_HOTKEY);
    updateDraft("dictation", "hotkey", DEFAULT_HOTKEY);
    try {
      await updateSetting("dictation", "hotkey", DEFAULT_HOTKEY);
      flashSavedToast();
    } catch (err) {
      setHotkeyError(err instanceof Error ? err.message : String(err));
    }
    setIsEditingHotkey(false);
  }, [updateDraft, flashSavedToast]);

  const handleKeyDownRecorder = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.key === "Escape") {
        handleHotkeyCancel();
        return;
      }
      if (e.key === "Enter") {
        handleHotkeySave();
        return;
      }

      const modifiers: string[] = [];
      if (e.ctrlKey) modifiers.push("Ctrl");
      if (e.altKey) modifiers.push("Alt");
      if (e.shiftKey) modifiers.push("Shift");
      if (e.metaKey) modifiers.push("Super");

      // Show live modifier combination while holding modifier keys
      if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) {
        setTempHotkey(modifiers.length > 0 ? `${modifiers.join("+")}+...` : "...");
        return;
      }

      let keyName = e.key;
      if (keyName === " ") keyName = "Space";
      else if (keyName.length === 1) keyName = keyName.toUpperCase();

      const combo = [...modifiers];
      if (!combo.includes(keyName)) {
        combo.push(keyName);
      }

      if (combo.length > 0) {
        setTempHotkey(combo.join("+"));
      }
    },
    [handleHotkeyCancel, handleHotkeySave]
  );

  const getOutputDescription = (mode: string) => {
    switch (mode) {
      case "paste":
        return DICTATION_COPY.destinationPasteDesc;
      case "clipboard":
        return DICTATION_COPY.destinationClipboardDesc;
      case "tray":
        return DICTATION_COPY.destinationTrayDesc;
      default:
        return "";
    }
  };

  const renderSilenceWatchdogSvg = (isEnabled: boolean, secondsStr: string) => {
    const secs = parseFloat(secondsStr) || 1.2;
    const clampedSecs = Math.max(0.5, Math.min(secs, 5.0));
    const arcDegrees = Math.round((clampedSecs / 5.0) * 270);
    const startAngle = -Math.PI / 2; // 12 o'clock
    const endAngle = startAngle + (arcDegrees * Math.PI) / 180;
    const r = 20;
    const cx = 26;
    const cy = 26;
    const x1 = cx + r * Math.cos(startAngle);
    const y1 = cy + r * Math.sin(startAngle);
    const x2 = cx + r * Math.cos(endAngle);
    const y2 = cy + r * Math.sin(endAngle);
    const largeArc = arcDegrees > 180 ? 1 : 0;
    const arcPath = `M ${x1.toFixed(2)} ${y1.toFixed(2)} A ${r} ${r} 0 ${largeArc} 1 ${x2.toFixed(2)} ${y2.toFixed(2)}`;

    return (
      <svg
        viewBox="0 0 52 52"
        className="w-full h-full overflow-visible select-none"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        {/* Ground shadow */}
        <ellipse
          cx="26"
          cy="49"
          rx="14"
          ry="2"
          fill="currentColor"
          className={cn("transition-opacity duration-300", isEnabled ? "opacity-20" : "opacity-05")}
        />

        {/* Outer dial ring */}
        <circle
          cx="26"
          cy="26"
          r={r}
          stroke="currentColor"
          strokeWidth="1.25"
          className={cn("transition-opacity duration-300", isEnabled ? "opacity-90" : "opacity-35")}
        />

        {/* 12 dial tick marks */}
        {Array.from({ length: 12 }).map((_, i) => {
          const angle = (i * 30 * Math.PI) / 180;
          const inner = i % 3 === 0 ? 15.5 : 17;
          const outer = 19;
          return (
            <line
              key={i}
              x1={(26 + inner * Math.sin(angle)).toFixed(2)}
              y1={(26 - inner * Math.cos(angle)).toFixed(2)}
              x2={(26 + outer * Math.sin(angle)).toFixed(2)}
              y2={(26 - outer * Math.cos(angle)).toFixed(2)}
              stroke="currentColor"
              strokeWidth={i % 3 === 0 ? "1.3" : "0.75"}
              strokeLinecap="round"
              className={cn("transition-opacity duration-300", isEnabled ? "opacity-75" : "opacity-25")}
            />
          );
        })}

        {/* Dynamic countdown sweep arc */}
        {isEnabled && arcDegrees > 0 && (
          <path
            d={arcPath}
            stroke="currentColor"
            strokeWidth="2.2"
            strokeLinecap="round"
            fill="none"
            className="transition-all duration-300"
          />
        )}

        {/* Acoustic Sound Decay Waveform (Left of Gate) */}
        {/* Loud speech bar */}
        <line
          x1="16"
          y1="19"
          x2="16"
          y2="33"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
          className={cn("transition-opacity duration-300", isEnabled ? "opacity-95" : "opacity-30")}
        />
        {/* Decaying speech bar */}
        <line
          x1="20"
          y1="22"
          x2="20"
          y2="30"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
          className={cn("transition-opacity duration-300", isEnabled ? "opacity-85" : "opacity-25")}
        />
        {/* Whisper bar */}
        <line
          x1="24"
          y1="24"
          x2="24"
          y2="28"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
          className={cn("transition-opacity duration-300", isEnabled ? "opacity-70" : "opacity-20")}
        />

        {/* Acoustic Silence Gate (Vertical threshold at x=26) */}
        <line
          x1="26"
          y1="16"
          x2="26"
          y2="36"
          stroke="currentColor"
          strokeWidth="0.8"
          strokeDasharray="2 2"
          className={cn("transition-opacity duration-300", isEnabled ? "opacity-45" : "opacity-15")}
        />

        {/* Serene Silence Flatline (Right of Gate) */}
        <line
          x1="27"
          y1="26"
          x2="33"
          y2="26"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
          className={cn("transition-opacity duration-300", isEnabled ? "opacity-90" : "opacity-30")}
        />

        {/* Silence Commit Node / Chronometer Pip */}
        <circle
          cx="36"
          cy="26"
          r="2.8"
          stroke="currentColor"
          strokeWidth="1.1"
          fill="none"
          className={cn("transition-opacity duration-300", isEnabled ? "opacity-90" : "opacity-30")}
        />
        <circle
          cx="36"
          cy="26"
          r="1.2"
          fill="currentColor"
          className={cn("transition-opacity duration-300", isEnabled ? "opacity-100" : "opacity-30")}
        />

        {/* Inactive Hush Diagonal Slash when Disabled */}
        {!isEnabled && (
          <line
            x1="16"
            y1="36"
            x2="36"
            y2="16"
            stroke="currentColor"
            strokeWidth="1.3"
            strokeLinecap="round"
            className="opacity-40"
          />
        )}
      </svg>
    );
  };

  return (
    <div
      className={cn(
        "w-full flex-1 flex flex-col justify-between select-none animate-fade-in transition-opacity duration-200",
        disabled && "opacity-40 pointer-events-none select-none"
      )}
    >
      {/* Layer 1: Subtab Navigation (Matching Personal & Working Memory) */}
      <div
        className="w-full flex items-center justify-between pt-0.5 pb-1.5 shrink-0 border-b border-[rgba(var(--accent),0.08)] mb-2 px-0.5 overflow-x-auto no-scrollbar"
        role="tablist"
        aria-label="Dictation configuration tabs"
      >
        {TABS.map((tab, idx, arr) => {
          const isActive = activeSubTab === tab.id;
          return (
            <div key={tab.id} className="flex-1 min-w-0 flex items-center justify-center">
              <button
                type="button"
                role="tab"
                aria-selected={isActive}
                onClick={() => setActiveSubTab(tab.id)}
                className={cn(
                  "w-full flex items-center justify-center pb-1 border-b-2 transition-all duration-200 bg-transparent text-[9.5px] sm:text-[10.5px] xl:text-[11px] font-black uppercase tracking-[0.04em] sm:tracking-[0.08em] outline-none cursor-pointer text-center truncate px-0.5",
                  isActive
                    ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]"
                )}
              >
                <span className="truncate">{tab.label}</span>
              </button>
              {idx < arr.length - 1 && (
                <span className="text-[10px] text-[rgb(var(--foreground-muted))]/25 font-light select-none pb-1 shrink-0 px-0.5 sm:px-1">
                  |
                </span>
              )}
            </div>
          );
        })}
      </div>

      {/* Layer 2: Subtab Workspace (Matching Personal & Working Memory) */}
      <div
        className={cn(
          "w-full flex flex-col flex-1 min-h-0 pt-0.5 pb-0.5 justify-center",
          isSmall ? "h-auto py-1" : "h-[120px] max-h-[120px]"
        )}
      >
        {/* TAB 1: ACTIVATION HOTKEY */}
        {activeSubTab === "hotkey" && (
          <div className="flex flex-row items-center justify-between gap-4 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            {/* Left: Title, Description & Status Line */}
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                  {DICTATION_COPY.hotkeyTitle}
                </span>
                <Tooltip label="System-wide shortcut to trigger voice typing. Hold while speaking, release to finalize. To rebind: click Edit, hold modifiers (Ctrl, Alt, Shift, Super), then press your key." side="top">
                  <span className="text-[rgb(var(--foreground-muted))]/50 hover:text-[rgb(var(--accent))] transition-colors cursor-help inline-flex items-center">
                    <Info size={11} />
                  </span>
                </Tooltip>
                {savedToast ? (
                  <span className="flex items-center gap-1 text-emerald-400 text-[10px] font-mono font-bold animate-fade-in">
                    <CheckCircle2 size={11} strokeWidth={2.5} />
                    <span>{DICTATION_COPY.savedFeedback}</span>
                  </span>
                ) : hotkeyError ? (
                  <span role="alert" className="text-[10px] font-mono font-bold text-red-400 max-w-[220px] truncate" title={hotkeyError}>
                    {hotkeyError}
                  </span>
                ) : !isEditingHotkey && (dictation.hotkey || DEFAULT_HOTKEY) !== DEFAULT_HOTKEY ? (
                  <button
                    type="button"
                    onClick={handleResetDefault}
                    className="text-[9.5px] text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] flex items-center gap-1 transition-colors cursor-pointer"
                    title={DICTATION_COPY.resetDefault}
                  >
                    <RotateCcw size={10} />
                    <span>{DICTATION_COPY.resetDefault}</span>
                  </button>
                ) : null}
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {DICTATION_COPY.hotkeySubtitle}
              </p>
              <div className="flex items-center gap-1.5 pt-0.5">
                <span className="flex items-center gap-1 text-[10px] sm:text-[10.5px] font-mono text-[rgb(var(--accent))] font-medium">
                  <Keyboard size={11} className="shrink-0 opacity-85" />
                  <span>Push-to-Talk active ({dictation.hotkey || DEFAULT_HOTKEY})</span>
                </span>
              </div>
            </div>

            {/* Right: Dynamic 3D SVG Keyboard Control + Edit / Tick / Cross */}
            <div className="shrink-0 flex flex-col items-center justify-center">
              {/* Hidden Keyboard Input */}
              <input
                ref={inputRef}
                type="text"
                value={tempHotkey}
                onKeyDown={handleKeyDownRecorder}
                readOnly
                placeholder={DICTATION_COPY.recordingPrompt}
                className="sr-only"
                tabIndex={isEditingHotkey ? 0 : -1}
                aria-label={DICTATION_COPY.hotkeyTitle}
              />

              <KeyboardHotkeySkeleton
                hotkey={isEditingHotkey ? tempHotkey : (dictation.hotkey || DEFAULT_HOTKEY)}
                isRecording={isEditingHotkey}
                onEdit={() => {
                  if (!isEditingHotkey) {
                    setTempHotkey(dictation.hotkey || DEFAULT_HOTKEY);
                    setIsEditingHotkey(true);
                  }
                  inputRef.current?.focus();
                }}
                bottomSlot={
                  <div className="mt-1 flex items-center justify-center min-h-[22px]">
                    {isEditingHotkey ? (
                      <div className="flex items-center gap-2 animate-fade-in">
                        <span className="text-[10px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))] select-none">
                          {tempHotkey && tempHotkey !== "..." ? tempHotkey : DICTATION_COPY.recordingPrompt}
                        </span>
                        <div className="flex items-center gap-1">
                          <Tooltip label={DICTATION_COPY.saveBtn} side="bottom">
                            <button
                              type="button"
                              onClick={handleHotkeySave}
                              className="p-1 text-emerald-400 hover:text-emerald-300 hover:scale-110 transition-all cursor-pointer rounded"
                              aria-label={DICTATION_COPY.saveBtn}
                            >
                              <Check size={12} strokeWidth={2.5} />
                            </button>
                          </Tooltip>
                          <Tooltip label={DICTATION_COPY.cancelBtn} side="bottom">
                            <button
                              type="button"
                              onClick={handleHotkeyCancel}
                              className="p-1 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:scale-110 transition-all cursor-pointer rounded"
                              aria-label={DICTATION_COPY.cancelBtn}
                            >
                              <X size={12} strokeWidth={2} />
                            </button>
                          </Tooltip>
                        </div>
                      </div>
                    ) : (
                      <button
                        type="button"
                        onClick={() => {
                          setTempHotkey(dictation.hotkey || DEFAULT_HOTKEY);
                          setIsEditingHotkey(true);
                          inputRef.current?.focus();
                        }}
                        className="group/hk flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-[rgba(var(--foreground),0.03)] hover:bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.12)] hover:border-[rgba(var(--accent),0.35)] transition-all duration-200 cursor-pointer shadow-sm"
                        title={DICTATION_COPY.rebindHint}
                        aria-label={`${DICTATION_COPY.hotkeyTitle}: ${dictation.hotkey || DEFAULT_HOTKEY}. Click to edit.`}
                      >
                        <span className="text-[10px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground))] group-hover/hk:text-[rgb(var(--accent))] transition-colors">
                          {(dictation.hotkey || DEFAULT_HOTKEY).split("+").map((s) => s.trim()).join(" + ")}
                        </span>
                        <Pencil size={10} className="text-[rgb(var(--foreground-muted))]/50 group-hover/hk:text-[rgb(var(--accent))] transition-colors shrink-0" />
                      </button>
                    )}
                  </div>
                }
              />
            </div>
          </div>
        )}

        {/* TAB 2: OUTPUT MODE */}
        {activeSubTab === "destination" && (
          <div className="flex flex-row items-center justify-between gap-4 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            {/* Left: Heading, dynamic description & delivery status */}
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                  {DICTATION_COPY.destinationTitle}
                </span>
                <Tooltip label="Paste: simulates direct keystrokes into active focus. Clipboard: silent buffer copy without focus grab. Tray: real-time floating transcript HUD." side="top">
                  <span className="text-[rgb(var(--foreground-muted))]/50 hover:text-[rgb(var(--accent))] transition-colors cursor-help inline-flex items-center">
                    <Info size={11} />
                  </span>
                </Tooltip>
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {getOutputDescription(outputMode)}
              </p>
              <div className="flex items-center gap-1.5 pt-0.5">
                <span className="flex items-center gap-1 text-[10px] sm:text-[10.5px] font-mono text-[rgb(var(--accent))] font-medium">
                  {outputMode === "paste" && <Send size={11} className="shrink-0 opacity-85" />}
                  {outputMode === "clipboard" && <Clipboard size={11} className="shrink-0 opacity-85" />}
                  {outputMode === "tray" && <Layers size={11} className="shrink-0 opacity-85" />}
                  <span className="capitalize">{outputMode} delivery active</span>
                </span>
              </div>
            </div>

            {/* Right: Interactive Triangular Loop SVG Selector */}
            <div className="shrink-0 flex items-center justify-center">
              <TriangularLoopSelector
                options={OUTPUT_OPTIONS}
                value={outputMode}
                onChange={(mode) => updateDraft("dictation", "output_mode", mode)}
                showActiveLabel={false}
              />
            </div>
          </div>
        )}

        {/* TAB 3: AUTO SILENCE */}
        {activeSubTab === "autostop" && (
          <div className="flex flex-row items-center justify-between gap-4 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            {/* Left: Title, Description, and Status */}
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                  Silence Auto-Stop
                </span>
                <Tooltip label="Acoustic silence watchdog. Automatically commits dictation after quiet pauses. Click dial to toggle ON/OFF, or click time badge to type any duration in seconds (>0s)." side="top">
                  <span className="text-[rgb(var(--foreground-muted))]/50 hover:text-[rgb(var(--accent))] transition-colors cursor-help inline-flex items-center">
                    <Info size={11} />
                  </span>
                </Tooltip>
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {isAutoStopEnabled
                  ? `Automatically commits dictation after ${secondsDraft} seconds of silence.`
                  : "Watchdog disabled. Dictation continues until hotkey release or manual stop."}
              </p>
              <div className="flex items-center gap-1.5 pt-0.5">
                {isAutoStopEnabled ? (
                  <span className="flex items-center gap-1 text-[10px] sm:text-[10.5px] font-mono text-[rgb(var(--accent))] font-medium">
                    <Timer size={11} className="shrink-0 opacity-85" />
                    <span>Auto-commit active ({secondsDraft}s)</span>
                  </span>
                ) : (
                  <span className="flex items-center gap-1 text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50">
                    <VolumeX size={11} className="shrink-0 opacity-60" />
                    <span>Manual release only</span>
                  </span>
                )}
              </div>
            </div>

            {/* Right: Acoustic Chronometer SVG schedule control (matching PersonalMemory Clock pattern) */}
            <div className="shrink-0 flex flex-col items-center justify-center">
              <button
                type="button"
                role="switch"
                aria-checked={isAutoStopEnabled}
                aria-label={isAutoStopEnabled ? "Disable silence auto-stop" : "Enable silence auto-stop"}
                onClick={handleAutoStopToggle}
                className={cn(
                  "group flex flex-col items-center select-none cursor-pointer rounded-2xl px-3 py-1.5 transition-all duration-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))]",
                  isAutoStopEnabled
                    ? "text-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--foreground-muted))]/70"
                )}
                title={isAutoStopEnabled ? "Click to disable auto-stop" : "Click to enable auto-stop"}
              >
                {/* 48x48 minimal line-style acoustic chronometer SVG */}
                <div
                  className="relative w-[48px] h-[48px] transition-transform duration-300 group-hover:scale-105"
                  style={{
                    animation: isAutoStopEnabled ? "wm-globe-float 3.6s ease-in-out infinite" : "none",
                  }}
                >
                  {renderSilenceWatchdogSvg(isAutoStopEnabled, secondsDraft)}
                </div>

                {/* Badge: OFF label when disabled */}
                {!isAutoStopEnabled && (
                  <span className="mt-2.5 px-2.5 py-0.5 rounded-full text-[9px] font-mono font-bold uppercase tracking-[0.14em] leading-none transition-all duration-300 text-[rgb(var(--foreground-muted))]/50">
                    OFF
                  </span>
                )}
              </button>

              {/* Time input badge — only when enabled, outside toggle button */}
              {isAutoStopEnabled && (
                <button
                  type="button"
                  onClick={() => secondsInputRef.current?.focus()}
                  className="mt-0 flex items-center gap-0.5 px-2 py-0.5 rounded-full cursor-text transition-all duration-200 hover:bg-[rgba(var(--accent),0.08)] group/time"
                  tabIndex={-1}
                  aria-label="Silence auto-stop duration in seconds"
                >
                  <input
                    ref={secondsInputRef}
                    type="text"
                    inputMode="decimal"
                    value={secondsDraft}
                    onChange={handleSecondsChange}
                    onBlur={handleSecondsBlur}
                    placeholder="1.2"
                    maxLength={4}
                    aria-label="Silence auto-stop duration in seconds"
                    className="w-[2.4rem] bg-transparent font-mono text-[11px] font-bold outline-none text-[rgb(var(--accent))] text-center tracking-wider caret-[rgb(var(--accent))] selection:bg-[rgba(var(--accent),0.25)] cursor-text"
                  />
                  <span className="text-[9px] font-mono font-bold uppercase tracking-[0.12em] text-[rgb(var(--accent))]/70">
                    s
                  </span>
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
});

DictationConfigDesk.displayName = "DictationConfigDesk";
