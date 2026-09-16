import React, { memo } from "react";
import { useStreamingRenderer } from "@/shared/hooks/useStreamingRenderer";
import { DIALOGUE_COPY } from "@/data/homeCopy";
import { Markdown } from "@/shared/ui/Markdown";

interface ActiveTranscriptProps {
  transcript: string;
  assistantText: string;
}

export const ActiveTranscript: React.FC<ActiveTranscriptProps> = memo(({ transcript, assistantText }) => {
  const streamedTranscript = useStreamingRenderer(transcript);
  const streamedAssistantText = useStreamingRenderer(assistantText);

  if (!streamedTranscript && !streamedAssistantText) return null;

  return (
    <div className="w-full flex flex-col gap-4 items-center select-text">
      {streamedTranscript && (
        <div className="w-full max-w-[280px] break-words text-left text-[rgb(var(--foreground))] font-normal text-[13px] leading-relaxed select-text p-3 rounded-2xl bg-[rgb(var(--card))]/80 border border-[rgba(var(--border),0.15)] shadow-lg backdrop-blur-xl">
          <span className="text-[11px] font-mono tracking-widest text-[rgb(var(--foreground-muted))] uppercase block mb-1 font-bold">
            {DIALOGUE_COPY.userBadge}
          </span>
          <Markdown content={streamedTranscript} variant="bubble" />
        </div>
      )}

      {streamedAssistantText && (
        <div className="w-full max-w-[280px] break-words text-left text-[rgb(var(--accent))] font-medium text-[13px] leading-relaxed select-text p-3 rounded-2xl bg-[rgb(var(--card))]/90 border border-[rgba(var(--accent),0.25)] shadow-xl backdrop-blur-xl">
          <span className="text-[11px] font-mono tracking-widest text-[rgb(var(--accent))]/80 uppercase block mb-1 font-bold">
            VOX
          </span>
          <Markdown content={streamedAssistantText} variant="bubble" />
        </div>
      )}
    </div>
  );
});

ActiveTranscript.displayName = "ActiveTranscript";
