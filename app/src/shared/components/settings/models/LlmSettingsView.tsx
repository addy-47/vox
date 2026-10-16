
import { useMemo, memo } from "react";
import { useSettingsStore, LlmProviderConfig } from "@/store/settingsStore";
import { useModelParameters } from "@/shared/hooks/useModelParameters";
import { Zap, Battery, Gauge } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { LLM_SETTINGS_COPY, COMPUTE_PROFILE_COPY } from "@/data/settingsCopy";
import {
  CustomSvgs,
  PresetButton,
  PresetCell,
  PresetInput,
  RemoteComputeGraphic,
  ManagedContextGraphic,
} from "./CustomSvgs";

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
  const activeModelId = llmSettings?.active?.model || null;
  // Per-model overrides from the `models` table; catalog defaults apply untouched.
  const { params, setParam } = useModelParameters(activeModelId);

  const numParam = (v: unknown): number | undefined =>
    typeof v === "number" && !Number.isNaN(v) ? v : undefined;

  // CPU Core configuration
  const totalCores = (typeof navigator !== "undefined" ? navigator.hardwareConcurrency : undefined) || 4;
  const optimalThreads = Math.max(2, totalCores - 2);
  const powerSaverThreads = Math.max(1, Math.floor(totalCores / 2));

  // Determine current CPU profile
  const currentThreads = numParam(params.threads) || optimalThreads;
  const currentProfile = useMemo(() => {
    if (currentThreads === optimalThreads) return "auto";
    if (currentThreads === powerSaverThreads) return "power";
    if (currentThreads === totalCores) return "max";
    return "custom";
  }, [currentThreads, optimalThreads, powerSaverThreads, totalCores]);

  // Determine token budget (default: 300)
  const currentTokens = numParam(params.max_output_tokens) ?? 300;

  // Determine creativity / temperature (default: 0.7)
  const currentTemp = numParam(params.temperature) ?? 0.7;
  const currentContext = Math.max(8192, numParam(params.context_window) ?? 8192);

  // Active capabilities and dynamic bounds
  const MIN_CONTEXT_WINDOW = 8192;
  const capabilitiesCache = useSettingsStore((s) => s.capabilitiesCache);
  const activeCapabilities = useMemo(() => {
    if (!activeModelId) return undefined;
    return (
      capabilitiesCache?.[`server:${activeModelId}`] ||
      capabilitiesCache?.[`cloud:${activeModelId}`] ||
      capabilitiesCache?.[`embedded:${activeModelId}`] ||
      capabilitiesCache?.[activeModelId]
    );
  }, [activeModelId, capabilitiesCache]);

  // Spec §7.1: the ceiling comes from the resolved capability. Unknown stays
  // at the floor — never a fabricated 128k. Remote models without a probed
  // window show "Server Managed" with no manual input (see the context tab).
  const maxContextCeiling = activeCapabilities?.context_window || MIN_CONTEXT_WINDOW;
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
          <CustomSvgs
            title={COMPUTE_PROFILE_COPY.title}
            description={LLM_SETTINGS_COPY.compute.description}
            layoutMode={layoutMode}
            controls={
              <>
                <PresetButton
                  mono={false}
                  selected={currentProfile === "auto"}
                  onClick={() => setParam("threads", optimalThreads)}
                >
                  <Zap size={11} className="text-[rgb(var(--accent))]" />
                  <span>{COMPUTE_PROFILE_COPY.auto}</span>
                </PresetButton>
                <PresetButton
                  mono={false}
                  selected={currentProfile === "power"}
                  onClick={() => setParam("threads", powerSaverThreads)}
                >
                  <Battery size={11} className="text-[rgb(var(--success))]" />
                  <span>{COMPUTE_PROFILE_COPY.eco}</span>
                </PresetButton>
                <PresetButton
                  mono={false}
                  selected={currentProfile === "max"}
                  onClick={() => setParam("threads", totalCores)}
                >
                  <Gauge size={11} className="text-[rgb(var(--warn-soft))]" />
                  <span>{COMPUTE_PROFILE_COPY.max}</span>
                </PresetButton>
                <PresetCell>{currentThreads}T</PresetCell>
              </>
            }
          />
        ) : (
          <CustomSvgs
            title={isCloudProvider ? LLM_SETTINGS_COPY.compute.remoteTitleLocal : LLM_SETTINGS_COPY.compute.remoteTitleRemote}
            description={LLM_SETTINGS_COPY.compute.remoteDescription}
            layoutMode={layoutMode}
            emptyGraphic={<RemoteComputeGraphic />}
          />
        )
      )}

      {/* TAB 2: TOKENS */}
      {activeSubTab === "tokens" && (
        <CustomSvgs
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
                  onClick={() => setParam("max_output_tokens", val)}
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
                    setParam("max_output_tokens", num);
                  }
                }}
              />
            </>
          }
        />
      )}

      {/* TAB 3: CONTEXT */}
      {activeSubTab === "context" && (
        <CustomSvgs
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
                    onClick={() => setParam("context_window", size)}
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
                      setParam("context_window", num);
                    }
                  }}
                />
              </>
            ) : undefined
          }
          emptyGraphic={
            isRemoteLlm ? <ManagedContextGraphic /> : undefined
          }
        />
      )}

      {/* TAB 4: CREATIVITY / TEMP */}
      {activeSubTab === "creativity" && (
        <CustomSvgs
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
                  onClick={() => setParam("temperature", val)}
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
                    setParam("temperature", num);
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
