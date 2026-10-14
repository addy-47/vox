import { memo, useCallback, useMemo } from "react";
import { AlertCircle, Check, RefreshCw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore, type SettingsState, SETTINGS_DOMAIN_TO_UI } from "@/store/settingsStore";
import { SETTINGS_COPY } from "@/data/settingsCopy";
import { useExpandableList } from "@/shared/ui/ExpandableList";
import { useViewportResize } from "@/shared/hooks/useViewportResize";
import { Tooltip } from "@/shared/ui/Tooltip";

export type DomainCommitMode =
  | "missing-key"
  | "restart"
  | "saved"
  | "restarting"
  | "failed";

export interface DomainCommitState {
  mode: DomainCommitMode | null;
  hasChanges: boolean;
  failedKeys: string[];
}

/**
 * Single source of commit-button logic for a settings domain, shared by the
 * card footer (SettingsCardWrapper) and expanded-modal headers. Mode
 * precedence mirrors the card footer exactly: dirty states first, then
 * saved toast, restarting indicator, and save failure.
 */
export function useDomainCommitState(domainId: string): DomainCommitState {
  // Atomic leaf selectors, not `(s) => s.draftSettings`. The whole-object
  // subscription re-rendered every mounted card wrapper on every keystroke in
  // any domain, and the derived dirtiness below ran two `JSON.stringify` walks
  // per store write per wrapper (~1,100 calls). `domainFlags` is recomputed
  // once per mutation, so these are now O(1) lookups.
  const llmActive = useSettingsStore((s) => s.draftSettings?.llm?.active);
  const cloudApiKey = useSettingsStore((s) => s.draftSettings?.llm?.cloud?.api_key);
  const pipelineMode = useSettingsStore((s) => s.draftSettings?.interaction?.pipeline_mode);
  const realtimeActive = useSettingsStore((s) => s.draftSettings?.realtime?.active);
  const geminiApiKey = useSettingsStore(
    (s) => s.draftSettings?.realtime?.gemini_live?.api_key
  );
  const deepgramApiKey = useSettingsStore(
    (s) => s.draftSettings?.realtime?.deepgram_voice_agent?.api_key
  );

  const hasChanges = useSettingsStore((s) => Boolean(s.domainFlags[domainId]?.dirty));
  const requiresRestart = useSettingsStore(
    (s) => Boolean(s.domainFlags[domainId]?.requiresRestart)
  );

  const isCloudLlmMissingKey =
    llmActive === "cloud" && !cloudApiKey?.trim();
  const isRealtimeMissingKey =
    pipelineMode === "realtime" &&
    ((realtimeActive === "gemini_live" && !geminiApiKey?.trim()) ||
     (realtimeActive === "deepgram_voice_agent" && !deepgramApiKey?.trim()));

  const isMissingKey = useMemo(() => {
    if (domainId === "models" || domainId === "interaction") {
      return pipelineMode === "realtime" ? isRealtimeMissingKey : isCloudLlmMissingKey;
    }
    return false;
  }, [domainId, pipelineMode, isRealtimeMissingKey, isCloudLlmMissingKey]);

  const autoSaved = useSettingsStore((s) => s.autoSavedDomain === domainId);
  const saveFailure = useSettingsStore((s) => s.failedSaveDomains[domainId]);
  const failedKeys = useSettingsStore((s) => s.failedSaveDomains[domainId]);
  const isRestarting = useSettingsStore(
    useCallback(
      (s: SettingsState) => {
        if (!s.restartInFlight) return false;
        if (s.restartKeys.length === 0) return domainId === "models";
        return s.restartKeys.some((k) => {
          const scope = k.split(".")[0];
          const targetUi = SETTINGS_DOMAIN_TO_UI[scope] || "models";
          return targetUi === domainId;
        });
      },
      [domainId]
    )
  );

  // Hot-save changes stay uncommitted only during the 600ms autosave window.
  // There is intentionally no routine Save/Cancel UI for them: they resolve to
  // the saved toast, while restart-class changes keep explicit controls.
  let mode: DomainCommitMode | null = null;
  if (hasChanges && isMissingKey) {
    mode = "missing-key";
  } else if (hasChanges && requiresRestart) {
    mode = "restart";
  } else if (!saveFailure && autoSaved) {
    mode = "saved";
  } else if (isRestarting) {
    mode = "restarting";
  } else if (saveFailure) {
    mode = "failed";
  }

  return { mode, hasChanges, failedKeys: failedKeys ?? [] };
}

