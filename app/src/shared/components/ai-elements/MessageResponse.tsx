import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { Markdown } from "@/shared/ui/Markdown";

export interface MessageResponseProps {
  content: string;
  className?: string;
  variant?: "bubble" | "document";
}

export const MessageResponse: React.FC<MessageResponseProps> = memo(
  ({ content, className, variant = "bubble" }) => {
    return (
      <div className={cn("size-full [&>*:first-child]:mt-0 [&>*:last-child]:mb-0 select-text", className)}>
        <Markdown content={content} variant={variant} />
      </div>
    );
  },
  (prevProps, nextProps) => prevProps.content === nextProps.content && prevProps.variant === nextProps.variant
);

MessageResponse.displayName = "MessageResponse";
