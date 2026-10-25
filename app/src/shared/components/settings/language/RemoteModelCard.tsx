import { memo } from "react";
import {
  Heart,
  Loader2,
  AlertTriangle,
  Sparkles,
  Check,
  Copy,
  Zap,
  Layers,
  Cpu,
  Wrench,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { LLM_CATALOG_COPY } from "@/data/settingsCopy";
import type { LlmModelInfo, ModelCapabilities } from "@/store/settingsStore";
import {
  hasProbeWarning,
  probeWarningLabel,
} from "./catalogProbeUtils";

export interface RemoteModelCardProps {
  model: LlmModelInfo;
  isSelected: boolean;
  probed: ModelCapabilities | undefined;
  isTesting: boolean;
  probeError?: string;
  isFavorite?: boolean;
  onSelect: (id: string) => void;
  onProbe: (id: string) => void;
  onCopy: (id: string) => void;
  onToggleFavorite?: (id: string) => void;
  isCopied: boolean;
}

export const RemoteModelCard = memo(({
  model,
  isSelected,
  probed,
  isTesting,
  probeError,
  isFavorite = false,
  onSelect,
  onProbe,
  onCopy,
  onToggleFavorite,
  isCopied,
}: RemoteModelCardProps) => {
  const isGpu = probed?.is_gpu_accelerated;

  let org = "";
  let shortName = model.name || model.id;
  if (model.id.includes("/")) {
    const parts = model.id.split("/");
    org = parts[0];
    shortName = parts.slice(1).join("/");
  }

  return (
    <div
      role="button"
      tabIndex={0}
      aria-pressed={isSelected}
      aria-label={shortName}
      onClick={() => onSelect(model.id)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect(model.id);
        }
      }}
      className={cn(
        "relative flex flex-col justify-between p-4 rounded-xl border transition-colors duration-150 select-none group cursor-pointer transform-gpu contain-paint",
        isSelected
          ? "bg-[rgba(var(--card),0.5)] border-[rgb(var(--accent))] shadow-[0_0_24px_rgba(var(--accent),0.16)] ring-1 ring-[rgb(var(--accent))]/30"
          : "bg-[rgba(var(--card),0.5)] border-[rgba(var(--foreground),0.06)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--card),0.85)]",
        isGpu && !isSelected ? "border-[rgba(var(--notif-models),0.35)]" : ""
      )}
    >
      <div>
        {/* Header: Org + Title + Badges + Radio Sphere */}
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            {org && (
              <div className="text-[11px] font-mono text-[rgb(var(--accent))] font-medium tracking-wide uppercase truncate mb-0.5">
                {org}
              </div>
            )}
            <div className="flex items-center gap-1 min-w-0">
              <h4 className="font-display text-[14px] font-bold text-[rgb(var(--foreground))] tracking-tight truncate leading-tight min-w-0">
                {shortName}
              </h4>

              {/* Copy Button */}
              <Tooltip label={isCopied ? LLM_CATALOG_COPY.copiedModelId : LLM_CATALOG_COPY.copyModelId} side="top" asChild>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onCopy(model.id);
                  }}
                  className="p-1 rounded-md text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-colors shrink-0 cursor-pointer"
                  aria-label={LLM_CATALOG_COPY.copyModelId}
                >
                  {isCopied ? (
                    <Check size={13} className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))]" />
                  ) : (
                    <Copy size={13} />
                  )}
                </button>
              </Tooltip>

              {/* Warning */}
              {hasProbeWarning(probeError, probed) && (
                <Tooltip
                  side="top"
                  align="start"
                  className="p-3 w-[300px] max-w-[340px] whitespace-normal text-left border border-[rgba(var(--foreground),0.14)] bg-[rgb(var(--card))]/98 shadow-lg"
                  label={probeWarningLabel(probeError, probed)}
                >
                  <span
                    className="inline-flex items-center shrink-0 cursor-help p-1 -m-1"
                    onClick={(e) => e.stopPropagation()}
                  >
                    <AlertTriangle size={13} className="text-[rgb(var(--warn-soft))]" />
                  </span>
                </Tooltip>
              )}
            </div>
          </div>

          <div className="flex items-center gap-1.5 shrink-0">
            {model.family && (
              <span className="text-[10px] font-bold px-2 py-0.5 rounded-md bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.2)]">
                {model.family}
              </span>
            )}
            {model.quantization && (
              <span className="text-[10px] font-mono font-bold px-1.5 py-0.5 rounded-md bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--foreground),0.08)]">
                {model.quantization}
              </span>
            )}
            {model.size_bytes != null && (
              <span className="text-[10px] font-mono font-bold px-1.5 py-0.5 rounded-md bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--foreground),0.08)]">
                {(model.size_bytes / (1024 * 1024 * 1024)).toFixed(1)}GB
              </span>
            )}

            {/* Top-Right Heart Button */}
            {onToggleFavorite && (
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  e.preventDefault();
                  onToggleFavorite(model.id);
                }}
                aria-label={isFavorite ? `Remove ${shortName} from favorites` : `Add ${shortName} to favorites`}
                className={cn(
                  "p-1 -mr-1 -mt-0.5 rounded-lg shrink-0 transition-all cursor-pointer relative z-10",
                  isFavorite
                    ? "text-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--foreground),0.04)]"
                )}
              >
                <Heart size={15} fill={isFavorite ? "currentColor" : "none"} strokeWidth={isFavorite ? 2 : 1.75} />
              </button>
            )}
          </div>
        </div>

        {/* Sleek Benchmark & Telemetry Spec Grid */}
        <div className="mt-3 flex flex-col gap-1.5">
          <div className="grid grid-cols-2 gap-2">
            {/* Speed Tile */}
            <div className="flex flex-col justify-between p-2 rounded-lg bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] group-hover:border-[rgba(var(--accent),0.2)] transition-colors">
              <div className="flex items-center gap-1.5 text-[10px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/75">
                <Zap size={11} className="text-[rgb(var(--accent))] shrink-0" />
                <span>{LLM_CATALOG_COPY.metricTiles.speed}</span>
              </div>
              <div className="mt-1 text-[12px] font-mono font-bold truncate">
                {probed?.tps ? (
                  <span className="text-[rgb(var(--notif-models))]">
                    {probed.tps.toFixed(1)} <span className="text-[10px] font-normal opacity-70">tps</span>
                  </span>
                ) : (
                  <span className="text-[rgb(var(--foreground-muted))]/40 text-[11px] font-normal">{LLM_CATALOG_COPY.untested}</span>
                )}
              </div>
            </div>

            {/* Context Window Tile */}
            <div className="flex flex-col justify-between p-2 rounded-lg bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] group-hover:border-[rgba(var(--accent),0.2)] transition-colors">
              <div className="flex items-center gap-1.5 text-[10px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/75">
                <Layers size={11} className="text-[rgb(var(--accent))] shrink-0" />
                <span>{LLM_CATALOG_COPY.metricTiles.context}</span>
              </div>
              <div className="mt-1 text-[12px] font-mono font-bold text-[rgb(var(--foreground))] truncate">
                {probed?.context_window ? (
                  probed.context_window >= 1000000 ? (
                    `${(probed.context_window / 1000000).toFixed(1)}M`
                  ) : (
                    `${Math.round(probed.context_window / 1024)}k`
                  )
                ) : (
                  <span className="text-[rgb(var(--foreground-muted))]/70 font-normal text-[11px]">{LLM_CATALOG_COPY.managedContext}</span>
                )}
              </div>
            </div>

            {/* Compute / Hardware Tile */}
            <div className="flex flex-col justify-between p-2 rounded-lg bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] group-hover:border-[rgba(var(--accent),0.2)] transition-colors">
              <div className="flex items-center gap-1.5 text-[10px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/75">
                <Cpu size={11} className="text-[rgb(var(--accent))] shrink-0" />
                <span>{LLM_CATALOG_COPY.metricTiles.compute}</span>
              </div>
              <div className="mt-1 text-[12px] font-mono font-bold truncate">
                {probed?.vram_bytes ? (
                  <span className="text-[rgb(var(--notif-models))]">
                    {(probed.vram_bytes / (1024 * 1024)).toFixed(0)}MB <span className="text-[10px] opacity-70">{isGpu ? "GPU" : "CPU"}</span>
                  </span>
                ) : isGpu ? (
                  <span className="text-[rgb(var(--notif-models))]">{LLM_CATALOG_COPY.computeTypes.gpuAccel}</span>
                ) : probed?.server_has_gpu ? (
                  <span className="text-[rgb(var(--notif-models))]">{LLM_CATALOG_COPY.computeTypes.serverGpu}</span>
                ) : (
                  <span className="text-[rgb(var(--foreground-muted))]/60 font-normal text-[11px]">{LLM_CATALOG_COPY.computeTypes.standard}</span>
                )}
              </div>
            </div>

            {/* Tool Calling / Tools Tile */}
            <div className="flex flex-col justify-between p-2 rounded-lg bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] group-hover:border-[rgba(var(--accent),0.2)] transition-colors">
              <div className="flex items-center gap-1.5 text-[10px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/75">
                <Wrench size={11} className="text-[rgb(var(--accent))] shrink-0" />
                <span>{LLM_CATALOG_COPY.metricTiles.tools}</span>
              </div>
              <div className="mt-1 text-[12px] font-mono font-bold truncate">
                {probed?.supports_tools === "supported" ? (
                  <span className="text-[rgb(var(--notif-models))]">{LLM_CATALOG_COPY.toolsSupported}</span>
                ) : probed?.supports_tools === "unsupported" ? (
                  <span className="text-[rgb(var(--foreground-muted))]/40 font-normal text-[11px]">{LLM_CATALOG_COPY.toolsNone}</span>
                ) : (
                  <span className="text-[rgb(var(--foreground-muted))]/40 font-normal text-[11px]">{LLM_CATALOG_COPY.toolsUnknown}</span>
                )}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Card Actions Footer: tactile Benchmark action on bottom right */}
      <div className="flex items-center justify-end gap-2 mt-3 pt-2.5 border-t border-[rgba(var(--foreground),0.06)] min-w-0">
        <Tooltip label={probed ? LLM_CATALOG_COPY.rerunBenchmark : LLM_CATALOG_COPY.runBenchmark} side="top" asChild>
          <button
            type="button"
            disabled={isTesting}
            onClick={(e) => {
              e.stopPropagation();
              onProbe(model.id);
            }}
            className="px-2.5 py-1.5 rounded-lg text-[11px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.25)] hover:bg-[rgb(var(--accent))]/20 transition-all flex items-center gap-1.5 cursor-pointer disabled:opacity-50"
          >
            {isTesting ? (
              <Loader2 size={12} className="animate-spin" />
            ) : (
              <Sparkles size={12} />
            )}
            <span>{probed ? LLM_CATALOG_COPY.reprobe : LLM_CATALOG_COPY.benchmark}</span>
          </button>
        </Tooltip>
      </div>
    </div>
  );
});

RemoteModelCard.displayName = "RemoteModelCard";
