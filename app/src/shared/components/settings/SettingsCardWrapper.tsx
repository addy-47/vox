import { memo, useEffect } from "react";
import { AlertCircle, Check, RefreshCw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore } from "@/store/settingsStore";
import { useExpandedModalStore } from "@/store/expandedModalStore";
import { useDomainCommitState, SettingsFooterBanner } from "./SettingsCommitControls";
import { ErrorBoundary } from "@/shared/components/common";
import { AnimatePresence, motion } from "framer-motion";
import type { SettingsDomain as Domain } from "@/data/settingsCopy";
import { SETTINGS_COPY } from "@/data/settingsCopy";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";
import { getDomainCardWidth } from "./shared/settingsLayout";
import { rca, rcaRect } from "@/shared/lib/rcaTrace";


export interface SettingsCardWrapperProps {
  domain: Domain;
  isActive: boolean;
  layoutMode: "full-max" | "full-min" | "small";
  children: React.ReactNode;
}

export const SettingsCardWrapper = memo(({ domain, isActive, layoutMode, children }: SettingsCardWrapperProps) => {
  useMemoryTrace(`SettingsCard (${domain.id})`);
  const { mode, hasChanges } = useDomainCommitState(domain.id);

  const isMissingKey = mode === "missing-key";
  const isMissingModel = mode === "missing-model";
  const requiresRestart = mode === "restart";
  const isAutoSavedHere = mode === "saved";
  const isRestartHere = mode === "restarting";
  const saveFailure = mode === "failed";
  const failedKeys = useSettingsStore((s) => s.failedSaveDomains[domain.id]) ?? [];
  const deskFooter = useSettingsStore((s) => s.deskFooter);
  const expandModalOpen = useExpandedModalStore((s) => s.openCount > 0);
  const showDeskFooter = !expandModalOpen && !hasChanges && deskFooter?.domain === domain.id;

  // RCA: the footer changes the card block height, and the wrapper centers that
  // block vertically. Log when the mode flips so a layout jump can be tied to
  // the exact transition.
  useEffect(() => {
    rca("card-footer", "mode", {
      domain: domain.id,
      mode,
      hasChanges,
      card: rcaRect(document.getElementById(`card-${domain.id}`)),
    });
  }, [domain.id, mode, hasChanges]);

  return (
    <AnimatePresence>
      {isActive && (
        <motion.div
          initial={layoutMode === "small" ? { opacity: 0 } : { opacity: 0, scale: 0.96 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={layoutMode === "small" ? { opacity: 0, transition: { duration: 0 } } : { opacity: 0, scale: 0.96, transition: { duration: 0 } }}
          transition={{ duration: layoutMode === "small" ? 0.15 : 0.28, ease: [0.16, 1, 0.3, 1] }}
          className={cn(
            "w-full pointer-events-auto",
            layoutMode === "small"
              ? "h-auto"
              : domain.id === "persona"
              ? "h-full flex items-start justify-center overflow-visible"
              : domain.id === "appearance"
              ? "h-full flex items-start justify-center"
              : "h-full flex items-center justify-center"
          )}
        >
          <div
            id={`card-${domain.id}`}
            className={cn(
              // The footer must share the card's width exactly. Do not cap this
              // container (`max-w-full`) while the card keeps its own fixed
              // width: at carousel widths the cap would shrink the footer while
              // the card still overflows, producing a narrow strip under a wide
              // card. Small mode is fluid, so it caps both together.
              "flex flex-col gap-0",
              layoutMode === "small" ? "w-full max-w-full" : cn("shrink-0", getDomainCardWidth(domain.id, layoutMode)),
              layoutMode !== "small" && (hasChanges || isAutoSavedHere || isRestartHere || saveFailure) && "has-unsaved-changes"
            )}
          >
            {/* Actual Card content */}
            <ErrorBoundary name={`Settings:${domain.id}`}>
              {children}
            </ErrorBoundary>

            {/* ─── Dynamic Footer: one of five mutually exclusive states ─── */}
            {/* Mode A: A required credential is missing, so the commit cannot
                succeed. Save stays disabled; only Discard is actionable. */}
            {!expandModalOpen && hasChanges && isMissingKey && (
              <div
                key="missing-key-footer"
                className="w-full p-2.5 sm:p-3 px-4 sm:px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex items-center justify-between gap-2 overflow-hidden text-[12px] animate-in fade-in duration-150"
              >
                <span className="font-bold uppercase tracking-wider text-[rgb(var(--danger))] flex items-center gap-1.5 min-w-0 flex-1">
                  <AlertCircle size={14} className="shrink-0" />
                  <span className="truncate">{SETTINGS_COPY.apiKeyRequired}</span>
                </span>
                <div className="flex items-center gap-2 shrink-0">
                  <button
                    disabled
                    className="px-3 py-1 rounded-lg bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]/40 font-black text-[12px] uppercase tracking-wider cursor-not-allowed border border-[rgba(var(--border),0.1)] shrink-0"
                  >
                    {SETTINGS_COPY.saveChanges}
                  </button>
                  <button
                    onClick={() => useSettingsStore.getState().discardDomainChanges(domain.id)}
                    className="px-3 py-1 rounded-lg bg-transparent text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.1)] border border-transparent hover:border-[rgba(var(--danger),0.2)] text-[12px] font-bold uppercase tracking-wider transition-all cursor-pointer shrink-0"
                  >
                    {SETTINGS_COPY.discardChanges}
                  </button>
                </div>
              </div>
            )}

            {/* Mode A2: A provider is selected but no model is. The commit would
                send an empty model id, so it is blocked; only Discard acts. */}
            {!expandModalOpen && hasChanges && isMissingModel && (
              <div
                key="missing-model-footer"
                className="w-full p-2.5 sm:p-3 px-4 sm:px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex items-center justify-between gap-2 overflow-hidden text-[12px] animate-in fade-in duration-150"
              >
                <span className="font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-1.5 min-w-0 flex-1">
                  <AlertCircle size={14} className="shrink-0" />
                  <span className="truncate">{SETTINGS_COPY.modelRequired}</span>
                </span>
                <div className="flex items-center gap-2 shrink-0">
                  <button
                    disabled
                    className="px-3 py-1 rounded-lg bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]/40 font-black text-[12px] uppercase tracking-wider cursor-not-allowed border border-[rgba(var(--border),0.1)] shrink-0"
                  >
                    {SETTINGS_COPY.applyAndRestart}
                  </button>
                  <button
                    onClick={() => useSettingsStore.getState().discardDomainChanges(domain.id)}
                    className="px-3 py-1 rounded-lg bg-transparent text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.1)] border border-transparent hover:border-[rgba(var(--danger),0.2)] text-[12px] font-bold uppercase tracking-wider transition-all cursor-pointer shrink-0"
                  >
                    {SETTINGS_COPY.discardChanges}
                  </button>
                </div>
              </div>
            )}

            {/* Mode B: Unsaved changes requiring engine restart -> Explicit "Apply & Restart" */}
            {!expandModalOpen && hasChanges && !isMissingKey && !isMissingModel && requiresRestart && (
              <div
                key="apply-restart-footer"
                className="w-full p-2.5 px-4 sm:px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex items-center justify-between gap-2 flex-wrap overflow-hidden text-[12px] animate-in fade-in duration-150"
              >
                <button
                  onClick={() => useSettingsStore.getState().commitChanges()}
                  className="px-3 py-1 rounded-lg border font-bold text-[12px] uppercase tracking-wider flex items-center gap-1.5 transition-all cursor-pointer bg-transparent text-[rgb(var(--accent))] border-[rgba(var(--accent),0.45)] hover:bg-[rgb(var(--accent))] hover:text-[rgb(var(--accent-foreground))] hover:border-[rgb(var(--accent))] shrink-0"
                >
                  <RefreshCw size={12} className="shrink-0" />
                  <span>{SETTINGS_COPY.applyAndRestart}</span>
                </button>
                <button
                  onClick={() => useSettingsStore.getState().discardDomainChanges(domain.id)}
                  className="px-3 py-1 rounded-lg bg-transparent text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.1)] border border-transparent hover:border-[rgba(var(--danger),0.2)] text-[12px] font-bold uppercase tracking-wider transition-all cursor-pointer shrink-0"
                >
                  {SETTINGS_COPY.discardChanges}
                </button>
              </div>
            )}

            {/* Routine hot-save changes have no footer: the autosave toast follows. */}

            {/* Mode D: Debounced "Changes Saved" Auto-Toast (Only on the specific modified card) */}
            {!expandModalOpen && !hasChanges && !saveFailure && isAutoSavedHere && (
              <div
                key="saved-toast-footer"
                className={cn(
                  "w-full py-2 px-4 sm:px-5 flex items-center justify-between overflow-hidden text-[12px] animate-in fade-in duration-150",
                  layoutMode === "small"
                    ? "rounded-xl mt-2 bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.2)]"
                    : "rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)]"
                )}
              >
                <span className="font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-1.5 truncate">
                  <Check size={14} className="shrink-0" />
                  <span className="truncate">{SETTINGS_COPY.changesSaved}</span>
                </span>
                <span className="text-[11px] text-[rgb(var(--accent))]/70 font-mono shrink-0 ml-2">{SETTINGS_COPY.autoSynced}</span>
              </div>
            )}

            {/* Mode E: the backend is rebuilding the engine right now */}
            {!expandModalOpen && !hasChanges && isRestartHere && (
              <div
                key="restarting-footer"
                role="status"
                aria-live="polite"
                className="w-full py-2 px-4 sm:px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex items-center gap-2 overflow-hidden text-[12px] animate-in fade-in duration-150"
              >
                <RefreshCw size={13} className="animate-spin shrink-0 text-[rgb(var(--accent))]" />
                <span className="font-bold uppercase tracking-wider text-[rgb(var(--accent))] truncate">
                  {SETTINGS_COPY.restartingEngine}
                </span>
              </div>
            )}

            {/* Mode F: Backend rejected the write */}
            {!expandModalOpen && !hasChanges && saveFailure && (
              <div
                key="save-failed-footer"
                role="alert"
                className="w-full max-w-full py-2 px-4 sm:px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--danger),0.1)] border border-t-0 border-[rgba(var(--danger),0.25)] flex flex-col sm:flex-row sm:items-start lg:items-center justify-between gap-1 sm:gap-4 overflow-hidden text-[12px] animate-in fade-in duration-150 box-border"
              >
                <span className="font-bold uppercase tracking-wider text-[rgb(var(--danger))] flex items-start gap-1.5 min-w-0 flex-1">
                  <AlertCircle size={14} className="shrink-0 mt-px" /> <span className="min-w-0 [overflow-wrap:anywhere] break-words">{SETTINGS_COPY.saveFailedTitle}</span>
                </span>
                <span className="text-[11px] text-[rgb(var(--danger))]/80 font-mono min-w-0 flex-1 [overflow-wrap:anywhere] break-all leading-snug sm:text-right">
                  {failedKeys.length > 0
                    ? `${SETTINGS_COPY.saveFailedHint} ${failedKeys.join(", ")}`
                    : SETTINGS_COPY.saveFailedNone}
                </span>
              </div>
            )}

            {/* Mode G: Transient desk error (provider health / model list) */}
            {showDeskFooter && deskFooter && (
              <SettingsFooterBanner
                key="desk-error-footer"
                title={deskFooter.title}
                detail={deskFooter.detail}
                className="rounded-b-[1.25rem] rounded-t-none border-t-0"
              />
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
});

SettingsCardWrapper.displayName = "SettingsCardWrapper";
