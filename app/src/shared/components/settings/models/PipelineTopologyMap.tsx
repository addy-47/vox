import { memo, useCallback } from "react";
import type { ReactElement } from "react";
import { AudioWaveform, Ear, BrainCircuit, AudioLines, LifeBuoy } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";
import { DIRTY_STATE_COPY, MODEL_HUB_COPY } from "@/data/settingsCopy";

export type PipelineTab = "vad" | "stt" | "llm" | "tts" | "auxiliary";

const PIPELINE_IDS: PipelineTab[] = ["vad", "stt", "llm", "tts", "auxiliary"];

export interface PipelineTopologyNode<T extends string = string> {
  id: T;
  label: string;
  shortLabel?: string;
  Icon: LucideIcon;
  disabled?: boolean;
  isDirty?: boolean;
}

export interface PipelineTopologyMapProps<T extends string = string> {
  activeTab: T;
  onChangeTab: (id: T) => void;
  nodes?: PipelineTopologyNode<T>[];
  layoutMode?: "full-max" | "full-min" | "small";
  ariaLabel?: string;
  isVadVerified?: boolean;
  isAsrVerified?: boolean;
  isLlmDownloaded?: boolean;
  isTtsVerified?: boolean;
  isAuxiliaryVerified?: boolean;
}

