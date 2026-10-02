import React, { memo } from "react";
import { DialogueBubble } from "./DialogueBubble";

interface ActiveTranscriptProps {
  transcript: string;
  assistantText: string;
}

export const ActiveTranscript: React.FC<ActiveTranscriptProps> = memo(({ transcript, assistantText }) => {
  if (!transcript && !assistantText) return null;

  return (
    <div className="w-full flex flex-col gap-4 items-center select-text">
      {transcript && (
        <DialogueBubble role="user" content={transcript} />
      )}

      {assistantText && (
        <DialogueBubble role="assistant" content={assistantText} />
      )}
    </div>
  );
});

ActiveTranscript.displayName = "ActiveTranscript";
