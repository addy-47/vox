import { memo, useState, useEffect, useCallback } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Clipboard, Layers, Send, Check, X } from "lucide-react";
import { Tooltip } from "@/shared/ui/Tooltip";
import { cn } from "@/shared/lib/utils";
import { DICTATION_COPY } from "@/data/settingsCopy";

interface DictationConfigDeskProps {
  layoutMode?: "full-max" | "full-min" | "small";
  disabled?: boolean;
}

const OUTPUT_OPTIONS = [
  { id: "paste" as const, label: DICTATION_COPY.modePaste },
  { id: "clipboard" as const, label: DICTATION_COPY.modeClipboard },
  { id: "tray" as const, label: DICTATION_COPY.modeTray },
];

export const DictationConfigDesk = memo(({ layoutMode, disabled = false }: DictationConfigDeskProps) => {
  const dictationDraft = useSettingsStore((s) => s.draftSettings?.dictation);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const dictation = dictationDraft ?? {
    enabled: true,
    interaction_mode: "ptt",
    hotkey: "Alt+Space",
    output_mode: "paste",
  };

  const [isEditingHotkey, setIsEditingHotkey] = useState(false);
  const [tempHotkey, setTempHotkey] = useState(dictation.hotkey || "Alt+Space");

  const outputMode = dictation.output_mode || "paste";

  useEffect(() => {
    setTempHotkey(dictation.hotkey || "Alt+Space");
  }, [dictation.hotkey]);

  const handleHotkeySave = useCallback(() => {
    if (tempHotkey.trim()) {
      updateDraft("dictation", "hotkey", tempHotkey.trim());
    }
    setIsEditingHotkey(false);
  }, [tempHotkey, updateDraft]);

  const handleHotkeyCancel = useCallback(() => {
    setTempHotkey(dictation.hotkey || "Alt+Space");
    setIsEditingHotkey(false);
  }, [dictation.hotkey]);

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

      const keys: string[] = [];
      if (e.ctrlKey) keys.push("Ctrl");
      if (e.altKey) keys.push("Alt");
      if (e.shiftKey) keys.push("Shift");
      if (e.metaKey) keys.push("Super");

      // Ignore bare modifier presses
      if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) {
        return;
      }

      let keyName = e.key;
      if (keyName === " ") keyName = "Space";
      else if (keyName.length === 1) keyName = keyName.toUpperCase();

      if (!keys.includes(keyName)) {
        keys.push(keyName);
      }

      if (keys.length > 0) {
        setTempHotkey(keys.join("+"));
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

  return (
    <div
      className={cn(
        "flex flex-col gap-1.5 sm:gap-2 w-full mt-1.5 animate-fade-in transition-opacity duration-200",
        disabled && "opacity-40 pointer-events-none select-none"
      )}
    >
      {/* Output Destination Selector Ribbon with arrow */}
      <div className="flex flex-wrap items-center justify-between gap-y-1.5 gap-x-1 w-full pb-1.5 sm:pb-2 pt-1 shrink-0 px-1.5 sm:px-3">
        {/* Left: Mode Title */}
        <div className="flex items-center gap-1 sm:gap-1.5 shrink-0 pr-0.5 sm:pr-1">
          <div className="p-0.5 sm:p-1 rounded-md text-[rgb(var(--accent))] flex items-center justify-center">
            <OutputIcon size={13} className="shrink-0 sm:w-3.5 sm:h-3.5" />
          </div>
          <span className="text-[12px] sm:text-[13px] font-black tracking-wider uppercase text-[rgb(var(--accent))] select-none">
            Output
          </span>
        </div>

        {/* Center Connector Arrow */}
        <div className="flex flex-1 items-center px-1 min-w-[8px] pointer-events-none select-none overflow-hidden">
          <svg
            className="w-full h-2.5 sm:h-3 text-[rgb(var(--accent))]/50 overflow-visible"
            viewBox="0 0 100 12"
            preserveAspectRatio="none"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <line
              x1="0"
              y1="6"
              x2="97"
              y2="6"
              stroke="currentColor"
              strokeWidth="1.25"
              strokeLinecap="round"
              vectorEffect="non-scaling-stroke"
            />
            <path
              d="M 92 2.5 L 98.5 6 L 92 9.5"
              stroke="currentColor"
              strokeWidth="1.25"
              strokeLinecap="round"
              strokeLinejoin="round"
              vectorEffect="non-scaling-stroke"
            />
          </svg>
        </div>

        {/* Right: Output Mode Underline Tabs (Short Titles) */}
        <div className="flex items-center gap-1.5 sm:gap-2.5 shrink-0 pl-0.5 sm:pl-1">
          {OUTPUT_OPTIONS.map((mode, idx, arr) => {
            const isActive = outputMode === mode.id;
            return (
              <div key={mode.id} className="flex items-center gap-1.5 sm:gap-2.5">
                <button
                  type="button"
                  onClick={() => updateDraft("dictation", "output_mode", mode.id)}
                  className={cn(
                    "flex items-center justify-center gap-1 pb-0.5 sm:pb-1 border-b-2 transition-all duration-200 bg-transparent text-[11px] sm:text-[12px] font-black uppercase tracking-[0.08em] sm:tracking-[0.12em] outline-none cursor-pointer",
                    isActive
                      ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                      : "text-[rgb(var(--foreground-muted))]/50 border-transparent hover:text-[rgb(var(--foreground-muted))]/80"
                  )}
                >
                  <span>{mode.label}</span>
                </button>
                {idx < arr.length - 1 && (
                  <span className="text-[11px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/20 font-light select-none pb-0.5 sm:pb-1">
                    |
                  </span>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* Desk Content Area */}
      <div
        className={cn(
          "w-full flex items-center justify-between rounded-xl p-3 relative border border-[rgba(var(--accent),0.06)] animate-fade-in",
          layoutMode === "small"
            ? "h-auto min-h-0 flex-col gap-3.5 items-stretch"
            : "h-[120px] min-h-[120px] max-h-[120px]"
        )}
      >
        {/* Main Content Area: Left icon + Description & Hotkey */}
        <div className="flex items-center justify-between h-full gap-3 sm:gap-4 flex-1 w-full px-1 sm:px-2">
          {/* Left Icon with ambient circle aura */}
          <div className="flex items-center justify-center relative min-w-[48px] sm:min-w-[70px] h-full shrink-0">
            <div className="absolute w-14 h-14 rounded-full border border-[rgb(var(--accent))]/5 animate-ring-pulse-slow" />
            <div className="w-8 h-8 sm:w-9 sm:h-9 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/40 flex items-center justify-center relative z-10">
              <OutputIcon className="text-[rgb(var(--accent))]" size={16} />
            </div>
          </div>

          {/* Center Info: Heading + Description */}
          <div className="flex flex-col justify-center gap-1.5 flex-1 min-w-0 h-full">
            <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1">
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80 truncate">
                {getOutputHeading(outputMode)}
              </span>
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/60 leading-relaxed font-semibold line-clamp-2">
              {getOutputDescription(outputMode)}
            </p>
          </div>

          {/* Right: Hotkey Rebind Controller (Inline on standard layout, cleanly positioned) */}
          <div className={cn(
            "flex items-center shrink-0 pl-3 sm:pl-4 border-l border-[rgba(var(--accent),0.08)]",
            layoutMode === "small" ? "self-end pt-1" : "h-full"
          )}>
            <div className="flex flex-col items-end justify-center gap-1">
              <span className="text-[11px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]/60 select-none">
                {DICTATION_COPY.hotkeyTitle}
              </span>
              {isEditingHotkey ? (
                <div className="flex items-center gap-1.5 animate-fade-in">
                  <input
                    type="text"
                    value={tempHotkey}
                    onKeyDown={handleKeyDownRecorder}
                    readOnly
                    placeholder={DICTATION_COPY.recordingPrompt}
                    className="w-24 sm:w-28 text-center text-[11px] sm:text-[12px] font-mono font-bold px-1.5 sm:px-2 py-0.5 rounded-md bg-[rgba(var(--background),0.9)] border border-[rgb(var(--accent))] text-[rgb(var(--accent))] focus:outline-none cursor-pointer"
                    autoFocus
                  />
                  <Tooltip label={DICTATION_COPY.saveLabel}>
                    <button
                      type="button"
                      onClick={handleHotkeySave}
                      className="p-1 rounded-md bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:opacity-90 active:scale-95 transition-all cursor-pointer flex items-center justify-center"
                    >
                      <Check size={12} strokeWidth={2.5} />
                    </button>
                  </Tooltip>
                  <Tooltip label={DICTATION_COPY.cancelLabel}>
                    <button
                      type="button"
                      onClick={handleHotkeyCancel}
                      className="p-1 rounded-md bg-[rgba(var(--foreground),0.06)] border border-[rgba(var(--border),0.15)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] active:scale-95 transition-all cursor-pointer flex items-center justify-center"
                    >
                      <X size={12} strokeWidth={2} />
                    </button>
                  </Tooltip>
                </div>
              ) : (
                <Tooltip label="Click to rebind activation shortcut">
                  <button
                    type="button"
                    onClick={() => {
                      setTempHotkey(dictation.hotkey || "Alt+Space");
                      setIsEditingHotkey(true);
                    }}
                    className="flex items-center gap-1 sm:gap-1.5 px-2 sm:px-2.5 py-1 rounded-lg bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.2)] hover:border-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.14)] text-[rgb(var(--foreground))] transition-all group cursor-pointer active:scale-95 shadow-sm"
                  >
                    <kbd className="text-[11px] font-mono font-bold tracking-wide text-[rgb(var(--accent))]">
                      {dictation.hotkey || "Alt+Space"}
                    </kbd>
                    <span className="text-[11px] uppercase font-mono font-bold text-[rgb(var(--accent))]/75 group-hover:text-[rgb(var(--accent))] ml-0.5">
                      {DICTATION_COPY.editLabel}
                    </span>
                  </button>
                </Tooltip>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
});

DictationConfigDesk.displayName = "DictationConfigDesk";
