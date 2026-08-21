import { useState, useMemo, memo } from "react";
import { useSettingsStore, LlmProviderConfig } from "@/store/settingsStore";
import { Cpu, Zap, Battery, Sparkles, Check, AlertTriangle, ShieldCheck, Loader2, Gauge, Layers, Server } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { invoke } from "@tauri-apps/api/core";

export interface LlmSettingsViewProps {
  layoutMode?: "full-max" | "full-min" | "small";
  isRemoteLlm: boolean;
  provider?: LlmProviderConfig;
}

type SettingsSubTab = "performance" | "tokens" | "creativity";

export const LlmSettingsView = memo(({
  layoutMode,
  isRemoteLlm,
  provider,
}: LlmSettingsViewProps) => {
  const [activeSubTab, setActiveSubTab] = useState<SettingsSubTab>("performance");
  const llmSettings = useSettingsStore((s) => s.draftSettings?.llm);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  // Custom token cap state and smoke test
  const [customTokenInput, setCustomTokenInput] = useState<string>(
    llmSettings?.max_output_tokens &&
      llmSettings.max_output_tokens > 0 &&
      llmSettings.max_output_tokens !== 300 &&
      llmSettings.max_output_tokens !== 1000
      ? String(llmSettings.max_output_tokens)
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
  const currentContext = llmSettings?.context_window ?? 2048;

  const handleVerifyCustomCap = async (capVal: number) => {
    if (!capVal || capVal <= 0) return;
    setIsVerifyingCap(true);
    setCapValidationResult({ status: null });

    try {
      const ceiling = await invoke<number | null>("validate_llm_token_cap", {
        provider: provider,
        modelId: provider && "model" in provider ? provider.model : undefined,
        targetCap: capVal,
      });

      if (ceiling === null) {
        setCapValidationResult({ status: "valid" });
      } else {
        setCapValidationResult({
          status: "exceeded",
          serverCeiling: ceiling,
          message: `Server maximum is ${ceiling.toLocaleString()} tokens for this model.`,
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

  return (
    <div className="w-full h-full flex flex-col min-h-0 gap-2.5 animate-fade-in text-[rgb(var(--foreground))] select-none">
      {/* Flat Underline Tabs (Fixed / Sticky at Top) */}
      <div className="flex items-center justify-center gap-3.5 border-b border-[rgba(var(--foreground),0.06)] pb-1 shrink-0">
        <button
          type="button"
          onClick={() => setActiveSubTab("performance")}
          className={cn(
            "pb-1.5 border-b-2 text-[12px] font-bold transition-all cursor-pointer flex items-center gap-1.5",
            activeSubTab === "performance"
              ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
              : "text-[rgb(var(--foreground-muted))] border-transparent hover:text-[rgb(var(--foreground))]"
          )}
        >
          <Cpu size={14} />
          <span>Performance</span>
        </button>

        <span className="text-[rgba(var(--foreground),0.15)] text-[11px] font-light pb-1.5 select-none">|</span>

        <button
          type="button"
          onClick={() => setActiveSubTab("tokens")}
          className={cn(
            "pb-1.5 border-b-2 text-[12px] font-bold transition-all cursor-pointer flex items-center gap-1.5",
            activeSubTab === "tokens"
              ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
              : "text-[rgb(var(--foreground-muted))] border-transparent hover:text-[rgb(var(--foreground))]"
          )}
        >
          <Sparkles size={14} />
          <span>Tokens & Context</span>
        </button>

        <span className="text-[rgba(var(--foreground),0.15)] text-[11px] font-light pb-1.5 select-none">|</span>

        <button
          type="button"
          onClick={() => setActiveSubTab("creativity")}
          className={cn(
            "pb-1.5 border-b-2 text-[12px] font-bold transition-all cursor-pointer flex items-center gap-1.5",
            activeSubTab === "creativity"
              ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
              : "text-[rgb(var(--foreground-muted))] border-transparent hover:text-[rgb(var(--foreground))]"
          )}
        >
          <Gauge size={14} />
          <span>Creativity</span>
        </button>
      </div>

      {/* Tab Contents (Scrollable Area) */}
      <div
        className={cn(
          "flex-1 min-h-0 overflow-y-auto custom-scrollbar pr-1",
          layoutMode === "small" ? "max-h-[235px]" : ""
        )}
      >
        {/* TAB 1: PERFORMANCE */}
        {activeSubTab === "performance" && (
        <div className="space-y-3 animate-fade-in">
          {!isRemoteLlm ? (
            <div className="p-3.5 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--foreground),0.06)] space-y-3">
              <div className="flex items-center justify-between">
                <span className="font-bold text-[13px] tracking-tight">CPU Core Allocation</span>
                <span className="text-[11px] font-mono font-medium px-2 py-0.5 rounded-md bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))]">
                  {currentThreads} / {totalCores} Cores Active
                </span>
              </div>

              <p className="text-[11.5px] text-[rgb(var(--foreground-muted))]/80 leading-relaxed">
                Calibrates processor core utilization during local token generation to prevent thermal throttling and maintain audio headroom.
              </p>

              <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
                <button
                  type="button"
                  onClick={() => updateDraft("llm", "threads", optimalThreads)}
                  className={cn(
                    "p-2.5 rounded-xl border text-left transition-all duration-200 cursor-pointer flex flex-col justify-between",
                    currentProfile === "auto"
                      ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.1)] ring-1 ring-[rgb(var(--accent))]/30"
                      : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.02)]"
                  )}
                >
                  <div className="flex items-center justify-between">
                    <span className="font-bold text-[12.5px] flex items-center gap-1.5">
                      <Zap size={13} className="text-[rgb(var(--accent))]" /> Auto
                    </span>
                    {currentProfile === "auto" && <Check size={13} className="text-[rgb(var(--accent))]" />}
                  </div>
                  <span className="text-[10.5px] text-[rgb(var(--foreground-muted))]/70 mt-1">
                    {optimalThreads} cores (Headroom protected)
                  </span>
                </button>

                <button
                  type="button"
                  onClick={() => updateDraft("llm", "threads", powerSaverThreads)}
                  className={cn(
                    "p-2.5 rounded-xl border text-left transition-all duration-200 cursor-pointer flex flex-col justify-between",
                    currentProfile === "power"
                      ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.1)] ring-1 ring-[rgb(var(--accent))]/30"
                      : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.02)]"
                  )}
                >
                  <div className="flex items-center justify-between">
                    <span className="font-bold text-[12.5px] flex items-center gap-1.5">
                      <Battery size={13} className="text-emerald-400" /> Power Saver
                    </span>
                    {currentProfile === "power" && <Check size={13} className="text-[rgb(var(--accent))]" />}
                  </div>
                  <span className="text-[10.5px] text-[rgb(var(--foreground-muted))]/70 mt-1">
                    {powerSaverThreads} cores (Cooler thermals)
                  </span>
                </button>

                <button
                  type="button"
                  onClick={() => updateDraft("llm", "threads", totalCores)}
                  className={cn(
                    "p-2.5 rounded-xl border text-left transition-all duration-200 cursor-pointer flex flex-col justify-between",
                    currentProfile === "max"
                      ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.1)] ring-1 ring-[rgb(var(--accent))]/30"
                      : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.02)]"
                  )}
                >
                  <div className="flex items-center justify-between">
                    <span className="font-bold text-[12.5px] flex items-center gap-1.5">
                      <Cpu size={13} className="text-amber-400" /> Maximum
                    </span>
                    {currentProfile === "max" && <Check size={13} className="text-[rgb(var(--accent))]" />}
                  </div>
                  <span className="text-[10.5px] text-[rgb(var(--foreground-muted))]/70 mt-1">
                    {totalCores} cores (Full saturation)
                  </span>
                </button>
              </div>
            </div>
          ) : (
            <div className="p-3.5 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--foreground),0.06)] space-y-2.5">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Server size={15} className="text-[rgb(var(--accent))]" />
                  <span className="font-bold text-[13px] tracking-tight">Compute Offload</span>
                </div>
                <span className="text-[11px] font-mono font-medium px-2 py-0.5 rounded-md bg-[rgba(var(--accent),0.08)] text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/20">
                  {isCloudProvider ? "Cloud Infrastructure" : "Remote Server"}
                </span>
              </div>

              <p className="text-[11.5px] text-[rgb(var(--foreground-muted))]/80 leading-relaxed">
                Inference computation is offloaded entirely to the remote provider. Zero local CPU cores or RAM are consumed during token generation.
              </p>

              <div className="p-3 rounded-lg bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-between text-[11.5px]">
                <span className="text-[rgb(var(--foreground-muted))]">Hardware Acceleration Status</span>
                <span className="font-bold text-emerald-400 flex items-center gap-1.5">
                  <Check size={13} /> Active & Offloaded
                </span>
              </div>
            </div>
          )}
        </div>
      )}

      {/* TAB 2: TOKENS & CONTEXT */}
      {activeSubTab === "tokens" && (
        <div className="space-y-3 animate-fade-in">
          {/* Context Window Transparency */}
          <div className="p-3.5 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--foreground),0.06)] space-y-2.5">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Layers size={15} className="text-[rgb(var(--accent))]" />
                <span className="font-bold text-[13px] tracking-tight">Context Window</span>
              </div>
              <span className="text-[11px] font-mono font-medium px-2 py-0.5 rounded-md bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))]">
                {!isRemoteLlm ? `${currentContext.toLocaleString()} Tokens` : "Provider Managed"}
              </span>
            </div>

            {!isRemoteLlm ? (
              <>
                <p className="text-[11.5px] text-[rgb(var(--foreground-muted))]/80 leading-relaxed">
                  Memory allocation budget for conversation history and retrieved knowledge context.
                </p>
                <div className="grid grid-cols-4 gap-2">
                  {[2048, 4096, 8192, 16384].map((size) => {
                    const isSelected = currentContext === size;
                    return (
                      <button
                        key={size}
                        type="button"
                        onClick={() => {
                          updateDraft("llm", "context_window", size);
                        }}
                        className={cn(
                          "p-2 rounded-xl border text-center transition-all cursor-pointer font-mono font-bold text-[12px]",
                          isSelected
                            ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] text-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30"
                            : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] hover:border-[rgba(var(--accent),0.3)]"
                        )}
                      >
                        {size >= 1024 ? `${size / 1024}k` : size}
                      </button>
                    );
                  })}
                </div>
              </>
            ) : (
              <p className="text-[11.5px] text-[rgb(var(--foreground-muted))]/80 leading-relaxed">
                The remote endpoint manages architectural context limits dynamically with zero client-side clamping.
              </p>
            )}
          </div>

          {/* Response Token Limit */}
          <div className="p-3.5 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--foreground),0.06)] space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Sparkles size={15} className="text-[rgb(var(--accent))]" />
                <span className="font-bold text-[13px] tracking-tight">Response Limit</span>
              </div>
              <span className="text-[11px] font-mono font-medium px-2 py-0.5 rounded-md bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))]">
                {currentTokenPreset === "concise"
                  ? "Voice Concise (300)"
                  : currentTokenPreset === "conversational"
                  ? "Conversational (1,000)"
                  : currentTokenPreset === "native"
                  ? "Native (Uncapped)"
                  : `${currentMaxTokens.toLocaleString()} tokens`}
              </span>
            </div>

            <p className="text-[11.5px] text-[rgb(var(--foreground-muted))]/80 leading-relaxed">
              Caps response generation lengths to ensure natural, fast voice turns.
            </p>

            <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
              <button
                type="button"
                onClick={() => {
                  updateDraft("llm", "max_output_tokens", 300);
                  setCustomTokenInput("");
                  setCapValidationResult({ status: null });
                }}
                className={cn(
                  "p-2.5 rounded-xl border text-left transition-all duration-200 cursor-pointer flex flex-col justify-between",
                  currentTokenPreset === "concise"
                    ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.1)] ring-1 ring-[rgb(var(--accent))]/30"
                    : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.02)]"
                )}
              >
                <div className="flex items-center justify-between">
                  <span className="font-bold text-[12.5px]">Voice Concise</span>
                  {currentTokenPreset === "concise" && <Check size={13} className="text-[rgb(var(--accent))]" />}
                </div>
                <span className="text-[10.5px] text-[rgb(var(--foreground-muted))]/70 mt-1">~300 tokens (Snappy voice replies)</span>
              </button>

              <button
                type="button"
                onClick={() => {
                  updateDraft("llm", "max_output_tokens", 1000);
                  setCustomTokenInput("");
                  setCapValidationResult({ status: null });
                }}
                className={cn(
                  "p-2.5 rounded-xl border text-left transition-all duration-200 cursor-pointer flex flex-col justify-between",
                  currentTokenPreset === "conversational"
                    ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.1)] ring-1 ring-[rgb(var(--accent))]/30"
                    : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.02)]"
                )}
              >
                <div className="flex items-center justify-between">
                  <span className="font-bold text-[12.5px]">Conversational</span>
                  {currentTokenPreset === "conversational" && <Check size={13} className="text-[rgb(var(--accent))]" />}
                </div>
                <span className="text-[10.5px] text-[rgb(var(--foreground-muted))]/70 mt-1">~1,000 tokens (Balanced depth)</span>
              </button>

              <button
                type="button"
                onClick={() => {
                  updateDraft("llm", "max_output_tokens", 0);
                  setCustomTokenInput("");
                  setCapValidationResult({ status: null });
                }}
                className={cn(
                  "p-2.5 rounded-xl border text-left transition-all duration-200 cursor-pointer flex flex-col justify-between",
                  currentTokenPreset === "native"
                    ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.1)] ring-1 ring-[rgb(var(--accent))]/30"
                    : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.02)]"
                )}
              >
                <div className="flex items-center justify-between">
                  <span className="font-bold text-[12.5px]">Native Full</span>
                  {currentTokenPreset === "native" && <Check size={13} className="text-[rgb(var(--accent))]" />}
                </div>
                <span className="text-[10.5px] text-[rgb(var(--foreground-muted))]/70 mt-1">Uncapped output capacity</span>
              </button>
            </div>

            {/* Custom Token Cap Input with Instant Smoke Test */}
            <div className="pt-2 border-t border-[rgba(var(--foreground),0.04)] space-y-2">
              <div className="flex items-center gap-2">
                <input
                  type="number"
                  min="64"
                  max="2000000"
                  step="256"
                  value={customTokenInput}
                  onChange={(e) => {
                    setCustomTokenInput(e.target.value);
                    const parsed = parseInt(e.target.value, 10);
                    if (!isNaN(parsed) && parsed > 0) {
                      updateDraft("llm", "max_output_tokens", parsed);
                    }
                  }}
                  placeholder="Or enter custom limit (e.g. 16384)..."
                  className="flex-1 px-3 py-1.5 rounded-lg bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.08)] text-[12px] text-[rgb(var(--foreground))] outline-none focus:border-[rgb(var(--accent))] transition-all font-mono [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                />
                {isRemoteLlm && (
                  <button
                    type="button"
                    disabled={!customTokenInput || isVerifyingCap}
                    onClick={() => handleVerifyCustomCap(parseInt(customTokenInput, 10))}
                    className="px-3 py-1.5 rounded-lg text-[11.5px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/25 hover:bg-[rgb(var(--accent))]/20 disabled:opacity-40 disabled:cursor-not-allowed transition-all cursor-pointer flex items-center gap-1.5 shrink-0"
                  >
                    {isVerifyingCap ? <Loader2 size={13} className="animate-spin" /> : <ShieldCheck size={13} />}
                    <span>Smoke Test</span>
                  </button>
                )}
              </div>

              {/* Validation Feedback Badges */}
              {capValidationResult.status === "valid" && (
                <div className="text-[11px] font-bold text-emerald-400 bg-emerald-400/10 border border-emerald-400/20 rounded-lg px-2.5 py-1.5 flex items-center gap-1.5 animate-fade-in">
                  <Check size={13} className="shrink-0" />
                  <span>Valid Limit: Remote model accepted token cap with 0 errors.</span>
                </div>
              )}

              {capValidationResult.status === "exceeded" && (
                <div className="text-[11px] text-amber-300 bg-amber-400/10 border border-amber-400/20 rounded-lg px-2.5 py-1.5 flex items-center justify-between gap-2 animate-fade-in">
                  <div className="flex items-center gap-1.5">
                    <AlertTriangle size={13} className="text-amber-400 shrink-0" />
                    <span>{capValidationResult.message}</span>
                  </div>
                  {capValidationResult.serverCeiling && (
                    <button
                      type="button"
                      onClick={() => {
                        if (capValidationResult.serverCeiling) {
                          setCustomTokenInput(String(capValidationResult.serverCeiling));
                          updateDraft("llm", "max_output_tokens", capValidationResult.serverCeiling);
                          setCapValidationResult({ status: "valid" });
                        }
                      }}
                      className="px-2 py-0.5 rounded bg-amber-400/20 text-amber-200 hover:bg-amber-400/30 text-[10.5px] font-bold cursor-pointer transition-colors shrink-0"
                    >
                      Auto-clamp to {capValidationResult.serverCeiling.toLocaleString()}
                    </button>
                  )}
                </div>
              )}

              {capValidationResult.status === "error" && (
                <div className="text-[11px] text-red-400 bg-red-400/10 border border-red-400/20 rounded-lg px-2.5 py-1.5 flex items-center gap-1.5 animate-fade-in">
                  <AlertTriangle size={13} className="shrink-0 text-red-400" />
                  <span className="truncate">{capValidationResult.message}</span>
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* TAB 3: CREATIVITY */}
      {activeSubTab === "creativity" && (
        <div className="p-3.5 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--foreground),0.06)] space-y-3 animate-fade-in">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Gauge size={15} className="text-[rgb(var(--accent))]" />
              <span className="font-bold text-[13px] tracking-tight">Creativity (Temperature)</span>
            </div>
            <span className="text-[11px] font-mono font-medium px-2 py-0.5 rounded-md bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))]">
              {currentTemp.toFixed(2)}
            </span>
          </div>

          <p className="text-[11.5px] text-[rgb(var(--foreground-muted))]/80 leading-relaxed">
            Controls the balance between deterministic, factual accuracy and expressive conversation.
          </p>

          <div className="grid grid-cols-3 gap-2">
            {[
              { label: "Precise", val: 0.2, desc: "Factual & exact" },
              { label: "Balanced", val: 0.7, desc: "Natural conversation" },
              { label: "Creative", val: 1.0, desc: "Expressive & varied" },
            ].map((preset) => {
              const isSelected = Math.abs(currentTemp - preset.val) < 0.05;
              return (
                <button
                  key={preset.label}
                  type="button"
                  onClick={() => {
                    updateDraft("llm", "temperature", preset.val);
                  }}
                  className={cn(
                    "p-2.5 rounded-xl border text-center transition-all cursor-pointer flex flex-col justify-between",
                    isSelected
                      ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] text-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30"
                      : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] hover:border-[rgba(var(--accent),0.3)] hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  <div className="font-bold text-[12px]">{preset.label}</div>
                  <div className="text-[10px] opacity-70 mt-1">{preset.desc}</div>
                </button>
              );
            })}
          </div>
        </div>
      )}
      </div>
    </div>
  );
});
