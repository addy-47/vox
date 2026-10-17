import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { useModelParameters } from "@/shared/hooks/useModelParameters";
import { SubModelCard } from "./SubModelCard";
import type { ModelStatus } from "@/shared/hooks/useModelDownloads";
import { cn } from "@/shared/lib/utils";
import { activeSelection } from "@/shared/lib/activeSelection";
import { Zap, Battery, Gauge } from "lucide-react";
import { STT_SETTINGS_COPY, COMPUTE_PROFILE_COPY } from "@/data/settingsCopy";
import { CustomSvgs, PresetButton, PresetInput } from "./CustomSvgs";

interface TransliterationToggleProps {
  enabled: boolean;
  onToggle: () => void;
  label?: string;
  ariaLabel?: string;
}

const TransliterationToggle = memo(
  ({
    enabled,
    onToggle,
    label,
    ariaLabel = "Toggle script transliteration",
  }: TransliterationToggleProps) => (
    <button
      type="button"
      role="switch"
      aria-checked={enabled}
      aria-label={ariaLabel}
      onClick={onToggle}
      className={cn(
        "group flex flex-col items-center justify-center shrink-0 select-none cursor-pointer rounded-2xl px-3 py-2 transition-all duration-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))]",
        enabled
          ? "text-[rgb(var(--accent))]"
          : "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--foreground-muted))]/70"
      )}
    >
      <div
        className="relative w-[76px] h-[46px] transition-transform duration-300 group-hover:scale-105"
        style={{
          animation: enabled ? "wm-globe-float 3.6s ease-in-out infinite" : "none",
        }}
      >
        <svg
          viewBox="0 0 76 46"
          className="w-full h-full overflow-visible select-none"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          {enabled ? (
            <g className="transition-all duration-300 animate-fade-in">
              <rect
                x="3"
                y="3"
                width="70"
                height="40"
                rx="10"
                stroke="currentColor"
                strokeWidth="1.1"
                className="opacity-70"
              />
              <text
                x="20"
                y="24"
                textAnchor="middle"
                dominantBaseline="central"
                fontSize="17"
                fontWeight="700"
                fill="currentColor"
                className="select-none"
                style={{ fontFamily: "'Noto Sans Devanagari', 'Mukta', sans-serif" }}
              >
                अ
              </text>
              <g className="opacity-80">
                <line
                  x1="32"
                  y1="23"
                  x2="44"
                  y2="23"
                  stroke="currentColor"
                  strokeWidth="1.2"
                  strokeDasharray="2 1.5"
                />
                <path
                  d="M 41 20 L 45 23 L 41 26"
                  stroke="currentColor"
                  strokeWidth="1.25"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </g>
              <text
                x="56"
                y="23"
                textAnchor="middle"
                dominantBaseline="central"
                fontSize="17"
                fontWeight="800"
                fill="currentColor"
                className="select-none"
                style={{ fontFamily: "monospace, system-ui, sans-serif" }}
              >
                a
              </text>
              <circle cx="20" cy="38" r="1.2" fill="currentColor" opacity="0.6" />
              <circle cx="56" cy="38" r="1.2" fill="currentColor" opacity="0.6" />
            </g>
          ) : (
            <g className="transition-all duration-300">
              <rect
                x="14"
                y="3"
                width="48"
                height="40"
                rx="10"
                stroke="currentColor"
                strokeWidth="1.1"
                strokeDasharray="3 2"
                className="opacity-30"
              />
              <text
                x="38"
                y="23"
                textAnchor="middle"
                dominantBaseline="central"
                fontSize="19"
                fontWeight="700"
                fill="currentColor"
                className="opacity-55 select-none"
                style={{ fontFamily: "'Noto Sans Devanagari', 'Mukta', sans-serif" }}
              >
                अ
              </text>
            </g>
          )}
        </svg>
      </div>
      <span
        className={cn(
          "mt-2 px-2.5 py-0.5 rounded-full text-[9px] font-mono font-bold uppercase tracking-[0.14em] leading-none transition-all duration-300",
          enabled
            ? "text-[rgb(var(--accent))]"
            : "text-[rgb(var(--foreground-muted))]/50"
        )}
      >
        {label || (enabled ? "ENABLED" : "DISABLED")}
      </span>
    </button>
  )
);

TransliterationToggle.displayName = "TransliterationToggle";

interface AsrWorkspaceProps {
  activeCategoryTab?: "model" | "settings";
  activeSubTab?: string;
  layoutMode?: "full-max" | "full-min" | "small";
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, ModelStatus>;
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
    const activeSttModel = draftSettings?.stt?.active?.model || null;
    const { params, setParam } = useModelParameters(activeSttModel);

    if (!draftSettings || !modelCatalog) return null;

