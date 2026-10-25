import { memo, useCallback, useMemo } from "react";
import { AlertCircle, Check, RefreshCw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore, type SettingsState, SETTINGS_DOMAIN_TO_UI } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { SETTINGS_COPY, PROVIDERS_CONFIG_DESK_COPY } from "@/data/settingsCopy";
import { useExpandableList } from "@/shared/ui/ExpandableList";
import { useViewportResize } from "@/shared/hooks/useViewportResize";
import { Tooltip } from "@/shared/ui/Tooltip";
import type { DomainFlags } from "@/types/settings";

export type DomainCommitMode =
  | "missing-key"
  | "missing-model"
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
 * True when a cloud/server LLM selection carries no model id. Switching
 * provider writes an empty model by design (a model id belongs to the provider
 * that serves it), so this state is normal and must block commit instead of
 * sending an empty model to the engine.
 */
export function isLlmSelectionMissingModel(
  selection: { category?: string; model?: string } | null | undefined
): boolean {
  if (!selection) return false;
  if (selection.category !== "cloud" && selection.category !== "server") return false;
  return !selection.model?.trim();
}

/**
 * Leaf selector hook for active LLM provider and cloud API key.
 * Gated by enabled flag so non-language cards skip subscribing to cloud keys.
 */
function useLlmCloudApiKey(enabled = true) {
  const llmCategory = useSettingsStore((s) => (enabled ? s.draftSettings?.llm.active.category : undefined));
  const llmActiveProvider = useSettingsStore((s) => (enabled ? s.draftSettings?.llm.active.provider : undefined));
  const llmActiveModel = useSettingsStore((s) => (enabled ? s.draftSettings?.llm.active.model : undefined));

  const draftLlmKey = useSettingsStore(
    useCallback((s: SettingsState) => (enabled && llmActiveProvider ? s.draftSettings?.llm.cloud_keys?.[llmActiveProvider] : undefined), [enabled, llmActiveProvider])
  );
  const draftProvKey = useSettingsStore(
    useCallback((s: SettingsState) => (enabled && llmActiveProvider ? s.draftSettings?.providers?.llm.cloud_keys?.[llmActiveProvider] : undefined), [enabled, llmActiveProvider])
  );
  const comProvKey = useSettingsStore(
    useCallback((s: SettingsState) => (enabled && llmActiveProvider ? s.settings?.providers?.llm.cloud_keys?.[llmActiveProvider] : undefined), [enabled, llmActiveProvider])
  );
  const comLlmKey = useSettingsStore(
    useCallback((s: SettingsState) => (enabled && llmActiveProvider ? s.settings?.llm.cloud_keys?.[llmActiveProvider] : undefined), [enabled, llmActiveProvider])
  );

  const cloudApiKey = draftLlmKey ?? draftProvKey ?? comProvKey ?? comLlmKey;

  return { llmCategory, llmActiveProvider, llmActiveModel, cloudApiKey };
}

/**
 * Single source of commit-button logic for a settings domain, shared by the
 * card footer (SettingsCardWrapper) and expanded-modal headers. Mode
 * precedence mirrors the card footer exactly: dirty states first, then
 * saved toast, restarting indicator, and save failure.
 */
