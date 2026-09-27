import { memo, useState, useEffect, useCallback, useRef } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Clipboard, Layers, Send, Check, X, CheckCircle2, RotateCcw, Keyboard } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { DICTATION_COPY } from "@/data/settingsCopy";
import { updateSetting } from "@/services/settingsService";

interface DictationConfigDeskProps {
  layoutMode?: "full-max" | "full-min" | "small";
  disabled?: boolean;
}

const DEFAULT_HOTKEY = DICTATION_COPY.defaultHotkey; // "Alt+V"

const OUTPUT_OPTIONS = [
  { id: "paste" as const, label: DICTATION_COPY.modePaste },
  { id: "clipboard" as const, label: DICTATION_COPY.modeClipboard },
  { id: "tray" as const, label: DICTATION_COPY.modeTray },
];

const OUTPUT_IDS = OUTPUT_OPTIONS.map((o) => o.id);

export const DictationConfigDesk = memo(({ layoutMode, disabled = false }: DictationConfigDeskProps) => {
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
  const inputRef = useRef<HTMLInputElement>(null);

  const outputMode = dictation.output_mode || "paste";

  const handleOutputKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const currentIdx = OUTPUT_IDS.indexOf(outputMode);
      if (e.key === "ArrowRight") {
        e.preventDefault();
        const nextIdx = (currentIdx + 1) % OUTPUT_IDS.length;
        updateDraft("dictation", "output_mode", OUTPUT_IDS[nextIdx]);
      } else if (e.key === "ArrowLeft") {
        e.preventDefault();
        const prevIdx = (currentIdx - 1 + OUTPUT_IDS.length) % OUTPUT_IDS.length;
        updateDraft("dictation", "output_mode", OUTPUT_IDS[prevIdx]);
      }
    },
    [outputMode, updateDraft]
  );

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
        setSavedToast(true);
        setTimeout(() => setSavedToast(false), 2200);
      } catch (err) {
        console.error("Failed to persist dictation hotkey via IPC:", err);
      }
    }
    setIsEditingHotkey(false);
  }, [tempHotkey, updateDraft]);

  const handleHotkeyCancel = useCallback(() => {
    setTempHotkey(dictation.hotkey || DEFAULT_HOTKEY);
    setIsEditingHotkey(false);
  }, [dictation.hotkey]);

  const handleResetDefault = useCallback(async () => {
    setTempHotkey(DEFAULT_HOTKEY);
    updateDraft("dictation", "hotkey", DEFAULT_HOTKEY);
    try {
      await updateSetting("dictation", "hotkey", DEFAULT_HOTKEY);
      setSavedToast(true);
      setTimeout(() => setSavedToast(false), 2200);
    } catch (err) {
      console.error("Failed to reset dictation hotkey via IPC:", err);
    }
    setIsEditingHotkey(false);
  }, [updateDraft]);

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

  const getOutputIcon = (mode: string) => {
    switch (mode) {
      case "paste":
        return Send;
      case "clipboard":
        return Clipboard;
      case "tray":
        return Layers;
      default:
        return Send;
    }
  };

  const getOutputHeading = (mode: string) => {
    switch (mode) {
      case "paste":
        return DICTATION_COPY.modePasteLong;
      case "clipboard":
        return DICTATION_COPY.modeClipboardLong;
      case "tray":
        return DICTATION_COPY.modeTrayLong;
      default:
        return DICTATION_COPY.modePasteLong;
    }
  };

  const OutputIcon = getOutputIcon(outputMode);

  const renderKeyBadges = (hotkeyStr: string, isRecording: boolean) => {
    const clean = hotkeyStr.replace(/\.\.\.$/, "").trim();
    const parts = clean ? clean.split("+") : [];
    const hasTrailingDots = hotkeyStr.endsWith("...");

    if (parts.length === 0 && isRecording) {
      return (
        <span className="text-[11px] font-mono text-[rgb(var(--accent))] animate-pulse">
          {DICTATION_COPY.recordingPrompt}
        </span>
      );
    }

    return (
      <div className="flex items-center gap-1">
        {parts.map((k, i) => (
          <span key={i} className="flex items-center gap-1">
            <kbd
              className={cn(
                "px-1.5 py-0.5 rounded text-[11px] font-mono font-bold tracking-wide shadow-xs border",
                isRecording
                  ? "bg-[rgba(var(--accent),0.15)] border-[rgb(var(--accent))]/40 text-[rgb(var(--accent))]"
                  : "bg-[rgba(var(--foreground),0.06)] border-[rgba(var(--foreground),0.12)] text-[rgb(var(--foreground))]"
              )}
            >
              {k}
            </kbd>
            {i < parts.length - 1 && (
              <span className="text-[10px] text-[rgb(var(--foreground-muted))]/50 font-bold select-none">+</span>
            )}
          </span>
        ))}
        {hasTrailingDots && (
          <span className="flex items-center gap-1">
            <span className="text-[10px] text-[rgb(var(--foreground-muted))]/50 font-bold select-none">+</span>
            <kbd className="px-1.5 py-0.5 rounded text-[11px] font-mono font-bold bg-[rgba(var(--accent),0.2)] border border-[rgb(var(--accent))] text-[rgb(var(--accent))] animate-pulse">
              ...
            </kbd>
          </span>
        )}
      </div>
    );
  };

  return (
    <div
      className={cn(
        "flex flex-col gap-1.5 sm:gap-2 w-full mt-1.5 animate-fade-in transition-opacity duration-200",
        disabled && "opacity-40 pointer-events-none select-none"
      )}
    >
      {/* Output Destination Underline Tabs */}
      <div
        className="w-full flex items-center justify-between pt-0.5 pb-1 shrink-0 border-b border-[rgba(var(--accent),0.08)] mb-1 px-0.5 select-none overflow-x-auto no-scrollbar"
        role="tablist"
        aria-label="Dictation output mode"
      >
        {OUTPUT_OPTIONS.map((mode, idx, arr) => {
          const isActive = outputMode === mode.id;
          const ModeIcon = getOutputIcon(mode.id);
          return (
            <div key={mode.id} className="flex-1 min-w-0 flex items-center justify-center">
              <button
                type="button"
                role="tab"
                aria-selected={isActive}
                tabIndex={isActive ? 0 : -1}
                data-arrow-nav
                onKeyDown={handleOutputKeyDown}
                onClick={() => updateDraft("dictation", "output_mode", mode.id)}
                className={cn(
                  "w-full flex items-center justify-center gap-1.5 pb-1 border-b-2 transition-all duration-200 bg-transparent text-[10px] sm:text-[11px] font-black uppercase tracking-[0.06em] sm:tracking-[0.1em] outline-none cursor-pointer text-center truncate px-1",
                  isActive
                    ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]"
                )}
              >
                <ModeIcon
                  size={12}
                  className={cn("shrink-0", isActive ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/50")}
                />
                <span className="truncate">{mode.label}</span>
              </button>
              {idx < arr.length - 1 && (
                <span className="text-[10px] text-[rgb(var(--foreground-muted))]/25 font-light select-none pb-1 shrink-0 px-1">
                  |
                </span>
              )}
            </div>
          );
        })}
      </div>

      {/* Desk Content Area: Clean 2-column balanced workspace */}
      <div
        className={cn(
          "w-full rounded-xl p-3 sm:p-3.5 relative border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] animate-fade-in",
          layoutMode === "small"
            ? "flex flex-col gap-3.5"
            : "grid grid-cols-2 gap-3.5 h-[122px]"
        )}
      >
        {/* Left Column: Output Destination Overview */}
        <div className="flex flex-col justify-between h-full min-w-0 pr-1">
          <div className="flex items-center gap-2">
            <div className="w-6 h-6 rounded-md bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center shrink-0">
              <OutputIcon className="text-[rgb(var(--accent))]" size={13} />
            </div>
            <span className="text-[11.5px] sm:text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] truncate">
              {getOutputHeading(outputMode)}
            </span>
          </div>
          <p className="text-[11px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium line-clamp-3">
            {getOutputDescription(outputMode)}
          </p>
        </div>

        {/* Right Column: Global Activation Shortcut */}
        <div className={cn(
          "flex flex-col justify-between h-full min-w-0",
          layoutMode !== "small" && "pl-3.5 border-l border-[rgba(var(--accent),0.08)]"
        )}>
          {/* Section Header */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-1.5">
              <Keyboard size={12} className="text-[rgb(var(--foreground-muted))]/60" />
              <span className="text-[10px] sm:text-[10.5px] font-black uppercase tracking-[0.08em] text-[rgb(var(--foreground-muted))]/70">
                {DICTATION_COPY.hotkeyTitle}
              </span>
            </div>
            {savedToast ? (
              <div className="flex items-center gap-1 text-emerald-400 text-[10px] font-bold animate-fade-in">
                <CheckCircle2 size={11} strokeWidth={2.5} />
                <span>{DICTATION_COPY.savedFeedback}</span>
              </div>
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

          {/* Shortcut Box / Interactive Recorder */}
          {isEditingHotkey ? (
            <div className="flex flex-col gap-1.5 animate-fade-in">
              {/* Active Key Capture Box */}
              <div
                className="w-full flex items-center justify-center gap-1 py-1.5 px-2 rounded-lg bg-[rgba(var(--accent),0.06)] border border-[rgb(var(--accent))] shadow-inner relative cursor-pointer"
                onClick={() => inputRef.current?.focus()}
              >
                <input
                  ref={inputRef}
                  type="text"
                  value={tempHotkey}
                  onKeyDown={handleKeyDownRecorder}
                  readOnly
                  placeholder={DICTATION_COPY.recordingPrompt}
                  className="absolute inset-0 opacity-0 cursor-pointer w-full h-full"
                  autoFocus
                />
                {renderKeyBadges(tempHotkey, true)}
              </div>

              {/* Action Buttons: Save & Cancel - 100% visible and prominent */}
              <div className="flex items-center gap-1.5 w-full">
                <button
                  type="button"
                  onClick={handleHotkeySave}
                  className="flex-1 py-1 px-2 rounded-md bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:opacity-90 active:scale-95 transition-all cursor-pointer flex items-center justify-center gap-1 text-[11px] font-bold shadow-xs"
                >
                  <Check size={12} strokeWidth={2.5} />
                  <span>{DICTATION_COPY.saveBtn}</span>
                </button>
                <button
                  type="button"
                  onClick={handleHotkeyCancel}
                  className="py-1 px-2.5 rounded-md bg-[rgba(var(--foreground),0.06)] border border-[rgba(var(--foreground),0.12)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] active:scale-95 transition-all cursor-pointer flex items-center justify-center gap-1 text-[11px] font-medium"
                >
                  <X size={12} strokeWidth={2} />
                  <span>{DICTATION_COPY.cancelBtn}</span>
                </button>
              </div>
            </div>
          ) : (
            <div className="flex flex-col gap-1">
              <button
                type="button"
                onClick={() => {
                  setTempHotkey(dictation.hotkey || DEFAULT_HOTKEY);
                  setIsEditingHotkey(true);
                }}
                className="w-full flex items-center justify-between py-1.5 px-2.5 rounded-lg bg-[rgba(var(--accent),0.05)] border border-[rgba(var(--accent),0.2)] hover:border-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.09)] transition-all cursor-pointer group active:scale-[0.98] shadow-xs"
              >
                <div className="flex items-center gap-1">
                  {renderKeyBadges(dictation.hotkey || DEFAULT_HOTKEY, false)}
                </div>
                <span className="text-[10px] font-bold uppercase tracking-wider text-[rgb(var(--accent))]/75 group-hover:text-[rgb(var(--accent))]">
                  {DICTATION_COPY.editLabel}
                </span>
              </button>
              <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]/60 font-medium truncate">
                {DICTATION_COPY.rebindHint}
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
});

DictationConfigDesk.displayName = "DictationConfigDesk";
