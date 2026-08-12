import React, { memo } from "react";
import ReactMarkdown from "react-markdown";
import { useStreamingRenderer } from "@/shared/hooks/useStreamingRenderer";

interface ActiveTranscriptProps {
  transcript: string;
  assistantText: string;
}

const MarkdownComponents = {
  p: ({ children }: any) => <p className="mb-1.5 last:mb-0 leading-relaxed">{children}</p>,
  code: ({ children }: any) => (
    <code className="px-1 py-0.5 rounded bg-[rgb(var(--foreground))]/10 font-mono text-[11px]">
      {children}
    </code>
  ),
};

export const ActiveTranscript: React.FC<ActiveTranscriptProps> = memo(({ transcript, assistantText }) => {
  const streamedTranscript = useStreamingRenderer(transcript);
  const streamedAssistantText = useStreamingRenderer(assistantText);

  if (!streamedTranscript && !streamedAssistantText) return null;

  return (
    <div className="w-full flex flex-col gap-4 items-center select-text">
      {streamedTranscript && (
        <div className="w-full max-w-[280px] break-words text-left text-[rgb(var(--foreground))] font-normal text-[13px] leading-relaxed prose prose-invert select-text p-3 rounded-2xl bg-[rgb(var(--card))]/80 border border-[rgba(var(--border),0.15)] shadow-lg backdrop-blur-xl">
          <span className="text-[9px] font-mono tracking-widest text-[rgb(var(--foreground-muted))] uppercase block mb-1 font-bold">
            USER
          </span>
          <ReactMarkdown components={MarkdownComponents}>{streamedTranscript}</ReactMarkdown>
        </div>
      )}

      {streamedAssistantText && (
        <div className="w-full max-w-[280px] break-words text-left text-[rgb(var(--accent))] font-medium text-[13px] leading-relaxed prose prose-invert select-text p-3 rounded-2xl bg-[rgb(var(--card))]/90 border border-[rgba(var(--accent),0.25)] shadow-xl backdrop-blur-xl">
          <span className="text-[9px] font-mono tracking-widest text-[rgb(var(--accent))]/80 uppercase block mb-1 font-bold">
            VOX
          </span>
          <ReactMarkdown components={MarkdownComponents}>{streamedAssistantText}</ReactMarkdown>
        </div>
      )}
    </div>
  );
});

ActiveTranscript.displayName = "ActiveTranscript";
