import React, { memo, type ComponentProps, type HTMLAttributes } from "react";
import { cn } from "@/shared/lib/utils";

export interface MessageProps extends HTMLAttributes<HTMLDivElement> {
  from: "user" | "assistant" | "system";
}

export const Message: React.FC<MessageProps> = memo(({ className, from, ...props }) => (
  <div
    className={cn(
      "group flex w-full max-w-[95%] flex-col gap-1.5 transition-all duration-300",
      from === "user" ? "is-user ml-auto items-end" : "is-assistant mr-auto items-start",
      className
    )}
    {...props}
  />
));
Message.displayName = "Message";

export interface MessageContentProps extends HTMLAttributes<HTMLDivElement> {
  from?: "user" | "assistant" | "system";
}

export const MessageContent: React.FC<MessageContentProps> = memo(({
  children,
  className,
  from,
  ...props
}) => (
  <div
    className={cn(
      "w-fit min-w-0 max-w-full flex-col gap-2 overflow-hidden text-[13px] leading-relaxed select-text p-3 rounded-2xl shadow-md",
      from === "user" || "group-[.is-user]:ml-auto"
        ? "text-[rgb(var(--foreground-muted))] font-normal bg-[rgb(var(--card))]/80 border border-[rgba(var(--border),0.15)] shadow-md"
        : "text-[rgb(var(--foreground))] bg-[rgb(var(--card))]/90 border border-[rgba(var(--accent),0.25)] shadow-xl backdrop-blur-xl",
      className
    )}
    {...props}
  >
    {children}
  </div>
));
MessageContent.displayName = "MessageContent";

export type MessageActionsProps = ComponentProps<"div">;

export const MessageActions: React.FC<MessageActionsProps> = memo(({
  className,
  children,
  ...props
}) => (
  <div className={cn("flex items-center gap-1 mt-1 opacity-70 hover:opacity-100 transition-opacity", className)} {...props}>
    {children}
  </div>
));
MessageActions.displayName = "MessageActions";
