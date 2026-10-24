import { useState, memo, useCallback } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { useModelParameters } from "@/shared/hooks/useModelParameters";
import { SlidersHorizontal, Mic, MicOff, Activity, Radio, Layers, Zap } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ToggleTile } from "@/shared/ui";
import { CardTitleBar } from "../shared/CardTitleBar";
import { SETTINGS_DOMAIN_CARD_HEIGHT_CLASS } from "../shared/settingsLayout";
import { DictationConfigDesk } from "./DictationConfigDesk";
import { WebSearchGlobe } from "./WebSearchGlobe";
import { RemoteDeskPanel } from "./RemoteDeskPanel";
import { ControllerRemoteDeskPanel } from "./ControllerRemoteDeskPanel";
import { TransliterationPanel } from "./TransliterationPanel";
import { isRealMobileDevice } from "@/layout/breakpoints";
import { DICTATION_COPY, INTERACTION_CARD_COPY, TRIGGER_MODE_COPY, PIPELINE_MODE_COPY, WEB_SEARCH_COPY, REMOTE_CONTROL_COPY, TRANSLITERATION_COPY } from "@/data/settingsCopy";

interface InteractionCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

type InteractionDeskTab = "remote" | "web_search" | "transliteration";

const INTERACTION_DESK_TABS: Array<{ id: InteractionDeskTab; label: string }> = [
  { id: "remote", label: REMOTE_CONTROL_COPY.tab },
  { id: "web_search", label: WEB_SEARCH_COPY.tab },
  { id: "transliteration", label: TRANSLITERATION_COPY.tab },
];