    const numParam = (v: unknown): number | undefined =>
      typeof v === "number" && !Number.isNaN(v) ? v : undefined;
    const currentThrottle = numParam(params.partial_throttle_ms) ?? 300;
    const transliterateEnabled = draftSettings.stt?.transliterate_enabled ?? true;

    const throttleCustom = ![150, 300, 500].includes(currentThrottle);

    return (
      <div className={cn("flex-1 min-h-0 w-full overflow-y-auto custom-scrollbar pr-1", layoutMode === "small" && "max-h-[220px]")}>
        {activeCategoryTab === "model" ? (
          <div
            className={cn(
              "grid gap-2.5 h-full",
              modelCatalog.stt.length <= 2
                ? (layoutMode === "small" ? "grid-cols-1 auto-rows-fr" : "grid-cols-2 grid-rows-1")
                : (layoutMode === "small" ? "grid-cols-1 auto-rows-max content-start" : "grid-cols-2 auto-rows-max content-start")
            )}
          >
          {modelCatalog.stt.map((model) => {
            const activeModel = draftSettings.stt?.active?.model ?? modelCatalog.stt?.[0]?.id;
            const isSelected = activeModel === model.id;
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
                  updateDraft("stt", "active", activeSelection("embedded", model.id));
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
        <div
          className={cn(
            "w-full flex flex-col",
            layoutMode === "small" ? "h-auto py-1 space-y-2.5" : "h-full"
          )}
        >
          {/* SUBTAB 1: STREAMING CADENCE */}
          {activeSubTab === "streamingRate" && (
            <CustomSvgs
              title={STT_SETTINGS_COPY.streamingRate.title}
              description={STT_SETTINGS_COPY.streamingRate.description}
              layoutMode={layoutMode}
              controls={
                <>
                  {[
                    { label: "150ms", val: 150 },
                    { label: "300ms", val: 300 },
                    { label: "500ms", val: 500 },
                  ].map(({ label, val }) => (
                    <PresetButton
                      key={val}
                      selected={currentThrottle === val}
                      onClick={() => setParam("partial_throttle_ms", val)}
                    >
                      {label}
                    </PresetButton>
                  ))}
                  <PresetInput
                    selected={throttleCustom}
                    value={throttleCustom ? `${currentThrottle}ms` : ""}
                    placeholder={COMPUTE_PROFILE_COPY.custom}
                    onChange={(e) => {
                      const clean = e.target.value.replace(/[^0-9]/g, "");
                      if (!clean) return;
                      const num = parseInt(clean, 10);
                      if (!isNaN(num) && num >= 50 && num <= 1500) {
                        setParam("partial_throttle_ms", num);
                      }
                    }}
                  />
                </>
              }
            />
          )}

          {/* SUBTAB 2: TRANSLITERATION */}
          {activeSubTab === "transliteration" && (
            <CustomSvgs
              title={STT_SETTINGS_COPY.transliteration.title}
              description={STT_SETTINGS_COPY.transliteration.description}
              layoutMode={layoutMode}
              rightSlot={
                <TransliterationToggle
                  enabled={transliterateEnabled}
                  onToggle={() => updateDraft("stt", "transliterate_enabled", !transliterateEnabled)}
                />
              }
            />
          )}

          {/* SUBTAB 3: COMPUTE ALLOCATION */}
          {activeSubTab === "compute" && (() => {
            const totalCores = (typeof navigator !== "undefined" ? navigator.hardwareConcurrency : undefined) || 4;
            const optimalThreads = Math.max(2, totalCores - 2);
            const ecoThreads = Math.max(1, Math.floor(totalCores / 2));
            const currentThreads = numParam(params.threads) ?? 4;
            const currentProfile =
              currentThreads === totalCores ? "max"
              : currentThreads === optimalThreads ? "auto"
              : currentThreads === ecoThreads ? "eco"
              : "custom";
            return (
              <CustomSvgs
                title={COMPUTE_PROFILE_COPY.title}
                description={STT_SETTINGS_COPY.compute.description}
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
                      selected={currentProfile === "eco"}
                      onClick={() => setParam("threads", ecoThreads)}
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
                    <PresetInput
                      selected={currentProfile === "custom"}
                      value={currentProfile === "custom" ? `${currentThreads}T` : ""}
                      placeholder={COMPUTE_PROFILE_COPY.custom}
                      onChange={(e) => {
                        const clean = e.target.value.replace(/[^0-9]/g, "");
                        if (!clean) return;
                        const num = parseInt(clean, 10);
                        if (!isNaN(num) && num >= 1 && num <= 64) {
                          setParam("threads", num);
                        }
                      }}
                    />
                  </>
                }
              />
            );
          })()}
        </div>
      )}
    </div>
  );
});

AsrWorkspace.displayName = "AsrWorkspace";
