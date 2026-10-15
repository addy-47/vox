import { memo, useState, useRef, useEffect, useCallback, useMemo } from "react";
import { cn } from "@/shared/lib/utils";
import { MODEL_HUB_COPY, DIRTY_STATE_COPY } from "@/data/settingsCopy";
import { useSettingsStore } from "@/store/settingsStore";
import { AudioWaveform, Ear, BrainCircuit, AudioLines, LifeBuoy } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { PipelineTab } from "./ModelsTopologyMap";

export interface UnifiedModelHubPanelProps {
  activeTab: PipelineTab;
  onChangeTab: (tab: PipelineTab) => void;
  layoutMode?: "full-max" | "full-min" | "small";
  isVadVerified?: boolean;
  isAsrVerified?: boolean;
  isLlmDownloaded?: boolean;
  isTtsVerified?: boolean;
  isAuxiliaryVerified?: boolean;
  children: React.ReactNode;
}

const PIPELINE_IDS: PipelineTab[] = ["vad", "stt", "llm", "tts", "auxiliary"];

function generateUnifiedPath(W: number, H: number, i: number): string {
  const yC = 52; // Top of card body
  const yT = 2;  // Top of active tab
  const yTabB = yT + 34; // Where tab straight sides begin curving into neck
  const Rcard = 16;
  const Rtab = 12;
  const tabW = W / 5;
  const xL = i * tabW;
  const xR = (i + 1) * tabW;

  let dL = 36;
  let dR = 36;
  if (i === 1) {
    dL = Math.min(xL - Rcard, 48);
    dR = 32;
  } else if (i === 3) {
    dL = 32;
    dR = Math.min(W - xR - Rcard, 48);
  }

  const xFlareL = Math.max(Rcard, xL - dL);
  const xFlareR = Math.min(W - Rcard, xR + dR);

  const p: string[] = [];

  if (i === 0) {
    // 1. Far Left Tab (Voice Detect): Left edge is a single continuous straight line from top to bottom
    p.push(`M 0 ${H - Rcard}`);
    p.push(`L 0 ${yT + Rtab}`);
    p.push(`C 0 ${yT + 0.45 * Rtab}, ${0.45 * Rtab} ${yT}, ${Rtab} ${yT}`);
    p.push(`L ${xR - Rtab} ${yT}`);
    p.push(`C ${xR - 0.45 * Rtab} ${yT}, ${xR} ${yT + 0.45 * Rtab}, ${xR} ${yT + Rtab}`);
    p.push(`L ${xR} ${yTabB}`);
    p.push(
      `C ${xR} ${yTabB + 0.5 * (yC - yTabB)}, ${xR + 0.5 * (xFlareR - xR)} ${yC}, ${xFlareR} ${yC}`
    );
    p.push(`L ${W - Rcard} ${yC}`);
    p.push(`C ${W - 0.45 * Rcard} ${yC}, ${W} ${yC + 0.45 * Rcard}, ${W} ${yC + Rcard}`);
    p.push(`L ${W} ${H - Rcard}`);
  } else if (i === 4) {
    // 2. Far Right Tab (Support): Right edge is a single continuous straight line from top to bottom
    p.push(`M 0 ${H - Rcard}`);
    p.push(`L 0 ${yC + Rcard}`);
    p.push(`C 0 ${yC + 0.45 * Rcard}, ${0.45 * Rcard} ${yC}, ${Rcard} ${yC}`);
    p.push(`L ${xFlareL} ${yC}`);
    p.push(
      `C ${xFlareL + 0.5 * (xL - xFlareL)} ${yC}, ${xL} ${yC - 0.5 * (yC - yTabB)}, ${xL} ${yTabB}`
    );
    p.push(`L ${xL} ${yT + Rtab}`);
    p.push(`C ${xL} ${yT + 0.45 * Rtab}, ${xL + 0.45 * Rtab} ${yT}, ${xL + Rtab} ${yT}`);
    p.push(`L ${W - Rtab} ${yT}`);
    p.push(`C ${W - 0.45 * Rtab} ${yT}, ${W} ${yT + 0.45 * Rtab}, ${W} ${yT + Rtab}`);
    p.push(`L ${W} ${H - Rcard}`);
  } else {
    // 3. Middle Tabs (Listening, Reasoning, Speaking): Symmetrical or weighted neck fillets on both sides
    p.push(`M 0 ${H - Rcard}`);
    p.push(`L 0 ${yC + Rcard}`);
    p.push(`C 0 ${yC + 0.45 * Rcard}, ${0.45 * Rcard} ${yC}, ${Rcard} ${yC}`);
    p.push(`L ${xFlareL} ${yC}`);
    p.push(
      `C ${xFlareL + 0.5 * (xL - xFlareL)} ${yC}, ${xL} ${yC - 0.5 * (yC - yTabB)}, ${xL} ${yTabB}`
    );
    p.push(`L ${xL} ${yT + Rtab}`);
    p.push(`C ${xL} ${yT + 0.45 * Rtab}, ${xL + 0.45 * Rtab} ${yT}, ${xL + Rtab} ${yT}`);
    p.push(`L ${xR - Rtab} ${yT}`);
    p.push(`C ${xR - 0.45 * Rtab} ${yT}, ${xR} ${yT + 0.45 * Rtab}, ${xR} ${yT + Rtab}`);
    p.push(`L ${xR} ${yTabB}`);
    p.push(
      `C ${xR} ${yTabB + 0.5 * (yC - yTabB)}, ${xR + 0.5 * (xFlareR - xR)} ${yC}, ${xFlareR} ${yC}`
    );
    p.push(`L ${W - Rcard} ${yC}`);
    p.push(`C ${W - 0.45 * Rcard} ${yC}, ${W} ${yC + 0.45 * Rcard}, ${W} ${yC + Rcard}`);
    p.push(`L ${W} ${H - Rcard}`);
  }

  // Common bottom corners
  p.push(`C ${W} ${H - 0.45 * Rcard}, ${W - 0.45 * Rcard} ${H}, ${W - Rcard} ${H}`);
  p.push(`L ${Rcard} ${H}`);
  p.push(`C ${0.45 * Rcard} ${H}, 0 ${H - 0.45 * Rcard}, 0 ${H - Rcard}`);
  p.push(`Z`);

  return p.join(" ");
}

