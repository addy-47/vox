import { useMemo, memo } from "react";
import { useSettingsStore, LlmProviderConfig } from "@/store/settingsStore";
import {
  Microchip, Zap, Battery, TextCursorInput, Layers2, WandSparkles, Gauge, Server
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { LLM_SETTINGS_COPY, COMPUTE_PROFILE_COPY } from "@/data/settingsCopy";
import {
  SettingsTabPane,
  PresetButton,
  PresetCell,
  PresetInput,
  RemoteComputeGraphic,
  ManagedContextGraphic,
} from "./SettingsTabPane";

export type SettingsSubTab = "compute" | "tokens" | "context" | "creativity";

export interface LlmSettingsViewProps {
  activeSubTab?: SettingsSubTab;
  layoutMode?: "full-max" | "full-min" | "small";
  isRemoteLlm: boolean;
  isCloud: boolean;
  provider?: LlmProviderConfig;
}

export const LlmSettingsView = memo(({
  activeSubTab = "compute",
  layoutMode,
  isRemoteLlm,
  isCloud,
}: LlmSettingsViewProps) => {
  const llmSettings = useSettingsStore((s) => s.draftSettings?.llm);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  // CPU Core configuration
  const totalCores = (typeof navigator !== "undefined" ? navigator.hardwareConcurrency : undefined) || 4;
  const optimalThreads = Math.max(2, totalCores - 2);
  const powerSaverThreads = Math.max(1, Math.floor(totalCores / 2));

  // Determine current CPU profile
  const currentThreads = llmSettings?.threads || optimalThreads;
  const currentProfile = useMemo(() => {
    if (currentThreads === optimalThreads) return "auto";
    if (currentThreads === powerSaverThreads) return "power";
    if (currentThreads === totalCores) return "max";
    return "custom";
  }, [currentThreads, optimalThreads, powerSaverThreads, totalCores]);

  // Determine token budget (default: 300)
  const currentTokens = llmSettings?.max_output_tokens ?? 300;

  // Determine creativity / temperature (default: 0.7)
  const currentTemp = llmSettings?.temperature ?? 0.7;
  const currentContext = Math.max(8192, llmSettings?.context_window ?? 8192);

  // Active capabilities and dynamic bounds
  const MIN_CONTEXT_WINDOW = 8192;
  const capabilitiesCache = useSettingsStore((s) => s.capabilitiesCache);
  const activeModel = useMemo(() => {
    if (llmSettings?.active === "embedded") return llmSettings?.embedded?.model;
    if (llmSettings?.active === "server") return llmSettings?.server?.model;
    if (llmSettings?.active === "cloud") return llmSettings?.cloud?.model;
    return undefined;
  }, [llmSettings?.active, llmSettings?.embedded?.model, llmSettings?.server?.model, llmSettings?.cloud?.model]);

  const activeCapabilities = useMemo(() => {
    if (!activeModel) return undefined;
    return (
      capabilitiesCache?.[`open_ai_compat:${activeModel}`] ||
      capabilitiesCache?.[`server:${activeModel}`] ||
      capabilitiesCache?.[`cloud:${activeModel}`] ||
      capabilitiesCache?.[`embedded:${activeModel}`] ||
      capabilitiesCache?.[activeModel]
    );
  }, [activeModel, capabilitiesCache]);

  const maxContextCeiling = activeCapabilities?.context_window || 131072;
  const contextPresets = useMemo(() => {
    return [8192, 16384, 32768].filter((size) => size <= maxContextCeiling);
  }, [maxContextCeiling]);

  if (!llmSettings) return null;

  const isCloudProvider = isRemoteLlm && isCloud;

  const tokensCustomSelected = ![0, 300, 1000].includes(currentTokens);
  const contextCustomSelected = !contextPresets.includes(currentContext);
  const tempCustomSelected = ![0.2, 0.7, 1.0].some((v) => Math.abs(currentTemp - v) < 0.05);

  return (
    <div
      className={cn(
        "w-full flex flex-col flex-1 min-h-0 select-none",
        layoutMode === "small" ? "h-auto py-1 space-y-2.5" : "h-full"
      )}
    >
      {/* TAB 1: COMPUTE */}
      {activeSubTab === "compute" && (
        !isRemoteLlm ? (
          <SettingsTabPane
            icon={Microchip}
            title={COMPUTE_PROFILE_COPY.title}
            description={LLM_SETTINGS_COPY.compute.description}
            layoutMode={layoutMode}
            controls={
              <>
                <PresetButton
                  mono={false}
                  selected={currentProfile === "auto"}
                  onClick={() => updateDraft("llm", "threads", optimalThreads)}
                >
                  <Zap size={11} className="text-[rgb(var(--accent))]" />
                  <span>{COMPUTE_PROFILE_COPY.auto}</span>
                </PresetButton>
                <PresetButton
                  mono={false}
                  selected={currentProfile === "power"}
                  onClick={() => updateDraft("llm", "threads", powerSaverThreads)}
                >
                  <Battery size={11} className="text-emerald-400" />
                  <span>{COMPUTE_PROFILE_COPY.eco}</span>
                </PresetButton>
                <PresetButton
                  mono={false}
                  selected={currentProfile === "max"}
                  onClick={() => updateDraft("llm", "threads", totalCores)}
                >
                  <Gauge size={11} className="text-amber-400" />
                  <span>{COMPUTE_PROFILE_COPY.max}</span>
                </PresetButton>
                <PresetCell>{currentThreads}T</PresetCell>
              </>
            }
          />
        ) : (
          <SettingsTabPane
            icon={Server}
            title={isCloudProvider ? LLM_SETTINGS_COPY.compute.remoteTitleLocal : LLM_SETTINGS_COPY.compute.remoteTitleRemote}
            description={LLM_SETTINGS_COPY.compute.remoteDescription}
            layoutMode={layoutMode}
            emptyGraphic={
              <RemoteComputeGraphic
                label={isCloudProvider ? "Cloud Offload" : "Remote Server"}
                subLabel="Zero Host RAM"
              />
            }
          />
        )
      )}

      {/* TAB 2: TOKENS */}
      {activeSubTab === "tokens" && (
        <SettingsTabPane
          icon={TextCursorInput}
          title={LLM_SETTINGS_COPY.tokens.title}
          description={LLM_SETTINGS_COPY.tokens.description}
          layoutMode={layoutMode}
          controls={
            <>
              {[
                { label: "300", val: 300 },
                { label: "1000", val: 1000 },
                { label: LLM_SETTINGS_COPY.tokens.native, val: 0 },
              ].map(({ label, val }) => (
                <PresetButton
                  key={label}
                  selected={currentTokens === val}
                  onClick={() => updateDraft("llm", "max_output_tokens", val)}
                >
                  {label}
                </PresetButton>
              ))}
              <PresetInput
                selected={tokensCustomSelected}
                value={tokensCustomSelected ? `${currentTokens}` : ""}
                placeholder={COMPUTE_PROFILE_COPY.custom}
                onChange={(e) => {
                  const clean = e.target.value.replace(/[^0-9]/g, "");
                  if (!clean) return;
                  const num = parseInt(clean, 10);
                  if (!isNaN(num) && num >= 50 && num <= 128000) {
                    updateDraft("llm", "max_output_tokens", num);
                  }
                }}
              />
            </>
          }
        />
      )}

      {/* TAB 3: CONTEXT */}
      {activeSubTab === "context" && (
        <SettingsTabPane
          icon={Layers2}
          title={LLM_SETTINGS_COPY.context.title}
          description={LLM_SETTINGS_COPY.context.description}
          layoutMode={layoutMode}
          controls={
            !isRemoteLlm ? (
              <>
                {contextPresets.map((size) => (
                  <PresetButton
                    key={size}
                    selected={currentContext === size}
                    onClick={() => updateDraft("llm", "context_window", size)}
                  >
                    {size / 1024}k
                  </PresetButton>
                ))}
                <PresetInput
                  selected={contextCustomSelected}
                  value={contextCustomSelected ? `${currentContext}` : ""}
                  placeholder={COMPUTE_PROFILE_COPY.custom}
                  onChange={(e) => {
                    const clean = e.target.value.replace(/[^0-9]/g, "");
                    if (!clean) return;
                    const num = parseInt(clean, 10);
                    if (!isNaN(num) && num >= MIN_CONTEXT_WINDOW && num <= maxContextCeiling) {
                      updateDraft("llm", "context_window", num);
                    }
                  }}
                />
              </>
            ) : undefined
          }
          emptyGraphic={
            isRemoteLlm ? (
              <ManagedContextGraphic
                label={
                  activeCapabilities?.context_window
                    ? `${activeCapabilities.context_window >= 1024 ? `${activeCapabilities.context_window / 1024}k` : activeCapabilities.context_window} tok max`
                    : "Server Managed"
                }
                subLabel="Dynamic Alloc"
              />
            ) : undefined
          }
        />
      )}

      {/* TAB 4: CREATIVITY / TEMP */}
      {activeSubTab === "creativity" && (
        <SettingsTabPane
          icon={WandSparkles}
          title={LLM_SETTINGS_COPY.creativity.title}
          description={LLM_SETTINGS_COPY.creativity.description}
          layoutMode={layoutMode}
          controls={
            <>
              {[
                { label: "0.2", val: 0.2 },
                { label: "0.7", val: 0.7 },
                { label: "1.0", val: 1.0 },
              ].map(({ label, val }) => (
                <PresetButton
                  key={label}
                  selected={Math.abs(currentTemp - val) < 0.05}
                  onClick={() => updateDraft("llm", "temperature", val)}
                >
                  {label}
                </PresetButton>
              ))}
              <PresetInput
                selected={tempCustomSelected}
                value={tempCustomSelected ? currentTemp.toFixed(2) : ""}
                placeholder={COMPUTE_PROFILE_COPY.custom}
                inputMode="decimal"
                onChange={(e) => {
                  const clean = e.target.value.replace(/[^0-9.]/g, "");
                  if (!clean) return;
                  const num = parseFloat(clean);
                  if (!isNaN(num) && num >= 0.0 && num <= 2.0) {
                    updateDraft("llm", "temperature", num);
                  }
                }}
              />
            </>
          }
        />
      )}
    </div>
  );
});

LlmSettingsView.displayName = "LlmSettingsView";