export const InteractionCard = memo(
  ({ layoutMode = "full-max" }: InteractionCardProps) => {
    const settings = useSettingsStore((s) => s.settings);
    const draftSettings = useSettingsStore((s) => s.draftSettings);
    const updateDraft = useSettingsStore((s) => s.updateDraft);

    const [activeView, setActiveView] = useState<"assistant" | "dictation">("assistant");
    const [activeDeskTab, setActiveDeskTab] = useState<InteractionDeskTab>("remote");

    const isRealtime = draftSettings?.interaction.pipeline_mode === "realtime";
    const activeRealtimeModel = draftSettings?.realtime.active.model || null;
    const { params: realtimeParams, setParam: setRealtimeParam } = useModelParameters(
      isRealtime ? activeRealtimeModel : null
    );

    const webSearchEnabled = isRealtime
      ? (realtimeParams.enable_web_search !== false && (draftSettings?.interaction.web_search_enabled ?? true))
      : (draftSettings?.interaction.web_search_enabled ?? true);

    const handleToggleWebSearch = useCallback(() => {
      const next = !webSearchEnabled;
      updateDraft("interaction", "web_search_enabled", next);
      if (isRealtime && activeRealtimeModel) {
        setRealtimeParam("enable_web_search", next);
      }
    }, [webSearchEnabled, updateDraft, isRealtime, activeRealtimeModel, setRealtimeParam]);

    if (!draftSettings || !settings) return null;
    const { interaction, dictation } = draftSettings;

    const dictationEnabled = dictation.enabled ?? true;
    const dictationInteractionMode = dictation.interaction_mode ?? "ptt";
    const interactionMode = interaction.mode ?? "passive";
    const isPassive = String(interactionMode).toLowerCase() === "passive";
    const pipelineMode = interaction.pipeline_mode ?? "modular";
    const isModular = pipelineMode === "modular";

    return (
      <div
        className={cn(
          "w-full flex flex-col text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 select-none justify-between",
          layoutMode === "small"
            ? "bg-transparent p-0 h-auto"
            : cn("glass-card p-5", SETTINGS_DOMAIN_CARD_HEIGHT_CLASS)
        )}
      >
        {/* Header Section */}
        <CardTitleBar
          icon={<SlidersHorizontal size={activeView === "dictation" ? 14 : 17} />}
          title={activeView === "dictation" ? INTERACTION_CARD_COPY.viewDictation : INTERACTION_CARD_COPY.cardTitle}
          onBack={activeView === "dictation" ? () => setActiveView("assistant") : undefined}
          actionLabel={activeView === "assistant" ? INTERACTION_CARD_COPY.viewDictation : undefined}
          onAction={activeView === "assistant" ? () => setActiveView("dictation") : undefined}
        />

        <div className="flex flex-col gap-3 flex-1 min-h-0">
          {activeView === "assistant" ? (
            <>
              {/* Core Assistant Controls */}
              <div className="grid grid-cols-2 gap-2 shrink-0">
                <ToggleTile
                  title={TRIGGER_MODE_COPY.cardTitle}
                  active={isPassive}
                  activeLabel={TRIGGER_MODE_COPY.continuousTitle}
                  inactiveLabel={TRIGGER_MODE_COPY.pttTitle}
                  activeSublabel={TRIGGER_MODE_COPY.continuousSub}
                  inactiveSublabel={TRIGGER_MODE_COPY.pttSub}
                  icon={isPassive ? Activity : Radio}
                  onToggle={() => updateDraft("interaction", "mode", isPassive ? "ptt" : "passive")}
                  layoutMode={layoutMode}
                  visualizer={
                    isPassive ? (
                      <div className="flex items-end gap-[1.5px] h-3">
                        <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-1" />
                        <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-2" />
                        <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-3" />
                        <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-4" />
                      </div>
                    ) : (
                      <div className="w-3 h-3 rounded-full border border-[rgb(var(--accent))]/40 flex items-center justify-center relative">
                        <span className="absolute inset-0 rounded-full border border-[rgb(var(--accent))] animate-ping opacity-60" />
                        <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
                      </div>
                    )
                  }
                />
                <ToggleTile
                  title={PIPELINE_MODE_COPY.cardTitle}
                  active={isModular}
                  activeLabel={PIPELINE_MODE_COPY.modularTitle}
                  inactiveLabel={PIPELINE_MODE_COPY.realtimeTitle}
                  activeSublabel={PIPELINE_MODE_COPY.modularSub}
                  inactiveSublabel={PIPELINE_MODE_COPY.realtimeSub}
                  icon={isModular ? Layers : Zap}
                  onToggle={() => updateDraft("interaction", "pipeline_mode", isModular ? "realtime" : "modular")}
                  layoutMode={layoutMode}
                  visualizer={
                    isModular ? (
                      <div className="flex flex-col gap-[1.5px] items-center">
                        <span className="w-3.5 h-[2px] bg-[rgb(var(--accent))] rounded animate-pulse" />
                        <span className="w-2.5 h-[2px] bg-[rgb(var(--accent))]/60 rounded animate-pulse" />
                        <span className="w-3.5 h-[2px] bg-[rgb(var(--accent))] rounded animate-pulse" />
                      </div>
                    ) : (
                      <div className="w-7 h-2 bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.06)] rounded-full relative overflow-hidden flex items-center">
                        <span className="absolute w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-flow-dot shadow-[0_0_6px_rgba(var(--accent),0.8)]" />
                      </div>
                    )
                  }
                />
              </div>

              {/* Interaction Desk: Remote Control (placeholder) | Web Search | Transliteration */}
              <div
                className={cn(
                  "flex-1 w-full flex flex-col min-h-0 rounded-xl p-3 relative border border-[rgba(var(--accent),0.06)] bg-[rgba(var(--foreground),0.02)] overflow-hidden",
                  layoutMode === "small" ? "h-auto" : "h-full"
                )}
              >
                <div className="w-full flex flex-col h-full">
                  {/* Tab Row — equal-width grid, no shift between tabs */}
                  <div className="w-full grid grid-cols-3 gap-1 pt-0.5 pb-1.5 shrink-0 border-b border-[rgba(var(--accent),0.08)] mb-2 px-0.5">
                    {INTERACTION_DESK_TABS.map((tab) => {
                      const isActive = activeDeskTab === tab.id;
                      return (
                        <button
                          key={tab.id}
                          type="button"
                          onClick={() => setActiveDeskTab(tab.id)}
                          className={cn(
                            "w-full min-w-0 flex items-center justify-center pb-1 border-b-2 transition-all duration-200 bg-transparent text-[9.5px] sm:text-[10.5px] xl:text-[11px] font-black uppercase tracking-[0.04em] sm:tracking-[0.08em] outline-none cursor-pointer text-center truncate px-0.5",
                            isActive
                              ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                              : "text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]"
                          )}
                        >
                          <span className="truncate">{tab.label}</span>
                        </button>
                      );
                    })}
                  </div>

                  {/* Panel */}
                  {activeDeskTab === "remote" ? (
                    isRealMobileDevice() ? (
                      <ControllerRemoteDeskPanel />
                    ) : (
                      <RemoteDeskPanel />
                    )
                  ) : activeDeskTab === "web_search" ? (
                    <div className="flex flex-row items-center justify-between gap-4 h-full p-2 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
                      <div className="flex flex-col gap-1 min-w-0 flex-1">
                        <span className="text-[12px] font-bold  tracking-wider text-[rgb(var(--foreground))]">
                          {WEB_SEARCH_COPY.title}
                        </span>
                        <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                          {isRealtime ? WEB_SEARCH_COPY.descriptionRealtime : WEB_SEARCH_COPY.descriptionModular}
                        </p>
                      </div>
                      <div className="shrink-0 w-[116px] flex flex-col items-center justify-center">
                        <WebSearchGlobe
                          enabled={webSearchEnabled}
                          onToggle={handleToggleWebSearch}
                          label={webSearchEnabled ? WEB_SEARCH_COPY.enabled : WEB_SEARCH_COPY.disabled}
                          ariaLabel={WEB_SEARCH_COPY.toggleAria}
                        />
                      </div>
                    </div>
                  ) : (
                    <TransliterationPanel />
                  )}
                </div>
              </div>
            </>
          ) : (

            <>
              {/* Dictation Mode Controls */}
              <div className="grid grid-cols-2 gap-2 shrink-0">
                <ToggleTile
                  title={INTERACTION_CARD_COPY.voiceTyping}
                  active={dictationEnabled}
                  activeLabel={INTERACTION_CARD_COPY.toggleEnabled}
                  inactiveLabel={INTERACTION_CARD_COPY.toggleDisabled}
                  activeSublabel={DICTATION_COPY.voiceTypingActive}
                  inactiveSublabel={DICTATION_COPY.voiceTypingInactive}
                  icon={dictationEnabled ? Mic : MicOff}
                  onToggle={() => updateDraft("dictation", "enabled", !dictationEnabled)}
                  layoutMode={layoutMode}
                />

                <ToggleTile
                  title={INTERACTION_CARD_COPY.triggerMode}
                  active={dictationInteractionMode === "passive"}
                  activeLabel={DICTATION_COPY.triggerContinuous}
                  inactiveLabel={DICTATION_COPY.triggerPtt}
                  activeSublabel={DICTATION_COPY.triggerContinuousSub}
                  inactiveSublabel={DICTATION_COPY.triggerPttSub}
                  icon={dictationInteractionMode === "passive" ? Activity : Radio}
                  disabled={!dictationEnabled}
                  onToggle={() =>
                    updateDraft(
                      "dictation",
                      "interaction_mode",
                      dictationInteractionMode === "passive" ? "ptt" : "passive"
                    )
                  }
                  layoutMode={layoutMode}
                />
              </div>

              {/* Dictation Output & Hotkey Configuration Desk */}
              <div
                className={cn(
                  "flex-1 w-full flex flex-col min-h-0 rounded-xl p-2.5 sm:p-3 relative border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] justify-between mt-0.5",
                  layoutMode === "small" ? "h-auto" : "h-full"
                )}
              >
                <DictationConfigDesk layoutMode={layoutMode} disabled={!dictationEnabled} />
              </div>
            </>
          )}
        </div>
      </div>
    );
  }
);

InteractionCard.displayName = "InteractionCard";
