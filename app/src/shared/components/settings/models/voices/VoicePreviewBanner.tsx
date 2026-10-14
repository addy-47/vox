import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  Cpu,
  Edit3,
  Globe,
  Heart,
  Loader2,
  Pause,
  Play,
  Sparkles,
  Users,
  X,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { VOICE_CATALOG_COPY } from "@/data/settingsCopy";
import type { UnifiedVoiceDto } from "@/services/voiceService";

export interface VoicePreviewBannerProps {
  voice: UnifiedVoiceDto | null;
  isPlaying: boolean;
  isSynthesizing: boolean;
  previewProgress: number;
  previewCurrentTime: number;
  previewDuration: number;
  previewPrompt: string;
  sessionBusy?: boolean;
  onTogglePreview: (voice: UnifiedVoiceDto) => void;
  onToggleFavorite: (id: string) => void;
  onSeekPreview: (fraction: number) => void;
  onSetPreviewPrompt: (prompt: string) => void;
}

function tagLabel(tag?: string | null): string {
  if (!tag) return "";
  const t = tag.trim();
  if (!t) return t;
  return t.charAt(0).toUpperCase() + t.slice(1);
}

export function getVoiceWaveform(voiceId: string, count = 46): number[] {
  let hash = 0;
  for (let i = 0; i < voiceId.length; i++) {
    hash = (hash << 5) - hash + voiceId.charCodeAt(i);
    hash |= 0;
  }
  const bars: number[] = [];
  for (let i = 0; i < count; i++) {
    const x = Math.sin((i + 1) * 0.45 + Math.abs(hash % 41)) * 10000;
    const rand = x - Math.floor(x);
    const envelope = Math.sin((i / (count - 1)) * Math.PI);
    const height = Math.max(0.2, Math.min(1.0, 0.22 + rand * 0.7 * (0.35 + envelope * 0.65)));
    bars.push(height);
  }
  return bars;
}

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export const VoicePreviewBanner = memo(
  ({
    voice,
    isPlaying,
    isSynthesizing,
    previewProgress,
    previewCurrentTime,
    previewDuration,
    previewPrompt,
    sessionBusy = false,
    onTogglePreview,
    onToggleFavorite,
    onSeekPreview,
    onSetPreviewPrompt,
  }: VoicePreviewBannerProps) => {
    const [isEditingPrompt, setIsEditingPrompt] = useState(false);
    const [customPromptInput, setCustomPromptInput] = useState(previewPrompt);
    const scrubberRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
      if (!isEditingPrompt) {
        setCustomPromptInput(previewPrompt);
      }
    }, [previewPrompt, isEditingPrompt]);

    const handleScrubberClick = useCallback(
      (e: React.MouseEvent<HTMLDivElement>) => {
        if (!scrubberRef.current) return;
        const rect = scrubberRef.current.getBoundingClientRect();
        const clickX = e.clientX - rect.left;
        const fraction = Math.max(0, Math.min(1, clickX / rect.width));
        onSeekPreview(fraction);
      },
      [onSeekPreview]
    );

    const handleSavePrompt = useCallback(() => {
      onSetPreviewPrompt(customPromptInput);
      setIsEditingPrompt(false);
      if (voice && isPlaying) {
        onTogglePreview(voice);
      }
    }, [customPromptInput, isPlaying, onSetPreviewPrompt, onTogglePreview, voice]);

    const waveformBars = useMemo(
      () => (voice ? getVoiceWaveform(voice.id, 46) : []),
      [voice]
    );

    if (!voice) return null;

    return (
      <div className="shrink-0 rounded-2xl border border-[rgba(var(--foreground),0.08)] bg-[rgba(var(--card),0.75)] backdrop-blur-xl p-5 md:p-6 lg:py-6 lg:px-7 shadow-lg hidden md:grid md:grid-cols-[auto_1fr_auto] lg:grid-cols-[auto_1fr_auto_auto] items-center gap-x-6 lg:gap-x-8 gap-y-4 md:gap-y-4.5">
        {/* Top-Left: Circular Play Button */}
        <button
          type="button"
          disabled={!voice.has_preview && sessionBusy}
          onClick={() => onTogglePreview(voice)}
          aria-label={isPlaying ? "Pause preview" : `Preview ${voice.name}`}
          className={cn(
            "col-start-1 row-start-1 md:row-span-2 md:self-center",
            "w-14 h-14 md:w-16 md:h-16 lg:w-[68px] lg:h-[68px] rounded-full flex items-center justify-center shrink-0 transition-all shadow-md",
            !voice.has_preview && sessionBusy
              ? "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))]/30 cursor-not-allowed"
              : isPlaying
                ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-[0_0_24px_rgba(var(--accent),0.45)] scale-105"
                : "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] hover:bg-[rgb(var(--accent))] hover:text-[rgb(var(--accent-foreground))] hover:shadow-[0_0_16px_rgba(var(--accent),0.3)] hover:scale-105 active:scale-95 cursor-pointer"
          )}
        >
          {isSynthesizing && isPlaying ? (
            <Loader2 size={24} className="animate-spin" />
          ) : isPlaying ? (
            <Pause size={24} />
          ) : (
            <Play size={24} className="ml-1" />
          )}
        </button>

        {/* Next to Icon: Title, Description, and Tag Badges */}
        <div className="col-start-2 row-start-1 min-w-0 flex flex-col justify-center gap-1.5 sm:gap-2">
          <div className="flex items-baseline gap-2.5 min-w-0">
            <span className="font-bold text-[16px] md:text-[18px] text-[rgb(var(--foreground))] truncate font-sans tracking-tight">
              {voice.name}
            </span>
            <span className="text-[12.5px] md:text-[13px] text-[rgb(var(--foreground-muted))] truncate hidden sm:inline leading-relaxed">
              {voice.description || VOICE_CATALOG_COPY.fallbackDescription}
            </span>
          </div>

          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-[10.5px] md:text-[11px] font-medium font-sans px-2.5 py-0.5 rounded-md bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--foreground),0.06)] leading-none whitespace-nowrap">
              {tagLabel(voice.gender)}
            </span>
            <span className="text-[10.5px] md:text-[11px] font-medium font-sans px-2.5 py-0.5 rounded-md bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--foreground),0.06)] leading-none whitespace-nowrap">
              {voice.accent || voice.locale}
            </span>
            <span className="text-[10.5px] md:text-[11px] font-medium font-sans px-2.5 py-0.5 rounded-md bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.18)] leading-none whitespace-nowrap">
              {tagLabel(voice.tags.find((t) => t.toLowerCase() !== "embedded" && t.toLowerCase() !== "fast") || "Natural")}
            </span>
          </div>
        </div>

        {/* Top-Right: Edit Phrase and Like (Favorite) Buttons */}
        <div className="col-start-3 row-start-1 flex items-center gap-2 shrink-0 self-center">
          {voice.source_kind !== "custom" && (
            <button
              type="button"
              onClick={() => {
                if (isEditingPrompt) {
                  setIsEditingPrompt(false);
                } else {
                  setCustomPromptInput(previewPrompt);
                  setIsEditingPrompt(true);
                }
              }}
              title={isEditingPrompt ? "Cancel editing phrase" : "Customize sample sentence"}
              className={cn(
                "p-2 rounded-lg transition-all cursor-pointer",
                isEditingPrompt
                  ? "text-[rgb(var(--accent))] bg-[rgba(var(--accent),0.12)] shadow-xs"
                  : "text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--foreground),0.04)]"
              )}
            >
              <Edit3 size={15} />
            </button>
          )}
          <button
            type="button"
            onClick={() => onToggleFavorite(voice.id)}
            aria-label={voice.is_favorite ? "Remove favorite" : "Add favorite"}
            className={cn(
              "p-2 rounded-lg transition-transform hover:scale-110 cursor-pointer",
              voice.is_favorite
                ? "text-[rgb(var(--accent))]"
                : "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--accent))]"
            )}
          >
            <Heart size={18} fill={voice.is_favorite ? "currentColor" : "none"} strokeWidth={voice.is_favorite ? 2 : 1.75} />
          </button>
        </div>

        {/* Row 2 Col 2-3: Waveform Scrubber or Edit Prompt */}
        <div className="col-span-full md:col-start-2 md:col-span-2 row-start-2 w-full min-w-0">
          {isEditingPrompt ? (
            <div className="flex items-center gap-2.5 animate-fade-in w-full h-10">
              <input
                type="text"
                value={customPromptInput}
                onChange={(e) => setCustomPromptInput(e.target.value)}
                placeholder="Type phrase to synthesize..."
                maxLength={120}
                autoFocus
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleSavePrompt();
                  else if (e.key === "Escape") setIsEditingPrompt(false);
                }}
                className="flex-1 min-w-0 bg-transparent border-b border-[rgb(var(--accent))] text-[13px] text-[rgb(var(--foreground))] outline-none py-1.5 placeholder:text-[rgb(var(--foreground-muted))]/50"
              />
              <button
                type="button"
                onClick={handleSavePrompt}
                className="px-3 py-1.5 text-[11.5px] font-bold rounded-lg bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] cursor-pointer shrink-0 transition-opacity hover:opacity-90 active:scale-95"
              >
                Apply
              </button>
              <button
                type="button"
                onClick={() => setIsEditingPrompt(false)}
                className="p-1.5 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] cursor-pointer shrink-0 transition-colors"
                aria-label="Cancel editing"
              >
                <X size={15} />
              </button>
            </div>
          ) : (
            <div className="flex items-center gap-3 sm:gap-3.5 w-full">
              <span className="font-mono text-[11px] sm:text-[11.5px] tabular-nums text-[rgb(var(--foreground-muted))] w-8 sm:w-9 text-right shrink-0">
                {formatTime(previewCurrentTime)}
              </span>

              <div
                ref={scrubberRef}
                onClick={voice.has_preview || !sessionBusy ? handleScrubberClick : undefined}
                className={cn(
                  "flex-1 h-10 sm:h-11 flex items-center gap-[2.5px] sm:gap-[3px] px-1.5 rounded-lg transition-all select-none",
                  !voice.has_preview && !isPlaying
                    ? "opacity-35 cursor-not-allowed"
                    : "cursor-pointer hover:bg-[rgba(var(--foreground),0.03)]"
                )}
                title={!voice.has_preview && !isPlaying ? "No preview synthesized yet" : "Click to seek preview"}
              >
                {waveformBars.map((height, i) => {
                  const barProgress = i / waveformBars.length;
                  const isPlayed = previewProgress > 0 && barProgress <= previewProgress;
                  const isSkeleton = !voice.has_preview && !isPlaying;
                  return (
                    <div
                      key={i}
                      style={{ height: `${Math.round(isSkeleton ? 24 : height * 100)}%` }}
                      className={cn(
                        "flex-1 rounded-full min-w-[2px] max-w-[4px] transition-all duration-75",
                        isSkeleton
                          ? "bg-[rgba(var(--foreground),0.2)]"
                          : isPlayed
                            ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.4)]"
                            : "bg-[rgba(var(--foreground),0.18)] hover:bg-[rgba(var(--foreground),0.35)]"
                      )}
                    />
                  );
                })}
              </div>

              <span className="font-mono text-[11px] sm:text-[11.5px] tabular-nums text-[rgb(var(--foreground-muted))] w-8 sm:w-9 shrink-0">
                {formatTime(previewDuration)}
              </span>
            </div>
          )}
        </div>

        {/* Right Section: Rich Metadata Indicators (2x2 Grid) - Language, Gender, Style, Model (Hidden on compact/tablet) */}
        <div className="hidden lg:grid lg:col-start-4 lg:row-span-2 grid-cols-2 gap-x-7 gap-y-3.5 border-l border-[rgba(var(--foreground),0.08)] pl-6 lg:pl-7 shrink-0 text-[12px]">
          <div className="flex items-center gap-2.5 min-w-[110px]">
            <div className="w-8 h-8 rounded-lg bg-[rgba(var(--foreground),0.04)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
              <Globe size={15} />
            </div>
            <div className="flex flex-col min-w-0">
              <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70 uppercase tracking-wider leading-none">
                Language
              </span>
              <span className="font-semibold text-[rgb(var(--foreground))] leading-tight mt-1 truncate">
                {voice.locale || "English"}
              </span>
            </div>
          </div>

          <div className="flex items-center gap-2.5 min-w-[110px]">
            <div className="w-8 h-8 rounded-lg bg-[rgba(var(--foreground),0.04)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
              <Users size={15} />
            </div>
            <div className="flex flex-col min-w-0">
              <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70 uppercase tracking-wider leading-none">
                Gender
              </span>
              <span className="font-semibold text-[rgb(var(--foreground))] capitalize leading-tight mt-1 truncate">
                {tagLabel(voice.gender) || "Neutral"}
              </span>
            </div>
          </div>

          <div className="flex items-center gap-2.5 min-w-[110px]">
            <div className="w-8 h-8 rounded-lg bg-[rgba(var(--foreground),0.04)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
              <Sparkles size={15} />
            </div>
            <div className="flex flex-col min-w-0">
              <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70 uppercase tracking-wider leading-none">
                Style
              </span>
              <span className="font-semibold text-[rgb(var(--foreground))] capitalize leading-tight mt-1 truncate">
                {tagLabel(voice.tags.find((t) => t.toLowerCase() !== "embedded" && t.toLowerCase() !== "fast") || voice.accent || "Natural")}
              </span>
            </div>
          </div>

          <div className="flex items-center gap-2.5 min-w-[110px]">
            <div className="w-8 h-8 rounded-lg bg-[rgba(var(--foreground),0.04)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
              <Cpu size={15} />
            </div>
            <div className="flex flex-col min-w-0">
              <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70 uppercase tracking-wider leading-none">
                Model
              </span>
              <span className="font-semibold text-[rgb(var(--foreground))] capitalize leading-tight mt-1 truncate">
                {voice.provider_id
                  ? voice.provider_id
                      .split(/[-_]/)
                      .map((w) => w.charAt(0).toUpperCase() + w.slice(1).toLowerCase())
                      .join(" ")
                  : "Neural"}
              </span>
            </div>
          </div>
        </div>
      </div>
    );
  }
);

VoicePreviewBanner.displayName = "VoicePreviewBanner";