export interface SettingsCommitControlsProps {
  domainId: string;
  layoutMode?: "full-max" | "full-min" | "small";
}

/**
 * Compact commit controls for modal headers: save / discard / apply-restart
 * with the exact card-footer logic, plus saved / restarting / failed states.
 * Renders nothing when the domain is clean and idle.
 */
export const SettingsCommitControls = memo(({ domainId, layoutMode }: SettingsCommitControlsProps) => {
  const { mode } = useDomainCommitState(domainId);
  const { isExpanded, close } = useExpandableList();
  const { isMobile, isCompact } = useViewportResize();

  if (mode === null) return null;

  if (mode === "saved") {
    return (
      <span className="flex items-center gap-1.5 text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] shrink-0">
        <Check size={13} className="shrink-0" />
        <span className="hidden sm:inline">{SETTINGS_COPY.changesSaved}</span>
      </span>
    );
  }

  if (mode === "restarting") {
    return (
      <span role="status" className="flex items-center gap-1.5 text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] shrink-0">
        <RefreshCw size={13} className="animate-spin shrink-0" />
        <span className="hidden sm:inline">{SETTINGS_COPY.restartingEngine}</span>
      </span>
    );
  }

  if (mode === "failed") {
    return (
      <span role="alert" className="flex items-center gap-1.5 text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--danger))] shrink-0">
        <AlertCircle size={13} className="shrink-0" />
        <span className="hidden sm:inline">{SETTINGS_COPY.saveFailedTitle}</span>
      </span>
    );
  }

  if (mode === "missing-key") {
    return (
      <div className="flex items-center gap-1.5 shrink-0">
        <span className="flex items-center gap-1 text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--danger))] shrink-0">
          <AlertCircle size={13} className="shrink-0" />
          <span className="hidden md:inline">{SETTINGS_COPY.apiKeyRequired}</span>
        </span>
        <button
          type="button"
          disabled
          className="px-2.5 py-1 rounded-lg font-bold text-[11px] uppercase tracking-wider bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]/40 cursor-not-allowed border border-[rgba(var(--border),0.1)] shrink-0"
        >
          {SETTINGS_COPY.saveChanges}
        </button>
        {!isExpanded && (
          <button
            type="button"
            onClick={() => useSettingsStore.getState().discardDomainChanges(domainId)}
            className="px-2.5 py-1 rounded-lg bg-transparent text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.1)] border border-transparent hover:border-[rgba(var(--danger),0.2)] text-[11px] font-bold uppercase tracking-wider transition-all cursor-pointer shrink-0"
          >
            {SETTINGS_COPY.discardChanges}
          </button>
        )}
      </div>
    );
  }

  const isSmall = layoutMode === "small" || isMobile || isCompact;
  const showLabel = !isSmall;

  return (
    <div className="flex items-center justify-between gap-2 shrink-0">
      <Tooltip label={SETTINGS_COPY.applyAndRestart} side="bottom">
        <button
          type="button"
          onClick={async () => {
            await useSettingsStore.getState().commitChanges();
            if (isExpanded) close();
          }}
          aria-label={SETTINGS_COPY.applyAndRestart}
          className={cn(
            "px-2 sm:px-2.5 py-1 rounded-lg border font-bold text-[11px] uppercase tracking-wider transition-all cursor-pointer shrink-0",
            "flex items-center gap-1.5 bg-transparent text-[rgb(var(--accent))] border-[rgba(var(--accent),0.45)]",
            "hover:bg-[rgb(var(--accent))] hover:text-[rgb(var(--accent-foreground))] hover:border-[rgb(var(--accent))]"
          )}
        >
          <RefreshCw size={11} className="shrink-0" />
          {showLabel && <span>{SETTINGS_COPY.applyAndRestart}</span>}
        </button>
      </Tooltip>
      {!isExpanded && (
        <button
          type="button"
          onClick={() => useSettingsStore.getState().discardDomainChanges(domainId)}
          className="px-2.5 py-1 rounded-lg bg-transparent text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.1)] border border-transparent hover:border-[rgba(var(--danger),0.2)] text-[11px] font-bold uppercase tracking-wider transition-all cursor-pointer shrink-0"
        >
          {SETTINGS_COPY.discardChanges}
        </button>
      )}
    </div>
  );
});

SettingsCommitControls.displayName = "SettingsCommitControls";