export const UnifiedModelHubPanel = memo(
  ({
    activeTab,
    onChangeTab,
    layoutMode,
    isVadVerified = true,
    isAsrVerified = true,
    isLlmDownloaded = true,
    isTtsVerified = true,
    isAuxiliaryVerified = true,
    children,
  }: UnifiedModelHubPanelProps) => {
    const isCategoryDirty = useSettingsStore((s) => s.isCategoryDirty);
    const containerRef = useRef<HTMLDivElement | null>(null);
    const [dimensions, setDimensions] = useState({ width: 500, height: 280 });

    useEffect(() => {
      const el = containerRef.current;
      if (!el) return;

      const observer = new ResizeObserver((entries) => {
        for (const entry of entries) {
          const { width, height } = entry.contentRect;
          if (width > 0 && height > 0) {
            setDimensions({ width: Math.round(width), height: Math.round(height) });
          }
        }
      });
      observer.observe(el);
      return () => observer.disconnect();
    }, []);

    const activeIndex = useMemo(() => {
      const idx = PIPELINE_IDS.indexOf(activeTab);
      return idx >= 0 ? idx : 2;
    }, [activeTab]);

    const pathD = useMemo(() => {
      return generateUnifiedPath(dimensions.width, dimensions.height, activeIndex);
    }, [dimensions.width, dimensions.height, activeIndex]);

    const PIPELINE_NODES: Array<{
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
        const currentIdx = PIPELINE_IDS.indexOf(activeTab);
        if (e.key === "ArrowRight") {
          e.preventDefault();
          onChangeTab(PIPELINE_IDS[(currentIdx + 1) % PIPELINE_IDS.length]);
        } else if (e.key === "ArrowLeft") {
          e.preventDefault();
          onChangeTab(PIPELINE_IDS[(currentIdx - 1 + PIPELINE_IDS.length) % PIPELINE_IDS.length]);
        }
      },
      [activeTab, onChangeTab]
    );

    // Small layout fallback
    if (layoutMode === "small") {
      return (
        <div className="flex flex-col gap-2 w-full">
          <div
            className="flex overflow-x-auto snap-x no-scrollbar scrollbar-none w-full scroll-smooth gap-1 p-1 rounded-xl glass bg-[rgba(var(--foreground),0.02)]"
            role="tablist"
            aria-label="Pipeline topology"
          >
            {PIPELINE_NODES.map(({ id, shortLabel, Icon }) => {
              const isActive = activeTab === id;
              const isDirty = isCategoryDirty(id);
              return (
                <button
                  key={id}
                  type="button"
                  role="tab"
                  aria-selected={isActive}
                  onClick={() => onChangeTab(id)}
                  className={cn(
                    "min-w-[62px] snap-center flex-1 py-1.5 px-0.5 rounded-lg flex flex-col items-center justify-center gap-1 text-center cursor-pointer transition-colors",
                    isActive
                      ? "bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))] text-[rgb(var(--accent))]"
                      : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  {isDirty && (
                    <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--warn-soft))] shrink-0" />
                  )}
                  <Icon size={16} />
                  <span className="text-[10px] font-bold">{shortLabel}</span>
                </button>
              );
            })}
          </div>
          <div className="w-full rounded-xl border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] p-3">
            {children}
          </div>
        </div>
      );
    }

    const radialCx = `${((activeIndex + 0.5) / 5) * 100}%`;

    return (
      <div
        ref={containerRef}
        className="relative flex-1 w-full min-h-0 flex flex-col select-none "
      >
        {/* Inactive Tabs Track — Left of Active Tab */}
        {activeIndex > 0 && (
          <div
            className="absolute top-[2px] h-[48px] bg-[rgba(var(--foreground),0.025)] border-t border-l border-[rgba(var(--foreground),0.05)] rounded-tl-xl pointer-events-none z-0"
            style={{
              left: 0,
              width: `${(activeIndex / 5) * 100}%`,
            }}
          />
        )}
        {/* Inactive Tabs Track — Right of Active Tab */}
        {activeIndex < 4 && (
          <div
            className="absolute top-[2px] h-[48px] bg-[rgba(var(--foreground),0.025)] border-t border-r border-[rgba(var(--foreground),0.05)] rounded-tr-xl pointer-events-none z-0"
            style={{
              left: `${((activeIndex + 1) / 5) * 100}%`,
              right: 0,
            }}
          />
        )}

        {/* 1. Single Continuous SVG Surface for Active Tab + Card Body */}
        <svg
          className="absolute inset-0 w-full h-full pointer-events-none overflow-visible z-0"
          width="100%"
          height="100%"
        >
          <defs>
            <radialGradient
              id="unified-tab-glow"
              cx={radialCx}
              cy="25px"
              r="220px"
              gradientUnits="userSpaceOnUse"
            >
              <stop offset="0%" stopColor="rgb(var(--accent))" stopOpacity="0.12" />
              <stop offset="45%" stopColor="rgb(var(--accent))" stopOpacity="0.04" />
              <stop offset="100%" stopColor="rgb(var(--accent))" stopOpacity="0" />
            </radialGradient>
            <filter id="unified-card-shadow" x="-5%" y="-5%" width="110%" height="110%">
              <feDropShadow dx="0" dy="4" stdDeviation="6" floodColor="rgba(0,0,0,0.06)" />
            </filter>
          </defs>

          {/* Base Card Fill and Border */}
          <path
            d={pathD}
            fill="rgba(var(--card), 0.95)"
            stroke="rgba(var(--accent), 0.28)"
            strokeWidth="1.25"
            filter="url(#unified-card-shadow)"
            className="transition-all duration-300 ease-out"
          />

          {/* Ethereal Pink / Accent Radial Glow Emanating from Active Tab */}
          <path
            d={pathD}
            fill="url(#unified-tab-glow)"
            className="transition-all duration-300 ease-out"
          />
        </svg>

        {/* 2. Top Interactive Tab Bar Grid */}
        <div
          className="relative z-10 grid grid-cols-5 h-[48px] shrink-0"
          role="tablist"
          aria-label="Pipeline topology"
        >
          {PIPELINE_NODES.map(({ id, label, Icon }) => {
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
                onClick={() => onChangeTab(id)}
                className={cn(
                  "flex flex-col items-center justify-center gap-1 text-center transition-colors cursor-pointer relative",
                  isActive
                    ? "text-[rgb(var(--accent))] font-bold"
                    : "text-[rgb(var(--foreground-muted))]/75 hover:text-[rgb(var(--foreground))]"
                )}
              >
                {isDirty && (
                  <span
                    title={DIRTY_STATE_COPY.category}
                    className="absolute top-1.5 right-3 w-1.5 h-1.5 rounded-full bg-[rgb(var(--warn-soft))] shadow-[0_0_6px_rgba(var(--warn-soft),0.8)] shrink-0"
                  />
                )}
                <Icon size={16} className="shrink-0 transition-colors" />
                <span className="text-[10.5px] font-bold tracking-tight truncate max-w-full px-1 leading-tight">
                  {label}
                </span>
              </button>
            );
          })}
        </div>

        {/* 4. Child Workspace Content Area (Inside the Card Body) */}
        <div className="relative z-10 flex-1 min-h-0 w-full flex flex-col p-3 ">
          {children}
        </div>
      </div>
    );
  }
);

UnifiedModelHubPanel.displayName = "UnifiedModelHubPanel";
