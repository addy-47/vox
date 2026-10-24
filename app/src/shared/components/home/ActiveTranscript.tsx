import React, { memo } from "react";
import { DialogueBubble } from "./DialogueBubble";
import { useSessionStore } from "@/store/sessionStore";

interface ActiveTranscriptProps {
  transcript?: string;
  assistantText?: string;
}

export const ActiveTranscript: React.FC<ActiveTranscriptProps> = memo(({ transcript: propTranscript, assistantText: propAssistantText }) => {
  const storeTranscript = useSessionStore((s) => s.transcript);
  const storeAssistantText = useSessionStore((s) => s.assistantText);

  const transcript = propTranscript ?? storeTranscript;
  const assistantText = propAssistantText ?? storeAssistantText;

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
