import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";

interface SettingsCardSkeletonProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const SettingsCardSkeleton: React.FC<SettingsCardSkeletonProps> = memo(({
  layoutMode = "full-max",
}) => {
  return (
    <div
      className={cn(
        "w-full flex flex-col select-none relative overflow-hidden",
        layoutMode === "small"
          ? "bg-transparent p-0"
          : cn(
              "glass-card p-4 min-h-[220px] max-w-[460px]",
              layoutMode === "full-min" ? "lg:w-[320px] xl:w-[380px] 2xl:w-[440px]" : "lg:w-[440px]"
            )
      )}
    >
      {/* Ambient Glass Shimmer Sweep */}
      <span className="absolute inset-y-0 left-0 w-1/2 -skew-x-12 pointer-events-none bg-gradient-to-r from-transparent via-[rgba(var(--accent),0.07)] to-transparent animate-[skeleton-shimmer_1.6s_ease-in-out_infinite]" />

      <div className="flex flex-col gap-3 w-full">
        {/* ── 1. Compact Header: Icon + Title + Action Pill ── */}
        <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
          <div className="flex items-center gap-2">
            <div className="w-6 h-6 rounded-lg bg-[rgba(var(--accent),0.12)] animate-pulse" />
            <div className="space-y-1">
              <div className="w-24 h-3 rounded bg-[rgba(var(--foreground),0.12)] animate-pulse" />
              <div className="w-14 h-1.5 rounded bg-[rgba(var(--foreground),0.05)] animate-pulse" />
            </div>
          </div>

          {/* Top-Right Toggle Pill */}
          <div className="flex items-center gap-1 p-0.5 rounded-lg bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.06)] w-20 h-5">
            <div className="h-full flex-1 rounded bg-[rgba(var(--accent),0.2)] animate-pulse" />
            <div className="h-full flex-1 rounded bg-transparent" />
          </div>
        </div>

        {/* ── 2. Compact Tab / Filter Row ── */}
        <div className="flex items-center gap-1.5 w-full">
          {[1, 2, 3].map((i) => (
            <div
              key={i}
              className={cn(
                "flex-1 h-5 rounded-lg flex items-center justify-center border",
                i === 1
                  ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.25)]"
                  : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--border),0.04)]"
              )}
            >
              <div className="w-10 h-2 rounded bg-[rgba(var(--foreground),0.08)] animate-pulse" />
            </div>
          ))}
        </div>

        {/* ── 3. Compact 2-Column Grid ── */}
        <div className="grid grid-cols-2 gap-2 w-full">
          {[1, 2].map((i) => (
            <div
              key={i}
              className="p-2.5 rounded-xl border border-[rgba(var(--border),0.06)] bg-[rgba(var(--foreground),0.015)] flex flex-col gap-1.5 min-h-[64px]"
            >
              <div className="flex items-center justify-between">
                <div className="w-16 h-2.5 rounded bg-[rgba(var(--foreground),0.1)] animate-pulse" />
                <div className="w-6 h-2 rounded bg-[rgba(var(--accent),0.12)] animate-pulse" />
              </div>
              <div className="w-full h-2 rounded bg-[rgba(var(--foreground),0.05)] animate-pulse" />
              <div className="w-3/4 h-2 rounded bg-[rgba(var(--foreground),0.04)] animate-pulse" />
            </div>
          ))}
        </div>

        {/* ── 4. Compact Control Row ── */}
        <div className="p-2.5 rounded-xl border border-[rgba(var(--border),0.06)] bg-[rgba(var(--foreground),0.015)] flex items-center justify-between">
          <div className="space-y-1">
            <div className="w-20 h-2.5 rounded bg-[rgba(var(--foreground),0.1)] animate-pulse" />
            <div className="w-28 h-1.5 rounded bg-[rgba(var(--foreground),0.04)] animate-pulse" />
          </div>
          <div className="w-8 h-4 rounded-full bg-[rgba(var(--accent),0.15)] animate-pulse" />
        </div>
      </div>
    </div>
  );
});

SettingsCardSkeleton.displayName = "SettingsCardSkeleton";
