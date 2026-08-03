import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface AudioLevelMeterProps {
  level?: number; // 0..1
  bars?: number;
  active?: boolean;
  className?: string;
}

export const AudioLevelMeter: React.FC<AudioLevelMeterProps> = memo(({
  level = 0,
  bars = 4,
  active = true,
  className,
}) => {
  return (
    <div className={cn("flex items-end gap-[2px] h-4 shrink-0", className)}>
      {Array.from({ length: bars }).map((_, i) => {
        const heightPercent = active
          ? Math.max(15, Math.min(100, (level * 100) + Math.sin(i * 1.5) * 20))
          : 15;

        return (
          <span
            key={i}
            className={cn(
              "w-[2px] rounded-full transition-all duration-150",
              active ? "bg-[rgb(var(--accent))]" : "bg-[rgb(var(--foreground-muted))]/30"
            )}
            style={{ height: `${heightPercent}%` }}
          />
        );
      })}
    </div>
  );
});

AudioLevelMeter.displayName = "AudioLevelMeter";
