import React from "react";
import { cn } from "@/shared/lib/utils";

interface StatusCapsuleProps {
  label: string;
  dotActive: boolean;
  testing?: boolean;
}

export const StatusCapsule: React.FC<StatusCapsuleProps> = ({ label, dotActive, testing }) => (
  <div className="flex items-center gap-2 px-3 py-1.5 rounded-full border border-[rgb(var(--accent))]/30 bg-[rgb(var(--accent))]/10 dark:glass-elevated dark:glass-base shadow-sm">
    {testing ? (
      <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-pulse" />
    ) : (
      <span
        className={cn(
          "w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] transition-all duration-700",
          dotActive ? "opacity-100 shadow-[0_0_6px_rgba(var(--accent),0.8)]" : "opacity-30"
        )}
        style={dotActive ? { animation: "pulse-slow 2.5s ease-in-out infinite" } : {}}
      />
    )}
    <span className="text-[10px] font-mono font-bold tracking-[0.2em] uppercase text-[rgb(var(--accent))] dark:text-[rgb(var(--foreground-muted))]">
      {testing ? "Testing" : label}
    </span>
  </div>
);
