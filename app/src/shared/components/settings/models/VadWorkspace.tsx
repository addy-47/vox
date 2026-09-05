import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { SubModelCard } from "../SubModelCard";
import { cn } from "@/shared/lib/utils";
import { VAD_SETTINGS_COPY, COMPUTE_PROFILE_COPY } from "@/data/settingsCopy";

interface VadWorkspaceProps {
  activeCategoryTab: "model" | "settings";
  activeSubTab?: string;
  layoutMode?: "full-max" | "full-min" | "small";
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, any>;
  startDownload: (id: string) => void;
  deleteModel: (id: string) => void;
}

export const VadWorkspace = memo(
  ({
    activeCategoryTab,
    activeSubTab = "sensitivity",
    layoutMode,
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
  }: VadWorkspaceProps) => {
    const vad = useSettingsStore((s) => s.draftSettings?.vad);
    const updateDraft = useSettingsStore((s) => s.updateDraft);
    const modelCatalog = useSettingsStore((s) => s.modelCatalog);

    if (!vad) return null;
    const activeVadBackend = vad.vad_backend || "earshot";
    const vadModels = modelCatalog?.vad || [];

    const currentThreshold = vad.threshold ?? 0.5;
    const currentSilenceMs = vad.silence_duration_ms ?? 800;
    const currentNoiseGate = vad.ptt_noise_gate ?? 0.005;

    return (
      <div className="flex-1 min-h-0 w-full overflow-y-auto custom-scrollbar pr-1">
        {activeCategoryTab === "model" ? (
          <div
            className={cn(
              "grid gap-2.5 h-full",
              vadModels.length <= 2
                ? (layoutMode === "small" ? "grid-cols-1 auto-rows-fr" : "grid-cols-2 grid-rows-1")
                : (layoutMode === "small" ? "grid-cols-1 auto-rows-full snap-y snap-mandatory" : "grid-cols-2 auto-rows-full snap-y snap-mandatory")
            )}
          >
            {vadModels.map((model) => {
              const isBuiltIn = !!model.is_built_in;
              const isDownloaded = isBuiltIn || !!modelPresence[model.id];
              const isActive = model.id === activeVadBackend;

              return (
                <SubModelCard
                  key={model.id}
                  id={model.id}
                  name={model.name}
                  description={model.description || ""}
                  parameters={model.parameters || (isBuiltIn ? "Built-in" : "ONNX")}
                  ramUsage={model.ram_usage || "0 MB"}
                  tradeoffs={model.tradeoffs || ""}
                  isDownloaded={isDownloaded}
                  isActive={isActive}
                  isRequired={isBuiltIn}
                  layoutMode={layoutMode}
                  onSelect={() => updateDraft("vad", "vad_backend", model.id)}
                  confirmDeleteId={confirmDeleteId}
                  setConfirmDeleteId={setConfirmDeleteId}
                  downloadStatus={downloadStatuses[model.id]}
                  startDownload={() => startDownload(model.id)}
                  deleteModel={() => deleteModel(model.id)}
                />
              );
            })}
          </div>
        ) : (
          <div className="w-full flex-1 flex flex-col justify-between select-none animate-fade-in h-full">
            <div
              className={cn(
                "w-full flex flex-col flex-1 min-h-0 pt-0.5 pb-0.5 justify-between",
                layoutMode === "small" ? "h-auto py-1" : "h-full"
              )}
            >
              {/* SUBTAB 1: SENSITIVITY (THRESHOLD) - MemoryCard Side-by-Side 2x2 Layout */}
              {activeSubTab === "sensitivity" && (
                <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
                  <div className="flex flex-col gap-1 min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                        {VAD_SETTINGS_COPY.sensitivity.title}
                      </span>
                      <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                        {Math.round(currentThreshold * 100)}%
                      </span>
                    </div>
                    <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                      {VAD_SETTINGS_COPY.sensitivity.description}
                    </p>
                  </div>

                  {/* 2x2 Grid: [30%, 50%, 70%, Custom] */}
                  <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[116px] sm:w-[136px]">
                    {[
                      { label: "30%", val: 0.3 },
                      { label: "50%", val: 0.5 },
                      { label: "70%", val: 0.7 },
                    ].map(({ label, val }) => {
                      const isSelected = Math.abs(currentThreshold - val) < 0.04;
                      return (
                        <button
                          key={val}
                          type="button"
                          onClick={() => updateDraft("vad", "threshold", val)}
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
                        ![0.3, 0.5, 0.7].some((v) => Math.abs(currentThreshold - v) < 0.04)
                          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                          : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                      )}
                    >
                      <input
                        type="text"
                        inputMode="numeric"
                        value={
                          ![0.3, 0.5, 0.7].some((v) => Math.abs(currentThreshold - v) < 0.04)
                            ? `${Math.round(currentThreshold * 100)}%`
                            : ""
                        }
                        onChange={(e) => {
                          const clean = e.target.value.replace(/[^0-9]/g, "");
                          if (!clean) return;
                          const num = parseInt(clean, 10);
                          if (!isNaN(num) && num >= 5 && num <= 95) {
                            updateDraft("vad", "threshold", num / 100);
                          }
                        }}
                        placeholder={COMPUTE_PROFILE_COPY.custom}
                        className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                      />
                    </div>
                  </div>
                </div>
              )}

              {/* SUBTAB 2: SILENCE CUTOFF (DURATION) - MemoryCard Side-by-Side 2x2 Layout */}
              {activeSubTab === "silence" && (
                <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
                  <div className="flex flex-col gap-1 min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                        {VAD_SETTINGS_COPY.silence.title}
                      </span>
                      <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                        {currentSilenceMs} ms
                      </span>
                    </div>
                    <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                      {VAD_SETTINGS_COPY.silence.description}
                    </p>
                  </div>

                  {/* 2x2 Grid: [400ms, 800ms, 1200ms, Custom] */}
                  <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[116px] sm:w-[136px]">
                    {[
                      { label: "400ms", val: 400 },
                      { label: "800ms", val: 800 },
                      { label: "1200ms", val: 1200 },
                    ].map(({ label, val }) => {
                      const isSelected = currentSilenceMs === val;
                      return (
                        <button
                          key={val}
                          type="button"
                          onClick={() => updateDraft("vad", "silence_duration_ms", val)}
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
                        ![400, 800, 1200].includes(currentSilenceMs)
                          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                          : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                      )}
                    >
                      <input
                        type="text"
                        inputMode="numeric"
                        value={
                          ![400, 800, 1200].includes(currentSilenceMs) ? `${currentSilenceMs}ms` : ""
                        }
                        onChange={(e) => {
                          const clean = e.target.value.replace(/[^0-9]/g, "");
                          if (!clean) return;
                          const num = parseInt(clean, 10);
                          if (!isNaN(num) && num >= 100 && num <= 3000) {
                            updateDraft("vad", "silence_duration_ms", num);
                          }
                        }}
                        placeholder={COMPUTE_PROFILE_COPY.custom}
                        className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                      />
                    </div>
                  </div>
                </div>
              )}

              {/* SUBTAB 3: NOISE GATE - MemoryCard Side-by-Side 2x2 Layout */}
              {activeSubTab === "noiseGate" && (
                <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
                  <div className="flex flex-col gap-1 min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                        {VAD_SETTINGS_COPY.noiseGate.title}
                      </span>
                      <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                        {currentNoiseGate.toFixed(3)}
                      </span>
                    </div>
                    <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                      {VAD_SETTINGS_COPY.noiseGate.description}
                    </p>
                  </div>

                  {/* 2x2 Grid: [0.001, 0.005, 0.020, Custom] */}
                  <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[116px] sm:w-[136px]">
                    {[
                      { label: "Studio", val: 0.001 },
                      { label: "Normal", val: 0.005 },
                      { label: "Noisy", val: 0.020 },
                    ].map(({ label, val }) => {
                      const isSelected = Math.abs(currentNoiseGate - val) < 0.0015;
                      return (
                        <button
                          key={val}
                          type="button"
                          onClick={() => updateDraft("vad", "ptt_noise_gate", val)}
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
                        ![0.001, 0.005, 0.020].some((v) => Math.abs(currentNoiseGate - v) < 0.0015)
                          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                          : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                      )}
                    >
                      <input
                        type="text"
                        inputMode="decimal"
                        value={
                          ![0.001, 0.005, 0.020].some((v) => Math.abs(currentNoiseGate - v) < 0.0015)
                            ? currentNoiseGate.toFixed(3)
                            : ""
                        }
                        onChange={(e) => {
                          const clean = e.target.value.replace(/[^0-9.]/g, "");
                          if (!clean) return;
                          const num = parseFloat(clean);
                          if (!isNaN(num) && num >= 0.001 && num <= 0.09) {
                            updateDraft("vad", "ptt_noise_gate", num);
                          }
                        }}
                        placeholder={COMPUTE_PROFILE_COPY.custom}
                        className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                      />
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    );
  }
);

VadWorkspace.displayName = "VadWorkspace";
