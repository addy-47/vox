import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { SubModelCard } from "../SubModelCard";
import { cn } from "@/shared/lib/utils";
import { Microchip, Zap, Battery, Gauge } from "lucide-react";
import { STT_SETTINGS_COPY, COMPUTE_PROFILE_COPY } from "@/data/settingsCopy";

interface AsrWorkspaceProps {
  activeCategoryTab?: "model" | "settings";
  activeSubTab?: string;
  layoutMode?: "full-max" | "full-min" | "small";
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, any>;
  startDownload: (id: string) => void;
  deleteModel: (id: string) => void;
  isGroupRequired: (id: string) => boolean;
}

export const AsrWorkspace = memo(
  ({
    activeCategoryTab = "model",
    activeSubTab = "streamingRate",
    layoutMode,
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
    isGroupRequired,
  }: AsrWorkspaceProps) => {
    const draftSettings = useSettingsStore((s) => s.draftSettings);
    const updateDraft = useSettingsStore((s) => s.updateDraft);
    const modelCatalog = useSettingsStore((s) => s.modelCatalog);

    if (!draftSettings || !modelCatalog) return null;

    const currentThrottle = draftSettings.stt?.embedded?.partial_throttle_ms ?? 300;
    const transliterateEnabled = draftSettings.stt?.transliterate_enabled ?? true;

    return (
      <div className="flex-1 min-h-0 w-full overflow-y-auto custom-scrollbar pr-1">
        {activeCategoryTab === "model" ? (
          <div
            className={cn(
              "grid gap-2.5 h-full",
              modelCatalog.stt.length <= 2
                ? (layoutMode === "small" ? "grid-cols-1 auto-rows-fr" : "grid-cols-2 grid-rows-1")
                : (layoutMode === "small" ? "grid-cols-1 auto-rows-full snap-y snap-mandatory" : "grid-cols-2 auto-rows-full snap-y snap-mandatory")
            )}
          >
          {modelCatalog.stt.map((model) => {
            const isSelected = draftSettings.stt.embedded.model === model.id;
            const modelGroupId = model.id;
            const isDownloaded = !!modelPresence[modelGroupId];
            const status = downloadStatuses[modelGroupId];

            return (
              <SubModelCard
                key={model.id}
                id={modelGroupId}
                name={model.name}
                description={model.description || ""}
                parameters={model.parameters || ""}
                ramUsage={model.ram_usage}
                tradeoffs={model.tradeoffs}
                isDownloaded={isDownloaded}
                isActive={isSelected}
                isRequired={isGroupRequired(model.id)}
                layoutMode={layoutMode}
                onSelect={() => {
                  updateDraft("stt", "model", model.id);
                  updateDraft("stt", "embedded", {
                    model: model.id,
                  });
                }}
                confirmDeleteId={confirmDeleteId}
                setConfirmDeleteId={setConfirmDeleteId}
                downloadStatus={status}
                startDownload={() => startDownload(modelGroupId)}
                deleteModel={() => deleteModel(modelGroupId)}
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
            {/* SUBTAB 1: STREAMING CADENCE - MemoryCard Side-by-Side 2x2 Layout */}
            {activeSubTab === "streamingRate" && (
              <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
                <div className="flex flex-col gap-1 min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                      {STT_SETTINGS_COPY.streamingRate.title}
                    </span>
                    <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                      {currentThrottle} ms
                    </span>
                  </div>
                  <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                    {STT_SETTINGS_COPY.streamingRate.description}
                  </p>
                </div>

                {/* 2x2 Grid: [150ms, 300ms, 500ms, Custom] */}
                <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[116px] sm:w-[136px]">
                  {[
                    { label: "150ms", val: 150 },
                    { label: "300ms", val: 300 },
                    { label: "500ms", val: 500 },
                  ].map(({ label, val }) => {
                    const isSelected = currentThrottle === val;
                    return (
                      <button
                        key={val}
                        type="button"
                        onClick={() => updateDraft("stt", "partial_throttle_ms", val)}
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
                      ![150, 300, 500].includes(currentThrottle)
                        ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                        : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                    )}
                  >
                    <input
                      type="text"
                      inputMode="numeric"
                      value={
                        ![150, 300, 500].includes(currentThrottle) ? `${currentThrottle}ms` : ""
                      }
                      onChange={(e) => {
                        const clean = e.target.value.replace(/[^0-9]/g, "");
                        if (!clean) return;
                        const num = parseInt(clean, 10);
                        if (!isNaN(num) && num >= 50 && num <= 1500) {
                          updateDraft("stt", "partial_throttle_ms", num);
                        }
                      }}
                      placeholder={COMPUTE_PROFILE_COPY.custom}
                      className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                    />
                  </div>
                </div>
              </div>
            )}

            {/* SUBTAB 2: TRANSLITERATION - MemoryCard Side-by-Side 2-Option Layout */}
            {activeSubTab === "transliteration" && (
              <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
                <div className="flex flex-col gap-1 min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                      {STT_SETTINGS_COPY.transliteration.title}
                    </span>
                    <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                      {transliterateEnabled ? "ACTIVE" : "DISABLED"}
                    </span>
                  </div>
                  <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                    {STT_SETTINGS_COPY.transliteration.description}
                  </p>
                </div>

                <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[116px] sm:w-[136px]">
                  {[
                    { label: "Normal", val: true },
                    { label: "Raw", val: false },
                  ].map(({ label, val }) => {
                    const isSelected = transliterateEnabled === val;
                    return (
                      <button
                        key={label}
                        type="button"
                        onClick={() => updateDraft("stt", "transliterate_enabled", val)}
                        className={cn(
                          "py-1.5 rounded-lg border text-[11px] font-mono font-bold transition-all duration-200 cursor-pointer flex items-center justify-center",
                          isSelected
                            ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                            : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                        )}
                      >
                        {label}
                      </button>
                    );
                  })}
                </div>
              </div>
            )}
            {/* SUBTAB 3: COMPUTE ALLOCATION */}
            {activeSubTab === "compute" && (() => {
              const totalCores = (typeof navigator !== "undefined" ? navigator.hardwareConcurrency : undefined) || 4;
              const optimalThreads = Math.max(2, totalCores - 2);
              const ecoThreads = Math.max(1, Math.floor(totalCores / 2));
              const currentThreads = draftSettings.stt?.embedded?.threads ?? 4;
              const currentProfile =
                currentThreads === totalCores ? "max"
                : currentThreads === optimalThreads ? "auto"
                : currentThreads === ecoThreads ? "eco"
                : "custom";
              return (
                <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
                  <div className="flex flex-col gap-1 min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] flex items-center gap-1.5">
                        <Microchip size={14} className="text-[rgb(var(--accent))]" />
                        {COMPUTE_PROFILE_COPY.title}
                      </span>
                      <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                        {currentThreads} / {totalCores} Cores
                      </span>
                    </div>
                    <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                      {STT_SETTINGS_COPY.compute.description}
                    </p>
                  </div>

                  <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[116px] sm:w-[136px]">
                    <button
                      type="button"
                      onClick={() => updateDraft("stt", "threads", optimalThreads)}
                      className={cn(
                        "py-1 rounded-lg border text-[11px] font-bold transition-all duration-200 cursor-pointer flex items-center justify-center gap-1",
                        currentProfile === "auto"
                          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                          : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                      )}
                    >
                      <Zap size={11} className="text-[rgb(var(--accent))]" />
                      <span>{COMPUTE_PROFILE_COPY.auto}</span>
                    </button>

                    <button
                      type="button"
                      onClick={() => updateDraft("stt", "threads", ecoThreads)}
                      className={cn(
                        "py-1 rounded-lg border text-[11px] font-bold transition-all duration-200 cursor-pointer flex items-center justify-center gap-1",
                        currentProfile === "eco"
                          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                          : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                      )}
                    >
                      <Battery size={11} className="text-emerald-400" />
                      <span>{COMPUTE_PROFILE_COPY.eco}</span>
                    </button>

                    <button
                      type="button"
                      onClick={() => updateDraft("stt", "threads", totalCores)}
                      className={cn(
                        "py-1 rounded-lg border text-[11px] font-bold transition-all duration-200 cursor-pointer flex items-center justify-center gap-1",
                        currentProfile === "max"
                          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                          : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                      )}
                    >
                      <Gauge size={11} className="text-amber-400" />
                      <span>{COMPUTE_PROFILE_COPY.max}</span>
                    </button>

                    <div className={cn(
                      "rounded-lg border flex items-center justify-center transition-all overflow-hidden",
                      currentProfile === "custom"
                        ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                        : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                    )}>
                      <input
                        type="text"
                        inputMode="numeric"
                        value={currentProfile === "custom" ? `${currentThreads}T` : ""}
                        onChange={(e) => {
                          const clean = e.target.value.replace(/[^0-9]/g, "");
                          if (!clean) return;
                          const num = parseInt(clean, 10);
                          if (!isNaN(num) && num >= 1 && num <= 64) {
                            updateDraft("stt", "threads", num);
                          }
                        }}
                        placeholder={COMPUTE_PROFILE_COPY.custom}
                        className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                      />
                    </div>
                  </div>
                </div>
              );
            })()}
          </div>
        </div>
      )}
    </div>
  );
});

AsrWorkspace.displayName = "AsrWorkspace";