function PipelineTopologyMapInner<T extends string = string>({
  activeTab,
  onChangeTab,
  nodes,
  layoutMode,
  ariaLabel,
  isVadVerified = true,
  isAsrVerified = true,
  isLlmDownloaded = true,
  isTtsVerified = true,
  isAuxiliaryVerified = true,
}: PipelineTopologyMapProps<T>) {
  const isCategoryDirty = useSettingsStore((s) => s.isCategoryDirty);

  // ── Pipeline branch (nodes omitted) ──
  if (!nodes) {
    const pipelineNodes: Array<{
      id: PipelineTab;
      label: string;
      shortLabel: string;
      Icon: LucideIcon;
      isVerified: boolean;
    }> = [
      { id: "vad", label: MODEL_HUB_COPY.tabs.vad, shortLabel: MODEL_HUB_COPY.tabs.vadShort, Icon: AudioWaveform, isVerified: isVadVerified },
      { id: "stt", label: MODEL_HUB_COPY.tabs.stt, shortLabel: MODEL_HUB_COPY.tabs.sttShort, Icon: Ear, isVerified: isAsrVerified },
      { id: "llm", label: MODEL_HUB_COPY.tabs.llm, shortLabel: MODEL_HUB_COPY.tabs.llmShort, Icon: BrainCircuit, isVerified: isLlmDownloaded },
      { id: "tts", label: MODEL_HUB_COPY.tabs.tts, shortLabel: MODEL_HUB_COPY.tabs.ttsShort, Icon: AudioLines, isVerified: isTtsVerified },
      { id: "auxiliary", label: MODEL_HUB_COPY.tabs.auxiliary, shortLabel: MODEL_HUB_COPY.tabs.auxiliaryShort, Icon: LifeBuoy, isVerified: isAuxiliaryVerified },
    ];

    const handleKeyDown = useCallback(
      (e: React.KeyboardEvent) => {
        const currentIdx = PIPELINE_IDS.indexOf(activeTab as PipelineTab);
        if (e.key === "ArrowRight") {
          e.preventDefault();
          onChangeTab(PIPELINE_IDS[(currentIdx + 1) % PIPELINE_IDS.length] as T);
        } else if (e.key === "ArrowLeft") {
          e.preventDefault();
          onChangeTab(PIPELINE_IDS[(currentIdx - 1 + PIPELINE_IDS.length) % PIPELINE_IDS.length] as T);
        }
      },
      [activeTab, onChangeTab]
    );

    return (
      <div
        className={cn(
          "gap-1 shrink-0 p-1 rounded-xl glass overflow-visible mb-2.5 bg-[rgba(var(--foreground),0.02)]",
          layoutMode === "small"
            ? "flex overflow-x-auto snap-x no-scrollbar scrollbar-none w-full scroll-smooth"
            : "grid grid-cols-5"
        )}
        role="tablist"
        aria-label={ariaLabel ?? "Pipeline topology"}
      >
        {pipelineNodes.map(({ id, label, shortLabel, Icon }) => {
          const isDirty = isCategoryDirty(id);
          const isActive = activeTab === id;

          return (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={isActive}
              tabIndex={isActive ? 0 : -1}
              data-arrow-nav
              onKeyDown={handleKeyDown}
              onClick={() => onChangeTab(id as T)}
              className={cn(
                "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden cursor-pointer",
                isActive
                  ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                  : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
                layoutMode === "small" && "min-w-[62px] snap-center flex-1 py-1.5 px-0.5"
              )}
            >
              {isDirty && (
                <span
                  title={DIRTY_STATE_COPY.category}
                  className="absolute top-1.5 right-1.5 w-1.5 h-1.5 rounded-full bg-[rgb(var(--warn-soft))] shadow-[0_0_6px_rgba(var(--warn-soft),0.8)] shrink-0"
                />
              )}
              <Icon
                size={18}
                className={cn(
                  "transition-colors shrink-0",
                  activeTab === id
                    ? "text-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]"
                )}
              />
              <span className="text-[10.5px] sm:text-[11px] font-bold text-[rgb(var(--foreground))] tracking-tight truncate max-w-full leading-tight">
                {layoutMode === "small" ? shortLabel : label}
              </span>
            </button>
          );
        })}
      </div>
    );
  }

  // ── Settings branch (nodes provided) ──
  const ids = nodes.map((n) => n.id);
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const currentIdx = ids.indexOf(activeTab);
      if (e.key === "ArrowRight") {
        e.preventDefault();
        onChangeTab(ids[(currentIdx + 1) % ids.length]);
      } else if (e.key === "ArrowLeft") {
        e.preventDefault();
        onChangeTab(ids[(currentIdx - 1 + ids.length) % ids.length]);
      }
    },
    [activeTab, onChangeTab, ids]
  );

  return (
    <div
      className={cn(
        "gap-1 shrink-0 p-1 rounded-xl glass overflow-visible mb-2.5 bg-[rgba(var(--foreground),0.02)]",
        layoutMode === "small"
          ? "flex overflow-x-auto snap-x no-scrollbar scrollbar-none w-full scroll-smooth"
          : "flex items-center justify-around"
      )}
      role="tablist"
      aria-label={ariaLabel ?? "Settings topology"}
    >
      {nodes.map(({ id, label, Icon, disabled, isDirty }) => {
        const isActive = activeTab === id;

        return (
          <button
            key={id}
            type="button"
            disabled={disabled}
            role="tab"
            aria-selected={isActive}
            tabIndex={isActive ? 0 : -1}
            data-arrow-nav
            onKeyDown={handleKeyDown}
            onClick={() => !disabled && onChangeTab(id)}
            className={cn(
              "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden cursor-pointer flex-1",
              isActive
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              disabled && "opacity-40 cursor-not-allowed",
              layoutMode === "small" && "min-w-[75px] snap-center py-1.5 px-1"
            )}
          >
            {isDirty && (
              <span
                title={DIRTY_STATE_COPY.category}
                className="absolute top-1.5 right-1.5 w-1.5 h-1.5 rounded-full bg-[rgb(var(--warn-soft))] shadow-[0_0_6px_rgba(var(--warn-soft),0.8)] shrink-0"
              />
            )}
            <Icon
              size={16}
              className={cn(
                "transition-colors shrink-0",
                isActive
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]"
              )}
            />
            <span
              className={cn(
                "text-[10.5px] font-bold tracking-tight truncate max-w-full leading-tight select-none",
                isActive
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground))] group-hover:text-[rgb(var(--foreground))]"
              )}
            >
              {label}
            </span>
          </button>
        );
      })}
    </div>
  );
}

export const PipelineTopologyMap = memo(PipelineTopologyMapInner) as <T extends string = string>(
  props: PipelineTopologyMapProps<T>
) => ReactElement | null;
