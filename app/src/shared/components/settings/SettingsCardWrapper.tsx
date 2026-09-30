import { useMemo, useCallback, memo } from "react";
import { AlertCircle, Check, RefreshCw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore, type SettingsState } from "@/store/settingsStore";
import { ErrorBoundary } from "@/shared/components/common";
import { AnimatePresence, motion } from "framer-motion";
import type { SettingsDomain as Domain } from "@/data/settingsCopy";
import { SETTINGS_COPY } from "@/data/settingsCopy";


export interface SettingsCardWrapperProps {
  domain: Domain;
  isActive: boolean;
  layoutMode: "full-max" | "full-min" | "small";
  children: React.ReactNode;
}

export const SettingsCardWrapper = memo(({ domain, isActive, layoutMode, children }: SettingsCardWrapperProps) => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);

  const hasChanges = useSettingsStore(useCallback((s: SettingsState) => Boolean(s.isDomainDirty(domain.id)), [domain.id]));

  // Reload policy is owned by the backend (`config::get_setting_reload_policy`).
  // The verdict arrives on `update_setting` and the backend has already acted on
  // a `Restart` classification, so the card never decides whether to reload.
  const restartInFlight = useSettingsStore((s) => s.restartInFlight);

  const isCloudLlmMissingKey =
    draftSettings?.llm?.active === "cloud" &&
    !draftSettings?.llm?.cloud?.api_key?.trim();
  // TODO: re-enable when STT cloud config desk exists (LlmConfigDesk.tsx placeholder at :364).
  // Selecting "cloud" STT currently puts the user in an unconfigurable dead-end with no
  // inputs to supply a key, making this banner unresolvable. Suppress until the desk is built.
  const isCloudSttMissingKey = false;
  const isRealtimeMissingKey =
    draftSettings?.interaction?.pipeline_mode === "realtime" &&
    ((draftSettings?.realtime?.active === "gemini_live" && !(draftSettings?.realtime?.gemini_live?.api_key)?.trim()) ||
     (draftSettings?.realtime?.active === "deepgram_voice_agent" && !(draftSettings?.realtime?.deepgram_voice_agent?.api_key)?.trim()));

  const isDomainMissingCloudKey = useMemo(() => {
    if (domain.id === "models") {
      const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
      return isRealtime ? isRealtimeMissingKey : (isCloudLlmMissingKey || isCloudSttMissingKey);
    }
    if (domain.id === "interaction") {
      const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
      return isRealtime ? isRealtimeMissingKey : false;
    }
    return false;
  }, [domain.id, draftSettings?.interaction?.pipeline_mode, isRealtimeMissingKey, isCloudLlmMissingKey, isCloudSttMissingKey]);

  const isAutoSavedHere = useSettingsStore((s) => s.autoSavedDomain === domain.id);
  const saveFailure = useSettingsStore((s) => s.failedSaveDomains[domain.id]);
  const failedKeys = useSettingsStore((s) => s.failedSaveKeys);

  return (
    <AnimatePresence>
      {isActive && (
        <motion.div
          initial={{ opacity: 0, scale: 0.96 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.96 }}
          transition={{ duration: 0.28, ease: [0.16, 1, 0.3, 1] }}
          className="w-full h-full flex items-center justify-center pointer-events-auto"
        >
          <div
            id={`card-${domain.id}`}
            className={cn(
              "shrink-0 flex flex-col gap-0",
              hasChanges && "has-unsaved-changes"
            )}
          >
            {/* Actual Card content */}
            <ErrorBoundary name={`Settings:${domain.id}`}>
              {children}
            </ErrorBoundary>

            {/* ─── Dynamic Footer: one of four mutually exclusive states ─── */}
            {(layoutMode === "full-max" || layoutMode === "full-min") && (
              <AnimatePresence>
                {/* Mode A: A required credential is missing, so the commit cannot
                    succeed. Save stays disabled; only Discard is actionable. */}
                {hasChanges && isDomainMissingCloudKey && (
                  <motion.div
                    key="missing-key-footer"
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: "auto" }}
                    exit={{ opacity: 0, height: 0 }}
                    transition={{ duration: 0.2 }}
                    className="w-full p-3 px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex items-center justify-between overflow-hidden text-[12px]"
                  >
                    <span className="font-bold uppercase tracking-wider text-rose-400 flex items-center gap-1.5">
                      <AlertCircle size={14} /> {SETTINGS_COPY.apiKeyRequired}
                    </span>
                    <div className="flex gap-2">
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

                {/* Mode A2: The backend classified a committed key as `Restart`
                    and is rebuilding the engine. Purely informational: there is
                    no action to take and no decision for the UI to make. */}
                {!hasChanges && !saveFailure && restartInFlight && (
                  <motion.div
                    key="restarting-footer"
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: "auto" }}
                    exit={{ opacity: 0, height: 0 }}
                    transition={{ duration: 0.2 }}
                    className="w-full py-2 px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex items-center justify-between overflow-hidden text-[12px]"
                  >
                    <span className="font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-1.5">
                      <RefreshCw size={14} className="animate-spin" /> {SETTINGS_COPY.restartingEngine}
                    </span>
                    <span className="text-[11px] text-[rgb(var(--accent))]/70 font-mono">{SETTINGS_COPY.applyingProviderChanges}</span>
                  </motion.div>
                )}

                {/* Mode B: Debounced "Changes Saved" Auto-Toast (Only on the specific modified card, using Primary Accent) */}
                {!hasChanges && !saveFailure && !restartInFlight && isAutoSavedHere && (
                  <motion.div
                    key="saved-toast-footer"
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: "auto" }}
                    exit={{ opacity: 0, height: 0 }}
                    transition={{ duration: 0.2 }}
                    className="w-full py-2 px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex items-center justify-between overflow-hidden text-[12px]"
                  >
                    <span className="font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-1.5">
                      <Check size={14} /> {SETTINGS_COPY.changesSaved}
                    </span>
                    <span className="text-[11px] text-[rgb(var(--accent))]/70 font-mono">{SETTINGS_COPY.autoSynced}</span>
                  </motion.div>
                )}

                {/* Mode C: Backend rejected the write. Without this the card
                    showed a green "Saved" tick for a value that was dropped. */}
                {!hasChanges && saveFailure && (
                  <motion.div
                    key="save-failed-footer"
                    role="alert"
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: "auto" }}
                    exit={{ opacity: 0, height: 0 }}
                    transition={{ duration: 0.2 }}
                    className="w-full py-2 px-5 rounded-b-[1.25rem] rounded-t-none bg-rose-500/10 border border-t-0 border-rose-500/25 flex items-center justify-between gap-4 overflow-hidden text-[12px]"
                  >
                    <span className="font-bold uppercase tracking-wider text-rose-400 flex items-center gap-1.5 shrink-0">
                      <AlertCircle size={14} /> {SETTINGS_COPY.saveFailedTitle}
                    </span>
                    <span className="text-[11px] text-rose-300/80 font-mono truncate">
                      {failedKeys.length > 0
                        ? `${SETTINGS_COPY.saveFailedHint} ${failedKeys.join(", ")}`
                        : SETTINGS_COPY.saveFailedNone}
                    </span>
                  </motion.div>
                )}
              </AnimatePresence>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
});

SettingsCardWrapper.displayName = "SettingsCardWrapper";
