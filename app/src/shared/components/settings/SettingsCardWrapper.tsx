import { memo } from "react";
import { AlertCircle, Check, RefreshCw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore } from "@/store/settingsStore";
import { useExpandedModalStore } from "@/store/expandedModalStore";
import { useDomainCommitState } from "./SettingsCommitControls";
import { ErrorBoundary } from "@/shared/components/common";
import { AnimatePresence, motion } from "framer-motion";
import type { SettingsDomain as Domain } from "@/data/settingsCopy";
import { SETTINGS_COPY } from "@/data/settingsCopy";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";


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
  const requiresRestart = mode === "restart";
  const isAutoSavedHere = mode === "saved";
  const isRestartHere = mode === "restarting";
  const saveFailure = mode === "failed";
  const failedKeys = useSettingsStore((s) => s.failedSaveDomains[domain.id]);
  const expandModalOpen = useExpandedModalStore((s) => s.openCount > 0);

  return (
    <AnimatePresence>
      {isActive && (
        <motion.div
          initial={{ opacity: 0, scale: 0.96 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.96 }}
          transition={{ duration: 0.28, ease: [0.16, 1, 0.3, 1] }}
          className={cn(
            "w-full pointer-events-auto",
            layoutMode === "small" ? "h-auto" : "h-full flex items-center justify-center"
          )}
        >
          <div
            id={`card-${domain.id}`}
            className={cn(
              "flex flex-col gap-0",
              layoutMode === "small" ? "w-full" : "shrink-0",
              (hasChanges || isAutoSavedHere || isRestartHere || saveFailure) && "has-unsaved-changes"
            )}
          >
            {/* Actual Card content */}
            <ErrorBoundary name={`Settings:${domain.id}`}>
              {children}
            </ErrorBoundary>

            {/* ─── Dynamic Footer: one of five mutually exclusive states ─── */}
            <AnimatePresence>
              {/* Mode A: A required credential is missing, so the commit cannot
                  succeed. Save stays disabled; only Discard is actionable. */}
              {!expandModalOpen && hasChanges && isMissingKey && (
                <motion.div
                  key="missing-key-footer"
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.2 }}
                  className="w-full p-2.5 sm:p-3 px-4 sm:px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-2.5 sm:gap-2 overflow-hidden text-[12px]"
                >
                  <span className="font-bold uppercase tracking-wider text-rose-400 flex items-center gap-1.5 shrink-0">
                    <AlertCircle size={14} className="shrink-0" />
                    <span className="truncate">{SETTINGS_COPY.apiKeyRequired}</span>
                  </span>
                  <div className="flex items-center gap-2 self-end sm:self-auto shrink-0">
                    <button
                      disabled
                      className="px-3.5 py-1 rounded-lg bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]/40 font-black text-[12px] uppercase tracking-wider cursor-not-allowed border border-[rgba(var(--border),0.1)]"
                    >
                      {SETTINGS_COPY.saveChanges}
                    </button>
                    <button
                      onClick={() => useSettingsStore.getState().discardDomainChanges(domain.id)}
                      className="px-3 py-1 rounded-lg bg-transparent text-[rgb(var(--foreground-muted))] hover:text-rose-400 hover:bg-rose-500/10 border border-transparent hover:border-rose-500/20 text-[12px] font-bold uppercase tracking-wider transition-all cursor-pointer"
                    >
                      {SETTINGS_COPY.discardChanges}
                    </button>
                  </div>
                </motion.div>
              )}

              {/* Mode B: Unsaved changes requiring engine restart -> Explicit "Apply & Restart" */}
              {!expandModalOpen && hasChanges && !isMissingKey && requiresRestart && (
                <motion.div
                  key="apply-restart-footer"
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.2 }}
                  className="w-full p-2.5 px-4 sm:px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-2.5 sm:gap-2 overflow-hidden text-[12px]"
                >
                  <div className="flex items-center gap-2 self-start sm:self-auto shrink-0">
                    <button
                      onClick={() => useSettingsStore.getState().commitChanges()}
                      className="px-3 py-1 rounded-lg border font-bold text-[12px] uppercase tracking-wider flex items-center gap-1.5 transition-all cursor-pointer bg-transparent text-[rgb(var(--accent))] border-[rgba(var(--accent),0.45)] hover:bg-[rgb(var(--accent))] hover:text-[rgb(var(--accent-foreground))] hover:border-[rgb(var(--accent))]"
                    >
                      <RefreshCw size={12} className="shrink-0" />
                      <span>{SETTINGS_COPY.applyAndRestart}</span>
                    </button>
                  </div>
                  <div className="flex items-center gap-2 self-end sm:self-auto shrink-0">
                    <button
                      onClick={() => useSettingsStore.getState().discardDomainChanges(domain.id)}
                      className="px-3 py-1 rounded-lg bg-transparent text-[rgb(var(--foreground-muted))] hover:text-rose-400 hover:bg-rose-500/10 border border-transparent hover:border-rose-500/20 text-[12px] font-bold uppercase tracking-wider transition-all cursor-pointer"
                    >
                      {SETTINGS_COPY.discardChanges}
                    </button>
                  </div>
                </motion.div>
              )}

              {/* Routine hot-save changes have no footer: the autosave toast follows. */}

              {/* Mode D: Debounced "Changes Saved" Auto-Toast (Only on the specific modified card) */}
              {!expandModalOpen && !hasChanges && !saveFailure && isAutoSavedHere && (
                <motion.div
                  key="saved-toast-footer"
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.2 }}
                  className="w-full py-2 px-4 sm:px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex items-center justify-between overflow-hidden text-[12px]"
                >
                  <span className="font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-1.5 truncate">
                    <Check size={14} className="shrink-0" />
                    <span className="truncate">{SETTINGS_COPY.changesSaved}</span>
                  </span>
                  <span className="text-[11px] text-[rgb(var(--accent))]/70 font-mono shrink-0 ml-2">{SETTINGS_COPY.autoSynced}</span>
                </motion.div>
              )}

              {/* Mode E: the backend is rebuilding the engine right now */}
              {!expandModalOpen && !hasChanges && isRestartHere && (
                <motion.div
                  key="restarting-footer"
                  role="status"
                  aria-live="polite"
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.2 }}
                  className="w-full py-2 px-4 sm:px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex items-center gap-2 overflow-hidden text-[12px]"
                >
                  <RefreshCw size={13} className="animate-spin shrink-0 text-[rgb(var(--accent))]" />
                  <span className="font-bold uppercase tracking-wider text-[rgb(var(--accent))] truncate">
                    {SETTINGS_COPY.restartingEngine}
                  </span>
                </motion.div>
              )}

              {/* Mode F: Backend rejected the write */}
              {!expandModalOpen && !hasChanges && saveFailure && (
                <motion.div
                  key="save-failed-footer"
                  role="alert"
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.2 }}
                  className="w-full py-2 px-4 sm:px-5 rounded-b-[1.25rem] rounded-t-none bg-rose-500/10 border border-t-0 border-rose-500/25 flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-1.5 sm:gap-4 overflow-hidden text-[12px]"
                >
                  <span className="font-bold uppercase tracking-wider text-rose-400 flex items-center gap-1.5 shrink-0">
                    <AlertCircle size={14} className="shrink-0" /> {SETTINGS_COPY.saveFailedTitle}
                  </span>
                  <span className="text-[11px] text-rose-300/80 font-mono truncate">
                    {failedKeys.length > 0
                      ? `${SETTINGS_COPY.saveFailedHint} ${failedKeys.join(", ")}`
                      : SETTINGS_COPY.saveFailedNone}
                  </span>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
});

SettingsCardWrapper.displayName = "SettingsCardWrapper";
