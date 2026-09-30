import React, { memo } from "react";
import { Clock, AlertCircle, Zap, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";
import type { PendingConfirmation } from "../stagingTypes";

export interface PendingConfirmationBannerProps {
  confirmation: PendingConfirmation;
  isConsolidating?: boolean;
  onConfirm: () => void;
  onDismiss: () => void;
}

export const PendingConfirmationBanner: React.FC<PendingConfirmationBannerProps> = memo(
  ({ confirmation, isConsolidating, onConfirm, onDismiss }) => {
    const isCompaction = confirmation.reason === "compaction_in_progress";

    const title = isCompaction
      ? MEMORY_COPY.compactionRunningTitle
      : MEMORY_COPY.pendingConfirmationTitle;

    const desc = isCompaction
      ? MEMORY_COPY.compactionRunningDesc
      : MEMORY_COPY.pendingConfirmationDesc.replace(
          "{count}",
          confirmation.pendingCount.toString()
        );

    return (
      <div
        className={cn(
          "w-full rounded-xl p-4 my-2.5 transition-all duration-200 shadow-lg animate-in fade-in slide-in-from-top-2",
          isCompaction
            ? "border border-amber-500/35 bg-amber-500/10"
            : "border border-[rgba(var(--accent),0.35)] bg-[rgba(var(--accent),0.07)]"
        )}
      >
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-start gap-3 flex-1 min-w-0">
            <div
              className={cn(
                "w-8 h-8 rounded-lg flex items-center justify-center shrink-0 mt-0.5",
                isCompaction
                  ? "bg-amber-500/20 text-amber-400 border border-amber-500/30"
                  : "bg-[rgba(var(--accent),0.18)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.35)]"
              )}
            >
              {isCompaction ? <AlertCircle size={16} /> : <Clock size={16} />}
            </div>

            <div className="flex-1 min-w-0">
              <h4 className="text-[13px] font-semibold text-[rgb(var(--foreground))] tracking-tight mb-1">
                {title}
              </h4>
              <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed mb-3">
                {desc}
              </p>

              <div className="flex items-center gap-2.5 flex-wrap">
                {!isCompaction && (
                  <button
                    type="button"
                    disabled={isConsolidating}
                    onClick={onConfirm}
                    className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl text-[12px] font-semibold bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:opacity-90 active:scale-[0.98] transition-all shadow-md cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    <Zap
                      size={13}
                      className={cn(isConsolidating && "animate-pulse")}
                    />
                    <span>
                      {isConsolidating
                        ? MEMORY_COPY.consolidating
                        : MEMORY_COPY.integrateExistingNow}
                    </span>
                  </button>
                )}

                <button
                  type="button"
                  onClick={onDismiss}
                  className="px-3 py-1.5 rounded-xl text-[12px] font-medium text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] border border-[rgba(var(--border),0.18)] transition-all cursor-pointer"
                >
                  {isCompaction
                    ? MEMORY_COPY.dismiss
                    : MEMORY_COPY.waitForBackground}
                </button>
              </div>
            </div>
          </div>

          <button
            type="button"
            onClick={onDismiss}
            className="p-1.5 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer shrink-0"
            aria-label="Dismiss"
          >
            <X size={14} />
          </button>
        </div>
      </div>
    );
  }
);

PendingConfirmationBanner.displayName = "PendingConfirmationBanner";
