import { useMemo, memo } from "react";
import { useSettingsStore, LlmProviderConfig } from "@/store/settingsStore";
import {
  Microchip, Zap, Battery, TextCursorInput, Layers2, WandSparkles, Check, Gauge, Server
} from "lucide-react";
import { cn } from "@/shared/lib/utils";

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
  const currentContext = llmSettings?.context_window ?? 2048;

  if (!llmSettings) return null;

  const isCloudProvider = isRemoteLlm && isCloud;

  return (
    <div className="w-full flex-1 flex flex-col justify-between select-none animate-fade-in">
      {/* ─── Layer: Workspace Content (Full Height Single Surface) ─── */}
      <div
        className={cn(
          "w-full flex flex-col flex-1 min-h-0 pt-0.5 pb-0.5 justify-between",
          layoutMode === "small"
            ? "h-auto max-h-none py-1 space-y-2.5"
            : "h-full"
        )}
      >
        {/* TAB 1: COMPUTE - Side-by-Side Ergonomics with 2x2 Grid */}
        {activeSubTab === "compute" && (
          <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            {!isRemoteLlm ? (
              <>
                <div className="flex flex-col gap-1 min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] flex items-center gap-1.5">
                      <Microchip size={14} className="text-[rgb(var(--accent))]" />
                      Compute Allocation
                    </span>
                    <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                      {currentThreads} / {totalCores} Cores
                    </span>
                  </div>
                  <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                    Allocate local CPU worker threads for model reasoning. Auto balances thermal load and latency.
                  </p>
                </div>

                {/* 2x2 Grid: [Auto, Eco, Max, Cores] */}
                <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[116px] sm:w-[136px]">
                  <button
                    type="button"
                    onClick={() => updateDraft("llm", "threads", optimalThreads)}
                    className={cn(
                      "py-1 rounded-lg border text-[11px] font-bold transition-all duration-200 cursor-pointer flex items-center justify-center gap-1",
                      currentProfile === "auto"
                        ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                        : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    <Zap size={11} className="text-[rgb(var(--accent))]" />
                    <span>Auto</span>
                  </button>

                  <button
                    type="button"
                    onClick={() => updateDraft("llm", "threads", powerSaverThreads)}
                    className={cn(
                      "py-1 rounded-lg border text-[11px] font-bold transition-all duration-200 cursor-pointer flex items-center justify-center gap-1",
                      currentProfile === "power"
                        ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                        : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    <Battery size={11} className="text-emerald-400" />
                    <span>Eco</span>
                  </button>

                  <button
                    type="button"
                    onClick={() => updateDraft("llm", "threads", totalCores)}
                    className={cn(
                      "py-1 rounded-lg border text-[11px] font-bold transition-all duration-200 cursor-pointer flex items-center justify-center gap-1",
                      currentProfile === "max"
                        ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                        : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    <Gauge size={11} className="text-amber-400" />
                    <span>Max</span>
                  </button>

                  <div className="py-1 rounded-lg border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[10px] font-mono font-bold text-[rgb(var(--foreground-muted))]/70 flex items-center justify-center">
                    {currentThreads}T
                  </div>
                </div>
              </>
            ) : (
              <div className="flex flex-col justify-between h-full gap-1.5 min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] flex items-center gap-1.5">
                    <Server size={14} className="text-[rgb(var(--accent))]" />
                    {isCloudProvider ? "Cloud Infrastructure" : "Remote Compute"}
                  </span>
                  <span className="text-[10px] sm:text-[11px] font-bold text-emerald-400 flex items-center gap-1">
                    <Check size={12} /> Active
                  </span>
                </div>
                <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                  Inference computation is offloaded entirely to the remote provider. Zero local CPU or RAM is consumed.
                </p>
              </div>
            )}
          </div>
        )}

        {/* TAB 2: TOKENS - Side-by-Side Ergonomics with 2x2 Grid */}
        {activeSubTab === "tokens" && (
          <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] flex items-center gap-1.5">
                  <TextCursorInput size={14} className="text-[rgb(var(--accent))]" />
                  Token Limit
                </span>
                <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                  {currentTokens === 0 ? "Native" : `${currentTokens} tok`}
                </span>
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                Maximum token generation per reply. Concise caps prevent rambling; Native lets the model complete reasoning.
              </p>
            </div>

            {/* 2x2 Grid: [300, 1000, Native, Custom] */}
            <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[116px] sm:w-[136px]">
              {[
                { label: "300", val: 300 },
                { label: "1000", val: 1000 },
                { label: "Native", val: 0 },
              ].map(({ label, val }) => {
                const isSelected = currentTokens === val;
                return (
                  <button
                    key={label}
                    type="button"
                    onClick={() => updateDraft("llm", "max_output_tokens", val)}
                    className={cn(
                      "py-1 rounded-lg border text-[11px] font-mono font-bold transition-all duration-200 cursor-pointer flex items-center justify-center",
                      isSelected
                        ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                        : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    {label}
                  </button>
                );
              })}
              <div
                className={cn(
                  "rounded-lg border flex items-center justify-center transition-all overflow-hidden",
                  ![0, 300, 1000].includes(currentTokens)
                    ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                    : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                )}
              >
                <input
                  type="text"
                  inputMode="numeric"
                  value={![0, 300, 1000].includes(currentTokens) ? currentTokens : ""}
                  onChange={(e) => {
                    const clean = e.target.value.replace(/[^0-9]/g, "");
                    if (!clean) return;
                    const num = parseInt(clean, 10);
                    if (!isNaN(num) && num >= 50 && num <= 128000) {
                      updateDraft("llm", "max_output_tokens", num);
                    }
                  }}
                  placeholder="Custom"
                  className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                />
              </div>
            </div>
          </div>
        )}

        {/* TAB 3: CONTEXT - Side-by-Side Ergonomics with 2x2 Grid */}
        {activeSubTab === "context" && (
          <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] flex items-center gap-1.5">
                  <Layers2 size={14} className="text-[rgb(var(--accent))]" />
                  Context Window
                </span>
                <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                  {currentContext >= 1024 ? `${currentContext / 1024}k` : currentContext} tok
                </span>
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                RAM-allocated token budget for conversation history and retrieved memory facts.
              </p>
            </div>

            {!isRemoteLlm ? (
              <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[116px] sm:w-[136px]">
                {[2048, 4096, 8192].map((size) => {
                  const isSelected = currentContext === size;
                  return (
                    <button
                      key={size}
                      type="button"
                      onClick={() => updateDraft("llm", "context_window", size)}
                      className={cn(
                        "py-1 rounded-lg border text-[11px] font-mono font-bold transition-all duration-200 cursor-pointer flex items-center justify-center",
                        isSelected
                          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                          : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                      )}
                    >
                      {size / 1024}k
                    </button>
                  );
                })}
                <div
                  className={cn(
                    "rounded-lg border flex items-center justify-center transition-all overflow-hidden",
                    ![2048, 4096, 8192].includes(currentContext)
                      ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                      : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                  )}
                >
                  <input
                    type="text"
                    inputMode="numeric"
                    value={![2048, 4096, 8192].includes(currentContext) ? `${currentContext}` : ""}
                    onChange={(e) => {
                      const clean = e.target.value.replace(/[^0-9]/g, "");
                      if (!clean) return;
                      const num = parseInt(clean, 10);
                      if (!isNaN(num) && num >= 512 && num <= 131072) {
                        updateDraft("llm", "context_window", num);
                      }
                    }}
                    placeholder="Custom"
                    className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                  />
                </div>
              </div>
            ) : (
              <span className="text-[10px] sm:text-[11px] font-bold text-emerald-400 flex items-center gap-1 shrink-0">
                <Check size={13} /> Managed
              </span>
            )}
          </div>
        )}

        {/* TAB 4: CREATIVITY / TEMP - Side-by-Side Ergonomics with 2x2 Grid */}
        {activeSubTab === "creativity" && (
          <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] flex items-center gap-1.5">
                  <WandSparkles size={14} className="text-[rgb(var(--accent))]" />
                  Creativity
                </span>
                <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                  {currentTemp.toFixed(2)}
                </span>
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                Sampling temperature. Lower values produce strict facts; higher values encourage conversational flair.
              </p>
            </div>

            {/* 2x2 Grid: [0.2, 0.7, 1.0, Custom] */}
            <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[116px] sm:w-[136px]">
              {[
                { label: "0.2", val: 0.2 },
                { label: "0.7", val: 0.7 },
                { label: "1.0", val: 1.0 },
              ].map(({ label, val }) => {
                const isSelected = Math.abs(currentTemp - val) < 0.05;
                return (
                  <button
                    key={label}
                    type="button"
                    onClick={() => updateDraft("llm", "temperature", val)}
                    className={cn(
                      "py-1 rounded-lg border text-[11px] font-mono font-bold transition-all duration-200 cursor-pointer flex items-center justify-center",
                      isSelected
                        ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                        : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    {label}
                  </button>
                );
              })}
              <div
                className={cn(
                  "rounded-lg border flex items-center justify-center transition-all overflow-hidden",
                  ![0.2, 0.7, 1.0].some((v) => Math.abs(currentTemp - v) < 0.05)
                    ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                    : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                )}
              >
                <input
                  type="text"
                  inputMode="decimal"
                  value={
                    ![0.2, 0.7, 1.0].some((v) => Math.abs(currentTemp - v) < 0.05)
                      ? currentTemp.toFixed(2)
                      : ""
                  }
                  onChange={(e) => {
                    const clean = e.target.value.replace(/[^0-9.]/g, "");
                    if (!clean) return;
                    const num = parseFloat(clean);
                    if (!isNaN(num) && num >= 0.0 && num <= 2.0) {
                      updateDraft("llm", "temperature", num);
                    }
                  }}
                  placeholder="Custom"
                  className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                />
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
});

LlmSettingsView.displayName = "LlmSettingsView";
