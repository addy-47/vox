import { useState, useMemo, memo } from "react";
import { useSettingsStore, LlmProviderConfig } from "@/store/settingsStore";
import {
  Cpu, Zap, Battery, Sparkles, Check, Loader2, Gauge, Layers, Server, Plus, X
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { validateLlmTokenCap } from "@/services/settingsService";

export interface LlmSettingsViewProps {
  layoutMode?: "full-max" | "full-min" | "small";
  isRemoteLlm: boolean;
  provider?: LlmProviderConfig;
}

type SettingsSubTab = "compute" | "tokens" | "context" | "creativity";

export const LlmSettingsView = memo(({
  layoutMode,
  isRemoteLlm,
  provider,
}: LlmSettingsViewProps) => {
  const [activeSubTab, setActiveSubTab] = useState<SettingsSubTab>("compute");
  const llmSettings = useSettingsStore((s) => s.draftSettings?.llm);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  // Toggle state for custom input fields per tab
  const [showCustomTokens, setShowCustomTokens] = useState(false);
  const [showCustomContext, setShowCustomContext] = useState(false);
  const [showCustomTemp, setShowCustomTemp] = useState(false);

  // Custom token cap state and smoke test
  const [customTokenInput, setCustomTokenInput] = useState<string>(
    llmSettings?.max_output_tokens &&
      llmSettings.max_output_tokens > 0 &&
      llmSettings.max_output_tokens !== 300 &&
      llmSettings.max_output_tokens !== 1000
      ? String(llmSettings.max_output_tokens)
      : ""
  );
  const [customContextInput, setCustomContextInput] = useState<string>(
    llmSettings?.context_window &&
      ![2048, 4096, 8192, 16384].includes(llmSettings.context_window)
      ? String(llmSettings.context_window)
      : ""
  );
  const [customTempInput, setCustomTempInput] = useState<string>(
    llmSettings?.temperature !== undefined &&
      ![0.2, 0.7, 1.0].includes(llmSettings.temperature)
      ? String(llmSettings.temperature)
      : ""
  );

  const [isVerifyingCap, setIsVerifyingCap] = useState(false);
  const [capValidationResult, setCapValidationResult] = useState<{
    status: "valid" | "exceeded" | "error" | null;
    serverCeiling?: number;
    message?: string;
  }>({ status: null });

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

  // Determine token budget preset (default: 300 / concise)
  const currentMaxTokens = llmSettings?.max_output_tokens ?? 300;
  const currentTokenPreset = useMemo(() => {
    if (currentMaxTokens === 300) return "concise";
    if (currentMaxTokens === 1000) return "conversational";
    if (currentMaxTokens === 0) return "native";
    return "custom";
  }, [currentMaxTokens]);

  // Determine creativity / temperature preset (default: 0.7 / balanced)
  const currentTemp = llmSettings?.temperature ?? 0.7;
  const currentTempPreset = useMemo(() => {
    if (Math.abs(currentTemp - 0.2) < 0.05) return "precise";
    if (Math.abs(currentTemp - 0.7) < 0.05) return "balanced";
    if (Math.abs(currentTemp - 1.0) < 0.05) return "creative";
    return "custom";
  }, [currentTemp]);
  const currentContext = llmSettings?.context_window ?? 2048;

  const handleVerifyCustomCap = async (capVal: number) => {
    if (!capVal || capVal <= 0) return;
    setIsVerifyingCap(true);
    setCapValidationResult({ status: null });

    try {
      const ceiling = await validateLlmTokenCap(
        provider,
        provider && "model" in provider ? provider.model : undefined,
        capVal
      );

      if (ceiling === null) {
        setCapValidationResult({ status: "valid" });
      } else {
        setCapValidationResult({
          status: "exceeded",
          serverCeiling: ceiling,
          message: `Server max: ${ceiling.toLocaleString()} tokens`,
        });
      }
    } catch (err: unknown) {
      const errMsg = typeof err === "string" ? err : String(err);
      setCapValidationResult({
        status: "error",
        message: errMsg,
      });
    } finally {
      setIsVerifyingCap(false);
    }
  };

  if (!llmSettings) return null;

  const providerName = provider && "provider_name" in provider ? provider.provider_name : "";
  const isCloudProvider =
    isRemoteLlm &&
    (providerName?.toLowerCase().includes("nvidia") ||
      providerName?.toLowerCase().includes("groq") ||
      providerName?.toLowerCase().includes("openrouter") ||
      providerName?.toLowerCase().includes("together") ||
      providerName?.toLowerCase().includes("openai") ||
      providerName?.toLowerCase().includes("gemini") ||
      providerName?.toLowerCase().includes("mistral"));

  const tabs: Array<{ id: SettingsSubTab; label: string }> = [
    { id: "compute", label: "Compute" },
    { id: "tokens", label: "Tokens" },
    { id: "context", label: "Context" },
    { id: "creativity", label: "Temp" },
  ];

  return (
    <div className="w-full flex-1 flex flex-col justify-between select-none animate-fade-in">
      {/* ─── Layer 1: Subtab Navigation (Full-Width Distributed Tabs) ─── */}
      <div className="w-full flex items-center justify-between pt-0.5 pb-2 shrink-0 border-b border-[rgba(var(--accent),0.08)] mb-2 px-0.5">
        {tabs.map((tab, idx, arr) => {
          const isActive = activeSubTab === tab.id;
          return (
            <div key={tab.id} className="flex-1 flex items-center justify-center">
              <button
                type="button"
                onClick={() => setActiveSubTab(tab.id)}
                className={cn(
                  "w-full flex items-center justify-center pb-1 border-b-2 transition-all duration-200 bg-transparent text-[11px] sm:text-[12px] font-black uppercase tracking-[0.08em] sm:tracking-[0.12em] outline-none cursor-pointer text-center",
                  isActive
                    ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]"
                )}
              >
                <span>{tab.label}</span>
              </button>
              {idx < arr.length - 1 && (
                <span className="text-[11px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/25 font-light select-none pb-1 shrink-0 px-1 sm:px-2">
                  |
                </span>
              )}
            </div>
          );
        })}
      </div>

      {/* ─── Layer 2: Workspace Content (Single Surface) ─── */}
      <div
        className={cn(
          "w-full flex flex-col flex-1 min-h-0 pt-0.5 pb-0.5 justify-between",
          layoutMode === "small"
            ? "h-auto max-h-none py-1 space-y-2.5"
            : "h-[128px] max-h-[128px]"
        )}
      >
        {/* TAB 1: COMPUTE */}
        {activeSubTab === "compute" && (
          <div className="flex flex-col gap-2 h-full justify-between animate-fade-in p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)]">
            {!isRemoteLlm ? (
              <>
                <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1.5 shrink-0">
                  <span className="font-bold text-[11px] sm:text-[11.5px] uppercase tracking-wider text-[rgb(var(--foreground))]/80 flex items-center gap-1.5">
                    <Cpu size={14} className="text-[rgb(var(--accent))]" />
                    CPU Thread Allocation
                  </span>
                  <span className="text-[10px] sm:text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                    {currentThreads} / {totalCores} Cores
                  </span>
                </div>

                <div className="grid grid-cols-3 gap-1.5 sm:gap-2.5 flex-1 items-center pb-0.5">
                  <button
                    type="button"
                    onClick={() => updateDraft("llm", "threads", optimalThreads)}
                    className={cn(
                      "p-2 sm:p-2.5 rounded-xl border text-left transition-all duration-200 cursor-pointer flex flex-col justify-between min-h-[46px] sm:h-[54px]",
                      currentProfile === "auto"
                        ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30"
                        : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.08)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.02)]"
                    )}
                  >
                    <div className="flex items-center justify-between">
                      <span className="font-bold text-[12px] sm:text-[13px] flex items-center gap-1 text-[rgb(var(--foreground))]">
                        <Zap size={13} className="text-[rgb(var(--accent))] shrink-0" /> Auto
                      </span>
                      {currentProfile === "auto" && <Check size={13} className="text-[rgb(var(--accent))] shrink-0" />}
                    </div>
                    <span className="text-[10px] sm:text-[11px] text-[rgb(var(--foreground-muted))] font-medium truncate">
                      {optimalThreads} cores (Balanced)
                    </span>
                  </button>

                  <button
                    type="button"
                    onClick={() => updateDraft("llm", "threads", powerSaverThreads)}
                    className={cn(
                      "p-2 sm:p-2.5 rounded-xl border text-left transition-all duration-200 cursor-pointer flex flex-col justify-between min-h-[46px] sm:h-[54px]",
                      currentProfile === "power"
                        ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30"
                        : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.08)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.02)]"
                    )}
                  >
                    <div className="flex items-center justify-between">
                      <span className="font-bold text-[12px] sm:text-[13px] flex items-center gap-1 text-[rgb(var(--foreground))]">
                        <Battery size={13} className="text-emerald-400 shrink-0" /> Eco
                      </span>
                      {currentProfile === "power" && <Check size={13} className="text-[rgb(var(--accent))] shrink-0" />}
                    </div>
                    <span className="text-[10px] sm:text-[11px] text-[rgb(var(--foreground-muted))] font-medium truncate">
                      {powerSaverThreads} cores (Cooler)
                    </span>
                  </button>

                  <button
                    type="button"
                    onClick={() => updateDraft("llm", "threads", totalCores)}
                    className={cn(
                      "p-2 sm:p-2.5 rounded-xl border text-left transition-all duration-200 cursor-pointer flex flex-col justify-between min-h-[46px] sm:h-[54px]",
                      currentProfile === "max"
                        ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30"
                        : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.08)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.02)]"
                    )}
                  >
                    <div className="flex items-center justify-between">
                      <span className="font-bold text-[12px] sm:text-[13px] flex items-center gap-1 text-[rgb(var(--foreground))]">
                        <Cpu size={13} className="text-amber-400 shrink-0" /> Max
                      </span>
                      {currentProfile === "max" && <Check size={13} className="text-[rgb(var(--accent))] shrink-0" />}
                    </div>
                    <span className="text-[10px] sm:text-[11px] text-[rgb(var(--foreground-muted))] font-medium truncate">
                      {totalCores} cores (Full)
                    </span>
                  </button>
                </div>
              </>
            ) : (
              <div className="flex flex-col justify-between h-full gap-2 animate-fade-in px-0.5">
                <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1 shrink-0 gap-2">
                  <span className="font-bold text-[11px] sm:text-[11.5px] uppercase tracking-wider text-[rgb(var(--foreground))]/80 flex items-center gap-1.5 truncate">
                    <Server size={14} className="text-[rgb(var(--accent))] shrink-0" />
                    <span className="truncate">
                      {isCloudProvider ? "Cloud Infrastructure" : "Remote Server Compute"}
                    </span>
                  </span>
                  <span className="text-[10px] sm:text-[11px] font-bold text-emerald-400 flex items-center gap-1 shrink-0">
                    <Check size={13} /> Active
                  </span>
                </div>
                <p className="text-[11px] sm:text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed font-medium">
                  Inference computation is offloaded entirely to the remote provider. Zero local CPU or RAM is consumed.
                </p>
              </div>
            )}
          </div>
        )}

        {/* TAB 2: TOKENS */}
        {activeSubTab === "tokens" && (
          <div className="flex flex-col gap-2 h-full justify-between animate-fade-in p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)]">
            {/* Header Line with Custom Cap Trigger */}
            <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1.5 shrink-0 gap-2">
              <span className="font-bold text-[11px] sm:text-[11.5px] uppercase tracking-wider text-[rgb(var(--foreground))]/80 flex items-center gap-1.5 truncate">
                <Sparkles size={14} className="text-[rgb(var(--accent))] shrink-0" />
                <span className="truncate">Response Token Limit</span>
              </span>

              <div className="flex items-center gap-1.5 shrink-0">
                {showCustomTokens ? (
                  <div className="flex items-center gap-1 border-b border-[rgb(var(--accent))] pb-0.5 animate-fade-in max-w-[180px] sm:max-w-xs">
                    <input
                      type="number"
                      min="1"
                      max="128000"
                      autoFocus
                      value={customTokenInput}
                      onChange={(e) => {
                        setCustomTokenInput(e.target.value);
                        const val = parseInt(e.target.value, 10);
                        if (!isNaN(val) && val > 0) {
                          updateDraft("llm", "max_output_tokens", val);
                          setCapValidationResult({ status: null });
                        }
                      }}
                      onKeyDown={(e) => {
                        if (e.key === "Escape") {
                          setShowCustomTokens(false);
                        }
                      }}
                      placeholder="e.g. 4096"
                      className="w-20 sm:w-36 bg-transparent border-none outline-none text-[11px] sm:text-[12px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                    />
                    {isRemoteLlm && (
                      <button
                        type="button"
                        disabled={!customTokenInput || isVerifyingCap}
                        onClick={() => handleVerifyCustomCap(parseInt(customTokenInput, 10))}
                        className="text-[11px] font-bold text-[rgb(var(--accent))] hover:underline cursor-pointer disabled:opacity-40 shrink-0"
                        title="Smoke Test"
                      >
                        {isVerifyingCap ? <Loader2 size={11} className="animate-spin" /> : "Test"}
                      </button>
                    )}
                    {capValidationResult.status === "valid" && (
                      <span title="Valid cap verified" className="shrink-0">
                        <Check size={13} className="text-emerald-400" />
                      </span>
                    )}
                    {capValidationResult.status === "exceeded" && capValidationResult.serverCeiling && (
                      <button
                        type="button"
                        onClick={() => {
                          if (capValidationResult.serverCeiling) {
                            setCustomTokenInput(String(capValidationResult.serverCeiling));
                            updateDraft("llm", "max_output_tokens", capValidationResult.serverCeiling);
                            setCapValidationResult({ status: "valid" });
                          }
                        }}
                        className="text-[11px] underline text-amber-300 font-bold cursor-pointer shrink-0"
                        title="Auto clamp to server ceiling"
                      >
                        Clamp
                      </button>
                    )}
                    <button
                      type="button"
                      onClick={() => setShowCustomTokens(false)}
                      className="p-0.5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer shrink-0"
                    >
                      <X size={13} />
                    </button>
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => setShowCustomTokens(true)}
                    className="flex items-center gap-1 text-[10px] sm:text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] hover:brightness-125 px-1.5 py-0.5 bg-[rgb(var(--accent))]/5 hover:bg-[rgb(var(--accent))]/10 transition-all cursor-pointer rounded"
                    title="Set custom token cap"
                  >
                    <Plus size={11} strokeWidth={2.5} />
                    <span>Custom</span>
                  </button>
                )}
              </div>
            </div>

            {/* Presets */}
            <div className="grid grid-cols-3 gap-1.5 sm:gap-2.5 flex-1 items-center pb-0.5">
              <button
                type="button"
                onClick={() => {
                  updateDraft("llm", "max_output_tokens", 300);
                  setCustomTokenInput("");
                  setCapValidationResult({ status: null });
                }}
                className={cn(
                  "p-2 sm:p-2.5 rounded-xl border text-left transition-all cursor-pointer flex flex-col justify-between min-h-[46px] sm:h-[54px]",
                  currentTokenPreset === "concise"
                    ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30"
                    : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.08)] hover:border-[rgba(var(--accent),0.3)]"
                )}
              >
                <div className="flex items-center justify-between">
                  <span className="font-bold text-[12px] sm:text-[13px] text-[rgb(var(--foreground))]">Concise</span>
                  {currentTokenPreset === "concise" && <Check size={13} className="text-[rgb(var(--accent))]" />}
                </div>
                <span className="text-[10px] sm:text-[11px] text-[rgb(var(--foreground-muted))] font-medium truncate">~300 tokens (Fast)</span>
              </button>

              <button
                type="button"
                onClick={() => {
                  updateDraft("llm", "max_output_tokens", 1000);
                  setCustomTokenInput("");
                  setCapValidationResult({ status: null });
                }}
                className={cn(
                  "p-2 sm:p-2.5 rounded-xl border text-left transition-all cursor-pointer flex flex-col justify-between min-h-[46px] sm:h-[54px]",
                  currentTokenPreset === "conversational"
                    ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30"
                    : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.08)] hover:border-[rgba(var(--accent),0.3)]"
                )}
              >
                <div className="flex items-center justify-between">
                  <span className="font-bold text-[12px] sm:text-[13px] text-[rgb(var(--foreground))]">Convo</span>
                  {currentTokenPreset === "conversational" && <Check size={13} className="text-[rgb(var(--accent))]" />}
                </div>
                <span className="text-[10px] sm:text-[11px] text-[rgb(var(--foreground-muted))] font-medium truncate">~1,000 tokens</span>
              </button>

              <button
                type="button"
                onClick={() => {
                  updateDraft("llm", "max_output_tokens", 0);
                  setCustomTokenInput("");
                  setCapValidationResult({ status: null });
                }}
                className={cn(
                  "p-2 sm:p-2.5 rounded-xl border text-left transition-all cursor-pointer flex flex-col justify-between min-h-[46px] sm:h-[54px]",
                  currentTokenPreset === "native"
                    ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30"
                    : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.08)] hover:border-[rgba(var(--accent),0.3)]"
                )}
              >
                <div className="flex items-center justify-between">
                  <span className="font-bold text-[12px] sm:text-[13px] text-[rgb(var(--foreground))]">Native</span>
                  {currentTokenPreset === "native" && <Check size={13} className="text-[rgb(var(--accent))]" />}
                </div>
                <span className="text-[10px] sm:text-[11px] text-[rgb(var(--foreground-muted))] font-medium truncate">Uncapped full</span>
              </button>
            </div>
          </div>
        )}

        {/* TAB 3: CONTEXT */}
        {activeSubTab === "context" && (
          <div className="flex flex-col gap-2 h-full justify-between animate-fade-in p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)]">
            <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1.5 shrink-0 gap-2">
              <span className="font-bold text-[11px] sm:text-[11.5px] uppercase tracking-wider text-[rgb(var(--foreground))]/80 flex items-center gap-1.5 truncate">
                <Layers size={14} className="text-[rgb(var(--accent))] shrink-0" />
                <span className="truncate">Context Window Budget</span>
              </span>

              {!isRemoteLlm ? (
                <div className="flex items-center gap-1.5 shrink-0">
                  {showCustomContext ? (
                    <div className="flex items-center gap-1 border-b border-[rgb(var(--accent))] pb-0.5 animate-fade-in max-w-[180px] sm:max-w-xs">
                      <input
                        type="number"
                        min="512"
                        max="2000000"
                        step="512"
                        autoFocus
                        value={customContextInput}
                        onChange={(e) => {
                          setCustomContextInput(e.target.value);
                          const parsed = parseInt(e.target.value, 10);
                          if (!isNaN(parsed) && parsed > 0) {
                            updateDraft("llm", "context_window", parsed);
                          }
                        }}
                        placeholder="e.g. 8192"
                        className="w-20 sm:w-36 bg-transparent border-none outline-none text-[11px] sm:text-[12px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                      />
                      <button
                        type="button"
                        onClick={() => setShowCustomContext(false)}
                        className="p-0.5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer shrink-0"
                      >
                        <X size={13} />
                      </button>
                    </div>
                  ) : (
                    <button
                      type="button"
                      onClick={() => setShowCustomContext(true)}
                      className="flex items-center gap-1 text-[10px] sm:text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] hover:brightness-125 px-1.5 py-0.5 bg-[rgb(var(--accent))]/5 hover:bg-[rgb(var(--accent))]/10 transition-all cursor-pointer rounded"
                      title="Set custom context window"
                    >
                      <Plus size={11} strokeWidth={2.5} />
                      <span>Custom</span>
                    </button>
                  )}
                </div>
              ) : (
                <span className="text-[10px] sm:text-[11px] font-bold text-emerald-400 flex items-center gap-1 shrink-0">
                  <Check size={13} /> Provider Managed
                </span>
              )}
            </div>

            {!isRemoteLlm ? (
              <div className="grid grid-cols-4 gap-1.5 sm:gap-2.5 flex-1 items-center pb-0.5">
                {[2048, 4096, 8192, 16384].map((size) => {
                  const isSelected = currentContext === size;
                  return (
                    <button
                      key={size}
                      type="button"
                      onClick={() => {
                        updateDraft("llm", "context_window", size);
                        setCustomContextInput("");
                      }}
                      className={cn(
                        "py-1.5 sm:py-2 px-1 rounded-xl border text-center transition-all cursor-pointer font-mono font-bold text-[12px] sm:text-[13px] min-h-[44px] sm:h-[52px] flex flex-col items-center justify-center",
                        isSelected
                          ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] text-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30"
                          : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.08)] text-[rgb(var(--foreground-muted))] hover:border-[rgba(var(--accent),0.3)] hover:text-[rgb(var(--foreground))]"
                      )}
                    >
                      <span>{size >= 1024 ? `${size / 1024}k` : size}</span>
                      <span className="text-[10px] sm:text-[11px] font-sans font-normal opacity-70">Budget</span>
                    </button>
                  );
                })}
              </div>
            ) : (
              <div className="flex flex-col justify-center gap-1.5 h-full animate-fade-in px-0.5 py-1">
                <p className="text-[11px] sm:text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed font-medium">
                  The remote endpoint dynamically manages architectural context capacity with zero client-side truncation.
                </p>
              </div>
            )}
          </div>
        )}

        {/* TAB 4: CREATIVITY / TEMP */}
        {activeSubTab === "creativity" && (
          <div className="flex flex-col gap-2 h-full justify-between animate-fade-in p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)]">
            <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1.5 shrink-0 gap-2">
              <span className="font-bold text-[11px] sm:text-[11.5px] uppercase tracking-wider text-[rgb(var(--foreground))]/80 flex items-center gap-1.5 truncate">
                <Gauge size={14} className="text-[rgb(var(--accent))] shrink-0" />
                <span className="truncate">Temperature (Creativity)</span>
              </span>

              <div className="flex items-center gap-1.5 shrink-0">
                {showCustomTemp ? (
                  <div className="flex items-center gap-1 border-b border-[rgb(var(--accent))] pb-0.5 animate-fade-in max-w-[180px] sm:max-w-xs">
                    <input
                      type="number"
                      min="0.0"
                      max="2.0"
                      step="0.05"
                      autoFocus
                      value={customTempInput}
                      onChange={(e) => {
                        setCustomTempInput(e.target.value);
                        const parsed = parseFloat(e.target.value);
                        if (!isNaN(parsed) && parsed >= 0) {
                          updateDraft("llm", "temperature", parsed);
                        }
                      }}
                      onKeyDown={(e) => {
                        if (e.key === "Escape") {
                          setShowCustomTemp(false);
                        }
                      }}
                      placeholder="e.g. 0.70"
                      className="w-20 sm:w-36 bg-transparent border-none outline-none text-[11px] sm:text-[12px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                    />
                    <button
                      type="button"
                      onClick={() => setShowCustomTemp(false)}
                      className="p-0.5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer shrink-0"
                    >
                      <X size={13} />
                    </button>
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => setShowCustomTemp(true)}
                    className="flex items-center gap-1 text-[10px] sm:text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] hover:brightness-125 px-1.5 py-0.5 bg-[rgb(var(--accent))]/5 hover:bg-[rgb(var(--accent))]/10 transition-all cursor-pointer rounded"
                    title="Set custom temperature"
                  >
                    <Plus size={11} strokeWidth={2.5} />
                    <span>Custom</span>
                  </button>
                )}
              </div>
            </div>

            <div className="grid grid-cols-3 gap-1.5 sm:gap-2.5 flex-1 items-center pb-0.5">
              <button
                type="button"
                onClick={() => {
                  updateDraft("llm", "temperature", 0.2);
                  setCustomTempInput("");
                }}
                className={cn(
                  "p-2 sm:p-2.5 rounded-xl border text-left transition-all cursor-pointer flex flex-col justify-between min-h-[46px] sm:h-[54px]",
                  currentTempPreset === "precise"
                    ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30"
                    : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.08)] hover:border-[rgba(var(--accent),0.3)]"
                )}
              >
                <div className="flex items-center justify-between">
                  <span className="font-bold text-[12px] sm:text-[13px] text-[rgb(var(--foreground))]">Precise</span>
                  {currentTempPreset === "precise" && <Check size={13} className="text-[rgb(var(--accent))]" />}
                </div>
                <span className="text-[10px] sm:text-[11px] text-[rgb(var(--foreground-muted))] font-medium truncate">0.2 (Factual)</span>
              </button>

              <button
                type="button"
                onClick={() => {
                  updateDraft("llm", "temperature", 0.7);
                  setCustomTempInput("");
                }}
                className={cn(
                  "p-2 sm:p-2.5 rounded-xl border text-left transition-all cursor-pointer flex flex-col justify-between min-h-[46px] sm:h-[54px]",
                  currentTempPreset === "balanced"
                    ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30"
                    : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.08)] hover:border-[rgba(var(--accent),0.3)]"
                )}
              >
                <div className="flex items-center justify-between">
                  <span className="font-bold text-[12px] sm:text-[13px] text-[rgb(var(--foreground))]">Balanced</span>
                  {currentTempPreset === "balanced" && <Check size={13} className="text-[rgb(var(--accent))]" />}
                </div>
                <span className="text-[10px] sm:text-[11px] text-[rgb(var(--foreground-muted))] font-medium truncate">0.7 (Default)</span>
              </button>

              <button
                type="button"
                onClick={() => {
                  updateDraft("llm", "temperature", 1.0);
                  setCustomTempInput("");
                }}
                className={cn(
                  "p-2 sm:p-2.5 rounded-xl border text-left transition-all cursor-pointer flex flex-col justify-between min-h-[46px] sm:h-[54px]",
                  currentTempPreset === "creative"
                    ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30"
                    : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.08)] hover:border-[rgba(var(--accent),0.3)]"
                )}
              >
                <div className="flex items-center justify-between">
                  <span className="font-bold text-[12px] sm:text-[13px] text-[rgb(var(--foreground))]">Creative</span>
                  {currentTempPreset === "creative" && <Check size={13} className="text-[rgb(var(--accent))]" />}
                </div>
                <span className="text-[10px] sm:text-[11px] text-[rgb(var(--foreground-muted))] font-medium truncate">1.0 (Dynamic)</span>
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
});

LlmSettingsView.displayName = "LlmSettingsView";
