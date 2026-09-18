import React, { memo } from "react";
import { useStreamingRenderer } from "@/shared/hooks/useStreamingRenderer";
import { DialogueBubble } from "./DialogueBubble";

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
        <DialogueBubble role="user" content={streamedTranscript} />
      )}

      {streamedAssistantText && (
        <DialogueBubble role="assistant" content={streamedAssistantText} />
      )}
    </div>
  );
});

ActiveTranscript.displayName = "ActiveTranscript";