export function useDomainCommitState(domainId: string): DomainCommitState {
  const isLanguageDomain = domainId === "language";
  const isSpeechOrInteraction = domainId === "speech" || domainId === "interaction";

  const { llmCategory, llmActiveModel, cloudApiKey } = useLlmCloudApiKey(isLanguageDomain);
  const pipelineMode = useSettingsStore((s) => (isSpeechOrInteraction ? s.draftSettings?.interaction.pipeline_mode : undefined));
  const realtimeProvider = useSettingsStore((s) => (isSpeechOrInteraction ? s.draftSettings?.realtime.active.provider : undefined));
  const realtimeKey = useSettingsStore(
    useCallback(
      (s: SettingsState) =>
        isSpeechOrInteraction && realtimeProvider
          ? (s.draftSettings?.realtime.api_keys?.[realtimeProvider] ??
             s.draftSettings?.providers?.realtime.api_keys?.[realtimeProvider])
          : undefined,
      [isSpeechOrInteraction, realtimeProvider]
    )
  );

  // `domainFlags` is empty until the first recompute, so a domain key can be
  // missing. The lookup is typed as possibly undefined on purpose.
  const hasChanges = useSettingsStore((s) => {
    const flag = s.domainFlags[domainId] as DomainFlags | undefined;
    return Boolean(flag?.dirty);
  });
  const requiresRestart = useSettingsStore((s) => {
    const flag = s.domainFlags[domainId] as DomainFlags | undefined;
    return Boolean(flag?.requiresRestart);
  });

  const isCloudLlmMissingKey =
    llmCategory === "cloud" && !cloudApiKey?.trim();
  const isRealtimeMissingKey =
    pipelineMode === "realtime" && !realtimeKey?.trim();

  const isMissingKey = useMemo(() => {
    if (domainId === "language") {
      return isCloudLlmMissingKey;
    }
    if (domainId === "speech" || domainId === "interaction") {
      return pipelineMode === "realtime" ? isRealtimeMissingKey : false;
    }
    return false;
  }, [domainId, pipelineMode, isRealtimeMissingKey, isCloudLlmMissingKey]);

  const autoSaved = useSettingsStore((s) => s.autoSavedDomain === domainId);
  // One selector for both: the failure list is either absent or a non-empty
  // array of rejected keys.
  const failedKeys = useSettingsStore((s) => s.failedSaveDomains[domainId]);
  const saveFailure = (failedKeys?.length ?? 0) > 0;
  const isRestarting = useSettingsStore(
    useCallback(
      (s: SettingsState) => {
        if (!s.restartInFlight) return false;
        if (s.restartKeys.length === 0) return domainId === "speech";
        return s.restartKeys.some((k) => {
          const scope = k.split(".")[0];
          const targetUi = SETTINGS_DOMAIN_TO_UI[scope] ?? "speech";
          return targetUi === domainId;
        });
      },
      [domainId]
    )
  );

  const isMissingModel = useMemo(() => {
    if (domainId !== "language") return false;
    return isLlmSelectionMissingModel({ category: llmCategory, model: llmActiveModel });
  }, [domainId, llmCategory, llmActiveModel]);

  // Hot-save changes stay uncommitted only during the 600ms autosave window.
  // There is intentionally no routine Save/Cancel UI for them: they resolve to
  // the saved toast, while restart-class changes keep explicit controls.
  let mode: DomainCommitMode | null = null;
  if (hasChanges && isMissingKey) {
    mode = "missing-key";
  } else if (hasChanges && isMissingModel) {
    // Never offer "Apply & Restart" for a selection that has no model: the
    // engine would receive an empty model id. Only Discard is actionable.
    mode = "missing-model";
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
/**
 * Shared footer banner grammar for every settings surface: card footers,
 * inline desks, and expanded modals render failures and transient provider
 * errors in this one shape, so copy wraps inside the parent width instead of
 * overflowing it. Title and detail both shrink and wrap; only the icon is
 * fixed.
 */
export interface SettingsFooterBannerProps {
  title: string;
  detail?: string | null;
  tone?: "danger" | "accent";
  icon?: "alert" | "check" | "restart";
  className?: string;
}

export const SettingsFooterBanner = memo(({
  title,
  detail,
  tone = "danger",
  icon = "alert",
  className,
}: SettingsFooterBannerProps) => {
  const danger = tone === "danger";
  return (
    <div
      role={danger ? "alert" : "status"}
      className={cn(
        "w-full max-w-full py-2 px-4 flex flex-col gap-0.5 overflow-hidden text-[12px] animate-in fade-in duration-150 box-border",
        danger
          ? "bg-[rgba(var(--danger),0.1)] border border-[rgba(var(--danger),0.25)]"
          : "bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.2)]",
        className
      )}
    >
      <span
        className={cn(
          "font-bold uppercase tracking-wider flex items-center gap-1.5 min-w-0 w-full",
          danger ? "text-[rgb(var(--danger))]" : "text-[rgb(var(--accent))]"
        )}
      >
        {icon === "check" ? (
          <Check size={14} className="shrink-0" />
        ) : icon === "restart" ? (
          <RefreshCw size={13} className="shrink-0" />
        ) : (
          <AlertCircle size={14} className="shrink-0" />
        )}
        <span className="min-w-0 flex-1 [overflow-wrap:anywhere] break-words">{title}</span>
      </span>
      {detail ? (
        <span
          className={cn(
            "text-[11px] font-mono min-w-0 w-full [overflow-wrap:anywhere] break-all leading-snug",
            danger ? "text-[rgb(var(--danger))]/80" : "text-[rgb(var(--accent))]/80"
          )}
        >
          {detail}
        </span>
      ) : null}
    </div>
  );
});

SettingsFooterBanner.displayName = "SettingsFooterBanner";

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
  const { isExpanded } = useExpandableList();
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
    if (isExpanded) {
      // In modal headers, do not render dead Save buttons or header alerts.
      // Missing API key alerts are rendered via the unified footer banner at the bottom.
      return null;
    }
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

  if (mode === "missing-model") {
    if (isExpanded) {
      // Same rule as a missing key: the modal shows one footer banner instead
      // of a dead header button.
      return null;
    }
    return (
      <div className="flex items-center gap-1.5 shrink-0">
        <span className="flex items-center gap-1 text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] shrink-0">
          <AlertCircle size={13} className="shrink-0" />
          <span className="hidden md:inline">{SETTINGS_COPY.modelRequired}</span>
        </span>
        <button
          type="button"
          disabled
          className="px-2.5 py-1 rounded-lg font-bold text-[11px] uppercase tracking-wider bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]/40 cursor-not-allowed border border-[rgba(var(--border),0.1)] shrink-0"
        >
          {SETTINGS_COPY.applyAndRestart}
        </button>
        <button
          type="button"
          onClick={() => useSettingsStore.getState().discardDomainChanges(domainId)}
          className="px-2.5 py-1 rounded-lg bg-transparent text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.1)] border border-transparent hover:border-[rgba(var(--danger),0.2)] text-[11px] font-bold uppercase tracking-wider transition-all cursor-pointer shrink-0"
        >
          {SETTINGS_COPY.discardChanges}
        </button>
      </div>
    );
  }

  const isSmall = layoutMode === "small" || isMobile || isCompact;
  const showLabel = !isSmall;
  // Small and mobile layouts get an icon-only button: the full "Apply &
  // Restart" label does not fit next to the close control.
  const actionLabel = showLabel ? SETTINGS_COPY.applyAndRestart : SETTINGS_COPY.restartOnly;

  return (
    <div className="flex items-center justify-between gap-2 shrink-0">
      <Tooltip label={actionLabel} side="bottom">
        <button
          type="button"
          onClick={async () => {
            await useSettingsStore.getState().commitChanges();
          }}
          aria-label={actionLabel}
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

export const ModalProviderFooter = memo(({ category }: { category: "STT" | "LLM" | "TTS" }) => {
  const isTts = category === "TTS";
  const failedKeys = useSettingsStore((s) => s.failedSaveDomains[isTts ? "speech" : "language"]);
  const remoteModelsError = useCatalogStore((s) => s.remoteModelsError);
  const { llmCategory, llmActiveModel, cloudApiKey } = useLlmCloudApiKey(category === "LLM");
  const isCloudLlmMissingKey = category === "LLM" && llmCategory === "cloud" && !cloudApiKey?.trim();
  const isMissingModel =
    isLlmSelectionMissingModel({ category: llmCategory, model: llmActiveModel });

  if (category === "STT") return null;

  // backend-gap: structured error_kind on fetch_remote_models response.
  // The catalog reports a missing cloud credential as a plain error string.
  // Rendering it as "Connection failed" next to the missing-key banner showed
  // the same cause twice. A missing credential is a credential problem, not a
  // connection failure: one banner, and only the credential one.
  const isApiKeyError = Boolean(remoteModelsError && /api key required/i.test(remoteModelsError));
  const showMissingKey = category === "LLM" && (isCloudLlmMissingKey || isApiKeyError);
  const showConnectionFailed = Boolean(remoteModelsError) && !(category === "LLM" && isApiKeyError);

  const hasFailedKeys = (failedKeys?.length ?? 0) > 0;
  const showMissingModel = category === "LLM" && isMissingModel;
  if (!hasFailedKeys && !showMissingKey && !showConnectionFailed && !showMissingModel) return null;
  return (
    <div className="flex flex-col gap-2 shrink-0 w-full max-w-full">
      {showMissingModel && (
        <SettingsFooterBanner
          title={SETTINGS_COPY.modelRequired}
          detail={SETTINGS_COPY.modelRequiredDetail}
          tone="accent"
          className="rounded-xl w-full max-w-full"
        />
      )}
      {showMissingKey && (
        <SettingsFooterBanner
          title={SETTINGS_COPY.apiKeyRequired}
          detail={SETTINGS_COPY.apiKeyRequiredDetail}
          className="rounded-xl w-full max-w-full"
        />
      )}
      {hasFailedKeys && failedKeys && (
        <SettingsFooterBanner
          title={SETTINGS_COPY.saveFailedTitle}
          detail={`${SETTINGS_COPY.saveFailedHint} ${failedKeys.join(", ")}`}
          className="rounded-xl w-full max-w-full"
        />
      )}
      {showConnectionFailed && (
        <SettingsFooterBanner
          title={PROVIDERS_CONFIG_DESK_COPY.status.connectionFailed}
          detail={remoteModelsError}
          className="rounded-xl w-full max-w-full"
        />
      )}
    </div>
  );
});
ModalProviderFooter.displayName = "ModalProviderFooter";
