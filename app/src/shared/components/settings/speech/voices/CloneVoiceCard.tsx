import { memo, useCallback, useEffect, useRef, useState } from "react";
import {
  AlertCircle,
  Check,
  FolderUp,
  Loader2,
  Mic,
  Plus,
  Sparkles,
  Square,
  X,
} from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { VOICE_CATALOG_COPY } from "@/data/settingsCopy";
import {
  addVoiceFromFile,
  addVoiceFromRecording,
  startBackendRecording,
  stopBackendRecording,
  type UnifiedVoiceDto,
} from "@/services/voicesService";

export interface CloneVoiceCardProps {
  onCreated: (newVoice: UnifiedVoiceDto) => void;
  className?: string;
}

export const CloneVoiceCard = memo(function CloneVoiceCard({
  onCreated,
  className,
}: CloneVoiceCardProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [voiceName, setVoiceName] = useState("");
  const [isRecording, setIsRecording] = useState(false);
  const [duration, setDuration] = useState(0);
  const [recordedPcm, setRecordedPcm] = useState<number[] | null>(null);
  const [recordedSampleRate, setRecordedSampleRate] = useState(0);
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const resetState = useCallback(() => {
    if (isRecording) {
      void stopBackendRecording().catch(() => {});
    }
    setIsRecording(false);
    setDuration(0);
    setRecordedPcm(null);
    setRecordedSampleRate(0);
    setSelectedFilePath(null);
    setVoiceName("");
    setError(null);
    setIsSubmitting(false);
    setIsExpanded(false);
  }, [isRecording]);

  const cardRef = useRef<HTMLDivElement | null>(null);

  useOverlay({
    active: isExpanded,
    onClose: resetState,
    ref: cardRef,
    dismissOnOutside: true,
    shouldDismissOnPointerDown: (target) => {
      if (isRecording) return false;
      return !cardRef.current?.contains(target);
    },
  });

  const handleStartRecording = useCallback(async (e?: React.MouseEvent) => {
    e?.stopPropagation();
    setError(null);
    setSelectedFilePath(null);
    setRecordedPcm(null);
    setRecordedSampleRate(0);
    setDuration(0);
    try {
      await startBackendRecording();
      setIsRecording(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  const handleStopRecording = useCallback(async (e?: React.MouseEvent) => {
    e?.stopPropagation();
    setIsRecording(false);
    try {
      const [pcm, sampleRate] = await stopBackendRecording();
      setRecordedPcm(pcm);
      setRecordedSampleRate(sampleRate);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  useEffect(() => {
    let timer: ReturnType<typeof setInterval> | undefined;
    if (isRecording) {
      timer = setInterval(() => {
        setDuration((d) => (d >= 30 ? 30 : d + 1));
      }, 1000);
    }
    return () => {
      if (timer) clearInterval(timer);
    };
  }, [isRecording]);

  useEffect(() => {
    if (isRecording && duration >= 30) {
      void handleStopRecording();
    }
  }, [isRecording, duration, handleStopRecording]);

  useEffect(() => {
    return () => {
      if (isRecording) {
        void stopBackendRecording().catch(() => {});
      }
    };
  }, [isRecording]);

  const handlePickFile = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    setError(null);
    setRecordedPcm(null);
    setDuration(0);
    try {
      const result = await openDialog({
        multiple: false,
        filters: [{ name: "Audio WAV", extensions: ["wav"] }],
      });
      if (typeof result === "string") {
        setSelectedFilePath(result);
        const fileName = result.split(/[/\\]/).pop() || "";
        const cleanName = fileName.replace(/\.[^/.]+$/, "");
        if (!voiceName.trim() && cleanName) {
          setVoiceName(cleanName.replace(/[-_]/g, " "));
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [voiceName]);

  const hasAudio = recordedPcm != null || selectedFilePath != null;
  const canSubmit = voiceName.trim().length > 0 && hasAudio && !isSubmitting && !isRecording;

  const handleClone = useCallback(async (e?: React.MouseEvent) => {
    e?.stopPropagation();
    const trimmed = voiceName.trim();
    if (!trimmed) {
      setError(VOICE_CATALOG_COPY.cloneVoiceNameRequired);
      return;
    }

    setError(null);
    setIsSubmitting(true);
    try {
      let created: UnifiedVoiceDto;
      if (recordedPcm && recordedSampleRate > 0) {
        created = await addVoiceFromRecording(trimmed, recordedPcm, recordedSampleRate);
      } else if (selectedFilePath) {
        created = await addVoiceFromFile(trimmed, selectedFilePath);
      } else {
        setError(VOICE_CATALOG_COPY.cloneVoiceAudioRequired);
        setIsSubmitting(false);
        return;
      }
      onCreated(created);
      resetState();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSubmitting(false);
    }
  }, [
    voiceName,
    recordedPcm,
    recordedSampleRate,
    selectedFilePath,
    onCreated,
    resetState,
  ]);

  const fileNameOnly = selectedFilePath
    ? selectedFilePath.split(/[/\\]/).pop()
    : null;

  // 1. Resting / Skeleton State: identical geometry and tokens as VoiceCard.tsx
  if (!isExpanded) {
    return (
      <div
        role="button"
        tabIndex={0}
        onClick={() => setIsExpanded(true)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setIsExpanded(true);
          }
        }}
        aria-label={VOICE_CATALOG_COPY.cloneVoice}
        className={cn(
          "group w-full text-left p-4 rounded-xl border transition-all duration-200 relative shrink-0 cursor-pointer flex items-start gap-3.5 hover:z-20 h-full",
          "bg-[rgba(var(--card),0.35)] border-[rgba(var(--foreground),0.07)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--card),0.55)]",
          className
        )}
      >
        {/* Left Action Sphere with Plus Icon */}
        <div className="w-10 h-10 rounded-full flex items-center justify-center shrink-0 mt-0.5 transition-all shadow-sm bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] group-hover:bg-[rgb(var(--accent))] group-hover:text-[rgb(var(--accent-foreground))] group-hover:scale-105">
          <Plus size={16} strokeWidth={2.5} />
        </div>

        {/* Center: Info Column */}
        <div className="flex-1 min-w-0 flex flex-col justify-between h-full">
          <div>
            <div className="flex items-start justify-between gap-2">
              <span className="font-bold text-[14.5px] leading-tight tracking-tight truncate font-sans text-[rgb(var(--foreground))] group-hover:text-[rgb(var(--accent))] transition-colors">
                {VOICE_CATALOG_COPY.cloneVoice}
              </span>
            </div>

            <p className="text-[12px] text-[rgb(var(--foreground-muted))] mt-1.5 line-clamp-2 leading-relaxed">
              {VOICE_CATALOG_COPY.cloneVoiceChatterboxNote}
            </p>
          </div>

          <div className="flex items-center gap-1.5 mt-auto pt-2.5 flex-wrap">
            <span className="text-[10px] font-mono font-medium px-2 py-0.5 rounded-md bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--foreground),0.07)] leading-none">
              {VOICE_CATALOG_COPY.cloned}
            </span>
            <span className="text-[10px] font-mono font-medium px-2 py-0.5 rounded-md bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--foreground),0.07)] leading-none">
              Chatterbox
            </span>
          </div>
        </div>
      </div>
    );
  }

  // 2. Expanded / In-Place Editing State: same card geometry, interactive controls
  return (
    <div
      ref={cardRef}
      className={cn(
        "group w-full text-left p-4 rounded-xl border transition-all duration-200 relative shrink-0 flex items-start gap-3.5 h-full",
        "bg-[rgba(var(--card),0.55)] border-[rgba(var(--accent),0.35)] shadow-[0_0_20px_rgba(var(--accent),0.12)] ring-1 ring-[rgb(var(--accent))]/30",
        isRecording && "border-[rgb(var(--error))] shadow-[0_0_20px_rgba(239,68,68,0.2)] ring-1 ring-[rgb(var(--error))]/40",
        className
      )}
    >
      {/* Left Slot: 40px Action Sphere with Mic Icon */}
      <Tooltip
        label={
          isRecording
            ? VOICE_CATALOG_COPY.cloneVoiceStop
            : recordedPcm
              ? VOICE_CATALOG_COPY.cloneVoiceRecordAgain
              : VOICE_CATALOG_COPY.cloneVoiceRecord
        }
        side="top"
        asChild
      >
        <button
          type="button"
          onClick={isRecording ? handleStopRecording : handleStartRecording}
          disabled={isSubmitting}
          aria-label={isRecording ? VOICE_CATALOG_COPY.cloneVoiceStop : VOICE_CATALOG_COPY.cloneVoiceRecord}
          className={cn(
            "w-10 h-10 rounded-full flex items-center justify-center shrink-0 mt-0.5 transition-all shadow-sm cursor-pointer",
            isRecording
              ? "bg-[rgb(var(--error))] text-white shadow-[0_0_14px_rgba(239,68,68,0.5)] scale-105 animate-pulse"
              : recordedPcm
                ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-[0_0_12px_rgba(var(--accent),0.4)] hover:scale-105 active:scale-95"
                : "bg-[rgba(var(--foreground),0.08)] text-[rgb(var(--foreground))] hover:bg-[rgb(var(--accent))] hover:text-[rgb(var(--accent-foreground))] hover:scale-105 active:scale-95"
          )}
        >
          {isRecording ? (
            <Square size={14} className="fill-current" />
          ) : (
            <Mic size={16} />
          )}
        </button>
      </Tooltip>

      {/* Center: Info Column */}
      <div className="flex-1 min-w-0 flex flex-col justify-between h-full">
        <div>
          {/* Top Row: Name Input on Left, Tick & Cross on Right */}
          <div className="flex items-center justify-between gap-2">
            <input
              autoFocus
              type="text"
              value={voiceName}
              onChange={(e) => setVoiceName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && canSubmit) {
                  e.preventDefault();
                  void handleClone();
                } else if (e.key === "Escape") {
                  e.preventDefault();
                  resetState();
                }
              }}
              placeholder={VOICE_CATALOG_COPY.cloneVoiceNamePlaceholder}
              disabled={isSubmitting}
              className="font-bold text-[14px] leading-tight tracking-tight font-sans text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 bg-transparent border-b border-[rgba(var(--foreground),0.15)] focus:border-[rgb(var(--accent))] focus:outline-none w-full py-0.5 transition-colors"
            />

            <div className="flex items-center gap-1 shrink-0 -mr-1">
              {/* Submit (Tick) Button */}
              <Tooltip label={VOICE_CATALOG_COPY.cloneVoiceSubmit} side="top" asChild>
                <button
                  type="button"
                  onClick={handleClone}
                  disabled={!canSubmit}
                  aria-label={VOICE_CATALOG_COPY.cloneVoiceSubmit}
                  className={cn(
                    "w-7 h-7 rounded-lg shrink-0 transition-all cursor-pointer flex items-center justify-center",
                    canSubmit
                      ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:scale-105 active:scale-95 shadow-[0_0_10px_rgba(var(--accent),0.3)]"
                      : "text-[rgb(var(--foreground-muted))]/30 cursor-not-allowed"
                  )}
                >
                  {isSubmitting ? (
                    <Loader2 size={13} className="animate-spin" />
                  ) : (
                    <Check size={14} strokeWidth={2.5} />
                  )}
                </button>
              </Tooltip>

              {/* Cancel (Cross) Button */}
              <Tooltip label={VOICE_CATALOG_COPY.cloneVoiceCancel} side="top" asChild>
                <button
                  type="button"
                  onClick={resetState}
                  disabled={isSubmitting}
                  aria-label={VOICE_CATALOG_COPY.cloneVoiceCancel}
                  className="w-7 h-7 rounded-lg text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-all cursor-pointer flex items-center justify-center"
                >
                  <X size={14} />
                </button>
              </Tooltip>
            </div>
          </div>

          {/* Subtitle / Status row */}
          <p className={cn(
            "text-[12px] mt-1.5 line-clamp-1 leading-relaxed",
            error
              ? "text-[rgb(var(--error))]"
              : isRecording
                ? "text-[rgb(var(--error))] font-medium animate-pulse"
                : "text-[rgb(var(--foreground-muted))]"
          )}>
            {error ? (
              <span className="flex items-center gap-1">
                <AlertCircle size={12} className="shrink-0" />
                <span className="truncate">{error}</span>
              </span>
            ) : isRecording ? (
              `Recording (${duration}s / 30s) · Speak clearly`
            ) : recordedPcm ? (
              `Recorded ${duration}s audio sample`
            ) : fileNameOnly ? (
              `WAV: ${fileNameOnly}`
            ) : (
              VOICE_CATALOG_COPY.cloneVoiceReadyPrompt
            )}
          </p>
        </div>

        {/* Bottom Row: Metadata Badges + Choose File Button on Right */}
        <div className="flex items-center justify-between gap-2 mt-auto pt-2.5">
          <div className="flex items-center gap-1.5 flex-wrap">
            <span className="text-[10px] font-mono font-medium px-2 py-0.5 rounded-md bg-[rgba(var(--accent),0.08)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.2)] leading-none flex items-center gap-1">
              <Sparkles size={9} />
              <span>{VOICE_CATALOG_COPY.cloned}</span>
            </span>
            {recordedPcm && (
              <span className="text-[10px] font-mono font-medium px-2 py-0.5 rounded-md bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] leading-none">
                {duration}s sample
              </span>
            )}
          </div>

          <Tooltip label={VOICE_CATALOG_COPY.cloneVoiceSelectFile} side="top" asChild>
            <button
              type="button"
              onClick={handlePickFile}
              disabled={isRecording || isSubmitting}
              className="inline-flex items-center gap-1 text-[11px] font-medium text-[rgb(var(--accent))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer shrink-0"
              aria-label={VOICE_CATALOG_COPY.cloneVoiceSelectFile}
            >
              <FolderUp size={12} />
              <span>{selectedFilePath ? VOICE_CATALOG_COPY.cloneVoiceChangeFile : VOICE_CATALOG_COPY.cloneVoiceUpload}</span>
            </button>
          </Tooltip>
        </div>
      </div>
    </div>
  );
});
