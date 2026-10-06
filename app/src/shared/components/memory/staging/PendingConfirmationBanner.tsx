import React, { memo, useCallback } from "react";
import { Clock, AlertCircle, Zap, CheckCircle2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { useSettingsStore } from "@/store/settingsStore";
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
    const pipelineProcessingEnabled = useSettingsStore(
      (s) => s.settings?.personal_memory?.pipeline_processing_enabled ?? true
    );
    const updateDraft = useSettingsStore((s) => s.updateDraft);
    const commitChanges = useSettingsStore((s) => s.commitChanges);

    const handleEnableAndWait = useCallback(async () => {
      updateDraft("personal_memory", "pipeline_processing_enabled", true);
      await commitChanges();
      onDismiss();
    }, [updateDraft, commitChanges, onDismiss]);

    const title = isCompaction
      ? MEMORY_COPY.compactionRunningTitle
      : pipelineProcessingEnabled
      ? MEMORY_COPY.pendingConfirmationTitle
      : MEMORY_COPY.pendingConfirmationTitleDisabled;

    const desc = isCompaction
      ? MEMORY_COPY.compactionRunningDesc
      : pipelineProcessingEnabled
      ? MEMORY_COPY.pendingConfirmationDescIngestionOn(confirmation.pendingCount)
      : MEMORY_COPY.pendingConfirmationDescIngestionOff(confirmation.pendingCount);

    return (
      <div className="flex-1 min-h-0 flex flex-col items-center justify-center p-6 text-center animate-in fade-in duration-300">
        <div
          className={cn(
            "w-12 h-12 rounded-2xl flex items-center justify-center mb-3.5 shadow-sm",
            isCompaction
              ? "bg-amber-500/15 border border-amber-500/30 text-amber-400"
              : "bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))]"
          )}
        >
          {isCompaction ? <AlertCircle size={22} /> : <Clock size={22} />}
        </div>

        <h3 className="text-[14px] font-semibold tracking-wide text-[rgb(var(--foreground))] mb-1.5 max-w-sm">
          {title}
        </h3>
        <p className="text-[12px] text-[rgb(var(--foreground-muted))] max-w-md mb-6 leading-relaxed font-sans">
          {desc}
        </p>

        <div className="flex items-center justify-center gap-3 flex-wrap max-w-sm w-full">
          {isCompaction ? (
            <button
              type="button"
              onClick={onDismiss}
              className="px-4 py-2 rounded-xl text-[12px] font-medium text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] border border-[rgba(var(--border),0.25)] transition-all cursor-pointer shadow-xs"
            >
              {MEMORY_COPY.dismiss}
            </button>
          ) : pipelineProcessingEnabled ? (
            <>
              <button
                type="button"
                onClick={onDismiss}
                className="flex items-center gap-1.5 px-4 py-2 rounded-xl text-[12px] font-semibold bg-[rgb(var(--accent))] text-black hover:opacity-90 active:scale-[0.98] transition-all shadow-md cursor-pointer"
              >
                <span>{MEMORY_COPY.waitForExtraction}</span>
              </button>
              <button
                type="button"
                disabled={isConsolidating}
                onClick={onConfirm}
                className="flex items-center gap-1.5 px-4 py-2 rounded-xl text-[12px] font-medium text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] border border-[rgba(var(--border),0.25)] transition-all cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <Zap size={13} className={cn(isConsolidating && "animate-pulse")} />
                <span>
                  {isConsolidating ? MEMORY_COPY.consolidating : MEMORY_COPY.proceedAnyway}
                </span>
              </button>
            </>
          ) : (
            <>
              <button
                type="button"
                onClick={handleEnableAndWait}
                className="flex items-center gap-1.5 px-4 py-2 rounded-xl text-[12px] font-semibold bg-[rgb(var(--accent))] text-black hover:opacity-90 active:scale-[0.98] transition-all shadow-md cursor-pointer"
              >
                <CheckCircle2 size={13} />
                <span>{MEMORY_COPY.enableAndWait}</span>
              </button>
              <button
                type="button"
                disabled={isConsolidating}
                onClick={onConfirm}
                className="flex items-center gap-1.5 px-4 py-2 rounded-xl text-[12px] font-medium text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] border border-[rgba(var(--border),0.25)] transition-all cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <Zap size={13} className={cn(isConsolidating && "animate-pulse")} />
                <span>
                  {isConsolidating ? MEMORY_COPY.consolidating : MEMORY_COPY.proceedAnyway}
                </span>
              </button>
            </>
          )}
        </div>
      </div>
    );
  }
);

PendingConfirmationBanner.displayName = "PendingConfirmationBanner";
