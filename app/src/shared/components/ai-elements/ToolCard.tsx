import React, { memo, useState } from "react";
import { cn } from "@/shared/lib/utils";
import { Wrench, ChevronDown, CheckCircle, Clock, XCircle } from "lucide-react";

export type ToolStatus = "running" | "completed" | "error";

export interface ToolCardProps {
  toolName: string;
  status: ToolStatus;
  title?: string;
  args?: Record<string, unknown> | string;
  result?: string;
  className?: string;
}

export const ToolCard: React.FC<ToolCardProps> = memo(({
  toolName,
  status,
  title,
  args,
  result,
  className,
}) => {
  const [isOpen, setIsOpen] = useState(false);

  const statusIcons: Record<ToolStatus, React.ReactNode> = {
    running: <Clock className="w-3.5 h-3.5 text-[rgb(var(--accent))] animate-spin" />,
    completed: <CheckCircle className="w-3.5 h-3.5 text-emerald-400" />,
    error: <XCircle className="w-3.5 h-3.5 text-red-400" />,
  };

  const statusLabels: Record<ToolStatus, string> = {
    running: "Running",
    completed: "Completed",
    error: "Failed",
  };

  return (
    <div
      className={cn(
        "w-full max-w-[280px] rounded-xl border border-[rgba(var(--border),0.2)] bg-[rgba(var(--card),0.7)] backdrop-blur-md overflow-hidden transition-all duration-200 text-left text-xs",
        className
      )}
    >
      <button
        type="button"
        onClick={() => setIsOpen((prev) => !prev)}
        className="w-full flex items-center justify-between p-2.5 hover:bg-[rgba(var(--foreground),0.03)] cursor-pointer"
      >
        <div className="flex items-center gap-2 min-w-0">
          <Wrench className="w-3.5 h-3.5 text-[rgb(var(--foreground-muted))] shrink-0" />
          <span className="font-mono font-semibold truncate text-[11px] text-[rgb(var(--foreground))]">
            {title || toolName}
          </span>
          <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[10px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.15)] text-[rgb(var(--foreground-muted))]">
            {statusIcons[status]}
            {statusLabels[status]}
          </span>
        </div>
        <ChevronDown
          className={cn(
            "w-3.5 h-3.5 text-[rgb(var(--foreground-muted))] transition-transform duration-200 shrink-0",
            isOpen && "rotate-180"
          )}
        />
      </button>

      {isOpen && (
        <div className="px-2.5 pb-2.5 pt-1 border-t border-[rgba(var(--border),0.1)] flex flex-col gap-2 font-mono text-[11px] text-[rgb(var(--foreground-muted))]">
          {args && (
            <div>
              <span className="text-[10px] uppercase font-bold text-[rgb(var(--foreground-muted))]/70">Inputs:</span>
              <pre className="mt-0.5 p-1.5 rounded bg-black/20 overflow-x-auto text-[10px] select-text">
                {typeof args === "string" ? args : JSON.stringify(args, null, 2)}
              </pre>
            </div>
          )}
          {result && (
            <div>
              <span className="text-[10px] uppercase font-bold text-[rgb(var(--foreground-muted))]/70">Output:</span>
              <pre className="mt-0.5 p-1.5 rounded bg-black/20 overflow-x-auto text-[10px] select-text">
                {result}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
});

ToolCard.displayName = "ToolCard";
