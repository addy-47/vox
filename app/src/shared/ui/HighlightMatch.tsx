import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface HighlightMatchProps {
  text: string;
  query: string;
  className?: string;
  highlightClassName?: string;
}

/**
 * High-performance, memory-efficient substring highlighter.
 * Uses exact case-insensitive matching with RegExp escaping and React fragments.
 */
export const HighlightMatch = memo<HighlightMatchProps>(({
  text,
  query,
  className,
  highlightClassName = "text-[rgb(var(--accent))] bg-[rgba(var(--accent),0.12)] font-semibold rounded-[3px] px-1 py-0.2",
}) => {
  const q = query.trim();
  if (!q || !text) {
    return <span className={className}>{text}</span>;
  }

  const escaped = q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const regex = new RegExp(`(${escaped})`, "gi");
  const parts = text.split(regex);
  const lowerQ = q.toLowerCase();

  return (
    <span className={className}>
      {parts.map((part, i) =>
        part.toLowerCase() === lowerQ ? (
          <mark key={i} className={cn("bg-transparent", highlightClassName)}>
            {part}
          </mark>
        ) : (
          <React.Fragment key={i}>{part}</React.Fragment>
        )
      )}
    </span>
  );
});

HighlightMatch.displayName = "HighlightMatch";
