import { memo } from "react";
import { cn } from "@/shared/lib/utils";

interface StatusCapsuleProps {
  label: string;
  dotActive: boolean;
  testing?: boolean;
}

export const StatusCapsule = memo<StatusCapsuleProps>(({ label, dotActive, testing }) => (
  <div
    role="status"
    aria-live="polite"
    aria-label={`Vox Status: ${testing ? "Testing" : label}`}
    className="flex items-center gap-2 px-3 py-1.5 rounded-full border border-[rgb(var(--accent))]/25 bg-[rgb(var(--accent))]/10 dark:bg-[rgba(10,12,14,0.40)] dark:backdrop-blur-md"
  >
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
    <span className="text-[11px] font-mono font-bold tracking-[0.2em] uppercase text-[rgb(var(--accent))] dark:text-[rgb(var(--foreground-muted))]">
      {testing ? "Testing" : label}
    </span>
  </div>
));

StatusCapsule.displayName = "StatusCapsule";
