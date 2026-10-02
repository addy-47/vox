import React, { memo, useState, useRef, useEffect } from "react";
import { cn } from "@/shared/lib/utils";
import { DIALOGUE_COPY } from "@/data/homeCopy";
import { Message, MessageContent, MessageResponse } from "@/shared/components/ai-elements";

interface DialogueBubbleProps {
  role: "user" | "assistant";
  content: string;
  badge?: string;
  className?: string;
}

export const DialogueBubble: React.FC<DialogueBubbleProps> = memo(({
  role,
  content,
  badge,
  className,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);
  const [isOverflowing, setIsOverflowing] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);

  const isUser = role === "user";
  const defaultBadge = isUser ? DIALOGUE_COPY.userBadge : DIALOGUE_COPY.assistantBadge;
  const displayBadge = badge ?? defaultBadge;

  useEffect(() => {
    const el = contentRef.current;
    if (!el) return;
    if (!isExpanded && el.scrollHeight > el.clientHeight + 4) {
      setIsOverflowing(true);
    }
  }, [content, isExpanded]);

  return (
    <Message from={role} className={cn("w-full max-w-[280px]", className)}>
      <MessageContent
        from={role}
        className={cn(
          "w-full break-words text-left font-medium text-[13px] leading-relaxed select-text p-3 rounded-2xl transition-all duration-300",
          isUser
            ? "text-[rgb(var(--foreground-muted))] font-normal bg-[rgb(var(--card))]/80 border border-[rgba(var(--border),0.15)] shadow-md"
            : "text-[rgb(var(--accent))] bg-[rgb(var(--card))]/90 border border-[rgba(var(--accent),0.25)] shadow-xl backdrop-blur-xl"
        )}
      >
        <span
          className={cn(
            "text-[11px] font-mono tracking-widest uppercase block mb-1 font-bold",
            isUser ? "text-[rgb(var(--foreground-muted))]/80" : "text-[rgb(var(--accent))]/80"
          )}
        >
          {displayBadge}
        </span>

        <div
          ref={contentRef}
          className={cn(
            "transition-all duration-300 overflow-hidden",
            !isExpanded && "line-clamp-4"
          )}
        >
          <MessageResponse content={content} variant="bubble" />
        </div>

        {isOverflowing && (
          <button
            type="button"
            onClick={() => setIsExpanded((prev) => !prev)}
            className={cn(
              "inline-flex items-center gap-1 mt-1.5 text-[11px] font-mono tracking-wider font-semibold transition-opacity hover:opacity-100 cursor-pointer",
              isUser
                ? "text-[rgb(var(--foreground-muted))] opacity-75"
                : "text-[rgb(var(--accent))] opacity-85"
            )}
            aria-expanded={isExpanded}
          >
            {isExpanded ? (
              <span>{DIALOGUE_COPY.readLess}</span>
            ) : (
              <React.Fragment>
                <span className="font-bold tracking-normal opacity-60">...</span>
                <span className="underline underline-offset-2">{DIALOGUE_COPY.readMore}</span>
              </React.Fragment>
            )}
          </button>
        )}
      </MessageContent>
    </Message>
  );
});

DialogueBubble.displayName = "DialogueBubble";
