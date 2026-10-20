import { useMemo, memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { useModelParameters } from "@/shared/hooks/useModelParameters";
import { cn } from "@/shared/lib/utils";
import { LLM_SETTINGS_COPY, CUSTOM_VALUE_COPY } from "@/data/settingsCopy";
import {
  CustomSvgs,
  PresetButton,
  PresetInput,
  ManagedContextGraphic,
} from "../shared/CustomSvgs";

export type LlmTuningTab = "tokens" | "context" | "creativity";

export interface LlmTuningPanelProps {
  activeSubTab: LlmTuningTab;
  layoutMode?: "full-max" | "full-min" | "small";
  /** Remote providers own the context window; the panel shows it read-only. */
  isRemoteLlm: boolean;
  /**
   * Read-only mode. The realtime pipeline owns the voice, so the panel still
   * renders the current values but never writes one.
   */
  disabled?: boolean;
}

/**
 * The three user-facing LLM knobs (response length, context window,
 * creativity). Lifted out of `LlmSettingsView` unchanged so the Tuning card
 * renders the same controls without a second implementation.
 */
export const LlmTuningPanel = memo(
  ({ activeSubTab, layoutMode, isRemoteLlm, disabled = false }: LlmTuningPanelProps) => {
    const llmSettings = useSettingsStore((s) => s.draftSettings?.llm);
    const activeModelId = llmSettings?.active?.model || null;
    // Per-model overrides from the `models` table; catalog defaults apply untouched.
    const { params, setParam } = useModelParameters(activeModelId);

    const numParam = (v: unknown): number | undefined =>
      typeof v === "number" && !Number.isNaN(v) ? v : undefined;

    // Determine token budget (default: 300)
    const currentTokens = numParam(params.max_output_tokens) ?? 300;

    // Determine creativity / temperature (default: 0.7)
    const currentTemp = numParam(params.temperature) ?? 0.7;
    const currentContext = Math.max(8192, numParam(params.context_window) ?? 8192);

    // Active capabilities and dynamic bounds
    const MIN_CONTEXT_WINDOW = 8192;
    const capabilitiesCache = useCatalogStore((s) => s.capabilitiesCache);
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

    const tokensCustomSelected = ![0, 300, 1000].includes(currentTokens);
    const contextCustomSelected = !contextPresets.includes(currentContext);
    const tempCustomSelected = ![0.2, 0.7, 1.0].some((v) => Math.abs(currentTemp - v) < 0.05);

    const write = (key: string, value: number) => {
      if (disabled) return;
      setParam(key, value);
    };

    return (
      <div
        className={cn(
          "w-full flex flex-col flex-1 min-h-0 select-none",
          layoutMode === "small" ? "h-auto py-1 space-y-2.5" : "h-full"
        )}
      >
        {/* TAB: TOKENS */}
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
                    onClick={() => write("max_output_tokens", val)}
                  >
                    {label}
                  </PresetButton>
                ))}
                <PresetInput
                  selected={tokensCustomSelected}
                  value={tokensCustomSelected ? `${currentTokens}` : ""}
                  placeholder={CUSTOM_VALUE_COPY.placeholder}
                  onChange={(e) => {
                    const clean = e.target.value.replace(/[^0-9]/g, "");
                    if (!clean) return;
                    const num = parseInt(clean, 10);
                    if (!isNaN(num) && num >= 50 && num <= 128000) {
                      write("max_output_tokens", num);
                    }
                  }}
                />
              </>
            }
          />
        )}

        {/* TAB: CONTEXT */}
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
                      onClick={() => write("context_window", size)}
                    >
                      {size / 1024}k
                    </PresetButton>
                  ))}
                  <PresetInput
                    selected={contextCustomSelected}
                    value={contextCustomSelected ? `${currentContext}` : ""}
                    placeholder={CUSTOM_VALUE_COPY.placeholder}
                    onChange={(e) => {
                      const clean = e.target.value.replace(/[^0-9]/g, "");
                      if (!clean) return;
                      const num = parseInt(clean, 10);
                      if (!isNaN(num) && num >= MIN_CONTEXT_WINDOW && num <= maxContextCeiling) {
                        write("context_window", num);
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

        {/* TAB: CREATIVITY / TEMP */}
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
                    onClick={() => write("temperature", val)}
                  >
                    {label}
                  </PresetButton>
                ))}
                <PresetInput
                  selected={tempCustomSelected}
                  value={tempCustomSelected ? currentTemp.toFixed(2) : ""}
                  placeholder={CUSTOM_VALUE_COPY.placeholder}
                  inputMode="decimal"
                  onChange={(e) => {
                    const clean = e.target.value.replace(/[^0-9.]/g, "");
                    if (!clean) return;
                    const num = parseFloat(clean);
                    if (!isNaN(num) && num >= 0.0 && num <= 2.0) {
                      write("temperature", num);
                    }
                  }}
                />
              </>
            }
          />
        )}
      </div>
    );
  }
);

LlmTuningPanel.displayName = "LlmTuningPanel";
