import { useMemo, useCallback, memo } from "react";
import { AlertCircle, Check, RefreshCw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore } from "@/store/settingsStore";
import { ErrorBoundary } from "@/shared/components/common";
import { AnimatePresence, motion } from "framer-motion";
import type { SettingsDomain as Domain } from "@/data/settingsCopy";
import { SETTINGS_COPY } from "@/data/settingsCopy";
import { HelpTriggerButton } from "@/shared/components/help/HelpTriggerButton";

export interface SettingsCardWrapperProps {
  domain: Domain;
  isActive: boolean;
  layoutMode: "full-max" | "full-min" | "small";
  children: React.ReactNode;
}

export const SettingsCardWrapper = memo(({ domain, isActive, layoutMode, children }: SettingsCardWrapperProps) => {
  const settings = useSettingsStore((s) => s.settings);
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const commitChanges = useSettingsStore((s) => s.commitChanges);

  const hasChanges = useSettingsStore(useCallback((s: any) => Boolean(s.isDomainDirty(domain.id)), [domain.id]));

  const requiresRestart = useMemo(() => {
    if (!settings || !draftSettings) return false;
    if (domain.id === "models") {
      const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
      if (isRealtime) return false;
      return (
        settings.vad.vad_backend !== draftSettings.vad.vad_backend ||
        settings.stt.active !== draftSettings.stt.active ||
        settings.stt.embedded.model !== draftSettings.stt.embedded.model ||
        settings.llm.active !== draftSettings.llm.active ||
        settings.llm.context_window !== draftSettings.llm.context_window ||
        settings.llm.threads !== draftSettings.llm.threads ||
        settings.tts.active !== draftSettings.tts.active
      );
    }
    return false;
  }, [domain.id, settings, draftSettings]);

  const isCloudLlmMissingKey =
    draftSettings?.llm?.active === "cloud" &&
    !draftSettings?.llm?.cloud?.api_key?.trim();
  // TODO: re-enable when STT cloud config desk exists (LlmConfigDesk.tsx placeholder at :364).
  // Selecting "cloud" STT currently puts the user in an unconfigurable dead-end with no
  // inputs to supply a key, making this banner unresolvable. Suppress until the desk is built.
  const isCloudSttMissingKey = false;
  const isRealtimeMissingKey =
    draftSettings?.interaction?.pipeline_mode === "realtime" &&
    ((draftSettings?.realtime?.active === "gemini_live" && !(draftSettings?.realtime?.gemini_live?.api_key || (draftSettings?.realtime as any)?.gemini?.api_key)?.trim()) ||
     (draftSettings?.realtime?.active === "deepgram_voice_agent" && !(draftSettings?.realtime?.deepgram_voice_agent?.api_key || (draftSettings?.realtime as any)?.deepgram?.api_key)?.trim()));
  const isMissingCloudKey = isCloudLlmMissingKey || isCloudSttMissingKey || isRealtimeMissingKey;

  const handleSave = () => {
    if (isMissingCloudKey) return;
    commitChanges();
  };

  const isAutoSavedHere = useSettingsStore((s) => s.autoSavedDomain === domain.id);

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
            {/* Per-card Help trigger (desktop layouts only) */}
            {(layoutMode === "full-max" || layoutMode === "full-min") && (
              <div className="flex justify-end pr-1 -mb-1">
                <HelpTriggerButton
                  deepLink={`settings:${domain.id}`}
                  size="sm"
                  label={`Help: ${domain.label}`}
                />
              </div>
            )}

            {/* Actual Card content */}
            <ErrorBoundary name={`Settings:${domain.id}`}>
              {children}
            </ErrorBoundary>

            {/* ─── Repurposed Dynamic Footer: Auto-Save Confirmation OR Heavy Restart Action Bar ─── */}
            {(layoutMode === "full-max" || layoutMode === "full-min") && (
              <AnimatePresence>
                {/* Mode A: Explicit Restart Required Bar (ONLY for Type 3 Restart or Missing Cloud Key) */}
                {hasChanges && (requiresRestart || isMissingCloudKey) && (
                  <motion.div
                    key="restart-footer"
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: "auto" }}
                    exit={{ opacity: 0, height: 0 }}
                    transition={{ duration: 0.2 }}
                    className="w-full p-3 px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex items-center justify-between overflow-hidden text-[12px]"
                  >
                    {isMissingCloudKey ? (
                      <>
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
                      </>
                    ) : (
                      <>
                        <span className="font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-1.5">
                          <RefreshCw size={14} /> {requiresRestart ? "Pipeline Restart Required" : SETTINGS_COPY.unsavedChanges}
                        </span>
                        <div className="flex gap-2">
                          <button
                            onClick={handleSave}
                            className="px-3.5 py-1 rounded-lg bg-[rgb(var(--accent))] text-black dark:text-white font-black text-[12px] uppercase tracking-wider hover:brightness-110 active:scale-95 transition-all cursor-pointer shadow-md flex items-center gap-1.5"
                          >
                            <span>{requiresRestart ? "Apply & Reload" : SETTINGS_COPY.saveChanges}</span>
                          </button>
                          <button
                            onClick={() => useSettingsStore.getState().discardDomainChanges(domain.id)}
                            className="px-3 py-1 rounded-lg bg-transparent text-[rgb(var(--foreground-muted))] hover:text-rose-400 hover:bg-rose-500/10 border border-transparent hover:border-rose-500/20 text-[12px] font-bold uppercase tracking-wider transition-all cursor-pointer"
                          >
                            {SETTINGS_COPY.discardChanges}
                          </button>
                        </div>
                      </>
                    )}
                  </motion.div>
                )}

                {/* Mode B: Debounced "Changes Saved" Auto-Toast (Only on the specific modified card, using Primary Accent) */}
                {!hasChanges && isAutoSavedHere && (
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
              </AnimatePresence>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
});

SettingsCardWrapper.displayName = "SettingsCardWrapper";
