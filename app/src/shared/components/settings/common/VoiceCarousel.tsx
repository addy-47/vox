import React, { memo } from "react";
import { Volume2, Play, Check } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export interface VoiceOption {
  id: string;
  name: string;
  gender?: string;
  accent?: string;
  previewUrl?: string;
}

interface VoiceCarouselProps {
  voices: VoiceOption[];
  selectedVoiceId: string;
  onSelectVoice: (id: string) => void;
  onTestClip?: (voiceId: string) => void;
  testingVoiceId?: string | null;
  className?: string;
}

export const VoiceCarousel: React.FC<VoiceCarouselProps> = memo(
  ({ voices, selectedVoiceId, onSelectVoice, onTestClip, testingVoiceId, className }) => {
    return (
      <div className={cn("grid grid-cols-2 sm:grid-cols-3 gap-2.5", className)}>
        {voices.map((v) => {
          const isSelected = v.id === selectedVoiceId;
          const isTesting = testingVoiceId === v.id;

          return (
            <div
              key={v.id}
              onClick={() => onSelectVoice(v.id)}
              className={cn(
                "relative flex items-center justify-between p-3 rounded-xl border cursor-pointer transition-all duration-300 group",
                isSelected
                  ? "bg-[rgba(var(--accent),0.1)] border-[rgba(var(--accent),0.4)] text-[rgb(var(--foreground))]"
                  : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--border),0.1)] hover:bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))]"
              )}
            >
              <div className="flex items-center gap-2.5 min-w-0">
                <div
                  className={cn(
                    "w-7 h-7 rounded-lg flex items-center justify-center shrink-0 border transition-colors",
                    isSelected
                      ? "bg-[rgb(var(--accent))] text-black border-[rgb(var(--accent))]"
                      : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.1)] text-[rgb(var(--foreground-muted))]"
                  )}
                >
                  {isSelected ? <Check size={14} strokeWidth={3} /> : <Volume2 size={14} />}
                </div>
                <div className="min-w-0">
                  <h5 className="text-xs font-semibold text-[rgb(var(--foreground))] truncate">
                    {v.name}
                  </h5>
                  {(v.gender || v.accent) && (
                    <p className="text-[10px] text-[rgb(var(--foreground-muted))]/70 truncate">
                      {[v.gender, v.accent].filter(Boolean).join(" • ")}
                    </p>
                  )}
                </div>
              </div>

              {onTestClip && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onTestClip(v.id);
                  }}
                  className={cn(
                    "p-1.5 rounded-lg border transition-all shrink-0 ml-1.5",
                    isTesting
                      ? "bg-[rgb(var(--accent))] text-black border-[rgb(var(--accent))] animate-pulse"
                      : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.1)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                  )}
                  title="Test voice audio"
                >
                  <Play size={12} fill={isTesting ? "black" : "none"} />
                </button>
              )}
            </div>
          );
        })}
      </div>
    );
  }
);

VoiceCarousel.displayName = "VoiceCarousel";
