import React, { memo, useState, useRef, useEffect, useCallback } from "react";
import { CornerDownLeft, X, Volume2, VolumeX, Mic, MicOff } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { HOME_CONTROLS_COPY } from "@/data/homeCopy";

export interface TextInputBarProps {
  onSubmit: (text: string) => void;
  onClose: () => void;
  isPlaybackMuted: boolean;
  onTogglePlaybackMute: () => void;
  isMicMuted: boolean;
  onToggleMicMute: () => void;
  disabled?: boolean;
  initialHistory?: string[];
}

export const TextInputBar = memo(({
  onSubmit,
  onClose,
  isPlaybackMuted,
  onTogglePlaybackMute,
  isMicMuted,
  onToggleMicMute,
  disabled = false,
  initialHistory,
}: TextInputBarProps) => {
  const [value, setValue] = useState("");
  const [history, setHistory] = useState<string[]>(() => (initialHistory ? [...initialHistory] : []));
  const [historyIndex, setHistoryIndex] = useState<number>(-1);
  const draftRef = useRef<string>("");
  const inputRef = useRef<HTMLInputElement>(null);
  const copy = HOME_CONTROLS_COPY.textMode;

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Sync any newly arriving turns into the history stack
  useEffect(() => {
    if (initialHistory && initialHistory.length > 0) {
      setHistory((prev) => {
        const merged = [...initialHistory];
        for (const item of prev) {
          if (!merged.includes(item)) {
            merged.push(item);
          }
        }
        return merged;
      });
    }
  }, [initialHistory]);

  const setCaretToEnd = useCallback(() => {
    requestAnimationFrame(() => {
      if (inputRef.current) {
        const len = inputRef.current.value.length;
        inputRef.current.setSelectionRange(len, len);
      }
    });
  }, []);

  const handleSubmit = useCallback((e?: React.FormEvent) => {
    if (e) e.preventDefault();
    const trimmed = value.trim();
    if (!trimmed || disabled) return;
    setHistory((prev) => (prev.length > 0 && prev[prev.length - 1] === trimmed ? prev : [...prev, trimmed]));
    setHistoryIndex(-1);
    draftRef.current = "";
    onSubmit(trimmed);
    setValue("");
  }, [value, disabled, onSubmit]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.key === "ArrowUp") {
      if (history.length === 0) return;
      e.preventDefault();
      if (historyIndex === -1) {
        draftRef.current = value;
        const newIndex = history.length - 1;
        setHistoryIndex(newIndex);
        setValue(history[newIndex]);
        setCaretToEnd();
      } else if (historyIndex > 0) {
        const newIndex = historyIndex - 1;
        setHistoryIndex(newIndex);
        setValue(history[newIndex]);
        setCaretToEnd();
      }
    } else if (e.key === "ArrowDown") {
      if (historyIndex === -1) return;
      e.preventDefault();
      if (historyIndex < history.length - 1) {
        const newIndex = historyIndex + 1;
        setHistoryIndex(newIndex);
        setValue(history[newIndex]);
        setCaretToEnd();
      } else {
        setHistoryIndex(-1);
        setValue(draftRef.current);
        setCaretToEnd();
      }
    }
  }, [handleSubmit, onClose, history, historyIndex, value, setCaretToEnd]);

  return (
    <div className="flex items-center w-full max-w-xl mx-auto gap-3 px-4 py-2  transition-all duration-300">
      {/* Mic Mute Toggle */}
      <button
        type="button"
        onClick={onToggleMicMute}
        className={cn(
          "flex items-center justify-center w-9 h-9 rounded-full transition-colors duration-200 cursor-pointer",
          isMicMuted
            ? "text-red-400/90 bg-red-500/15 hover:bg-red-500/25"
            : "text-[rgb(var(--accent))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10"
        )}
        aria-label={isMicMuted ? copy.unmuteMicAriaLabel : copy.muteMicAriaLabel}
        title={isMicMuted ? copy.unmuteMicTooltip : copy.muteMicTooltip}
      >
        {isMicMuted ? <MicOff size={17} /> : <Mic size={17} />}
      </button>

      {/* Speaker Playback Mute Toggle */}
      <button
        type="button"
        onClick={onTogglePlaybackMute}
        className={cn(
          "flex items-center justify-center w-9 h-9 rounded-full transition-colors duration-200 cursor-pointer",
          isPlaybackMuted
            ? "text-amber-400/90 bg-amber-500/15 hover:bg-amber-500/25"
            : "text-[rgb(var(--accent))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10"
        )}
        aria-label={isPlaybackMuted ? copy.unmuteSpeakerAriaLabel : copy.muteSpeakerAriaLabel}
        title={isPlaybackMuted ? copy.unmuteSpeakerTooltip : copy.muteSpeakerTooltip}
      >
        {isPlaybackMuted ? <VolumeX size={17} /> : <Volume2 size={17} />}
      </button>

      {/* Underline Input Box */}
      <form onSubmit={handleSubmit} className="flex-1 flex items-center min-w-0">
        <input
          ref={inputRef}
          type="text"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={copy.placeholder}
          disabled={disabled}
          className="w-full bg-transparent border-b border-[rgb(var(--accent))]/30 focus:border-[rgb(var(--accent))] px-2 py-1 text-sm text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground))]/40 outline-none transition-colors duration-200"
        />
      </form>

      {/* Send Button */}
      <button
        type="button"
        onClick={() => handleSubmit()}
        disabled={!value.trim() || disabled}
        className={cn(
          "flex items-center justify-center w-9 h-9 rounded-full transition-all duration-200 cursor-pointer border border-[rgb(var(--accent))]/40",
          value.trim() && !disabled
            ? "bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/30 hover:scale-105 active:scale-95"
            : "opacity-30 cursor-not-allowed text-[rgb(var(--accent))]/40 border-transparent"
        )}
        aria-label={copy.sendAriaLabel}
        title={copy.sendTooltip}
      >
        <CornerDownLeft size={16} />
      </button>

      {/* Discard / Close Button */}
      <button
        type="button"
        onClick={onClose}
        className="flex items-center justify-center w-9 h-9 rounded-full text-[rgb(var(--foreground))]/60 hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/10 transition-colors duration-200 cursor-pointer"
        aria-label={copy.discardAriaLabel}
        title={copy.discardTooltip}
      >
        <X size={17} />
      </button>
    </div>
  );
});

TextInputBar.displayName = "TextInputBar";
