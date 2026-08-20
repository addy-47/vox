import React, { useState } from "react";
import { Activity, RefreshCw } from "lucide-react";
import { useMemoryProfiler } from "@/shared/hooks/useMemoryProfiler";
import { useMemoryProfilerContext } from "@/shared/context/MemoryProfilerContext";
import { Tooltip } from "@/shared/ui/Tooltip";
import { ErrorBoundary } from "@/shared/components/common";
import { cn } from "@/shared/lib/utils";
import { PROFILER_TABS, PROFILER_COPY, type ProfilerTabItem } from "@/data/profilerData";

import { OverviewTab } from "@/shared/components/profiler/OverviewTab";
import { PagesTab } from "@/shared/components/profiler/PagesTab";
import { InsightsTab } from "@/shared/components/profiler/InsightsTab";

export const MemoryProfiler: React.FC = () => {
  const {
    latestSnapshot,
    history,
    pageRecords,
    jsHeap,
    domStats,
    cssStats,
    isSampling,
    currentRoute,
    captureSnapshot,
  } = useMemoryProfiler(true, 2500);

  const { componentTraces } = useMemoryProfilerContext();
  const [activeTab, setActiveTab] = useState<ProfilerTabItem["id"]>("overview");

  return (
    <div className="flex-1 flex flex-col h-full w-full overflow-hidden text-[rgb(var(--foreground))] px-6 lg:px-10 pt-5 pb-6">
      {/* Top Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-4 border-b border-[rgba(var(--border),0.12)] shrink-0">
        <div className="flex items-center gap-3">
          <div className="p-2.5 rounded-xl bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/25 text-[rgb(var(--accent))]">
            <Activity size={22} />
          </div>
          <div>
            <h1 className="font-display text-lg font-bold tracking-wide">
              {PROFILER_COPY.headerTitle}
            </h1>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))] font-sans mt-0.5">
              {PROFILER_COPY.headerSubtitle}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-3">
          {/* Live telemetry indicator pill */}
          <div className="flex items-center gap-2 px-3 py-1.5 rounded-xl bg-[rgba(var(--card),0.8)] border border-[rgba(var(--border),0.15)] text-[11px] font-mono">
            <span
              className={cn(
                "w-2 h-2 rounded-full",
                isSampling ? "bg-[rgb(var(--accent))] animate-ping" : "bg-[rgb(var(--accent))]"
              )}
            />
            <span className="text-[rgb(var(--foreground-muted))]">2.5s Polling</span>
          </div>

          <Tooltip label="Trigger immediate on-demand OS process sample">
            <button
              onClick={() => captureSnapshot()}
              disabled={isSampling}
              className="flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-sans font-bold uppercase tracking-wider bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/30 transition-all border border-[rgb(var(--accent))]/30 cursor-pointer disabled:opacity-50"
            >
              <RefreshCw size={14} className={cn(isSampling && "animate-spin")} />
              <span>{PROFILER_COPY.snapshotButton}</span>
            </button>
          </Tooltip>
        </div>
      </div>

      {/* Horizontal Tab Navigation Bar */}
      <div className="flex items-center gap-2 py-3 border-b border-[rgba(var(--border),0.1)] overflow-x-auto shrink-0 no-scrollbar">
        {PROFILER_TABS.map((tab) => {
          const isActive = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={cn(
                "px-4 py-2 rounded-xl text-xs font-sans font-semibold transition-all cursor-pointer whitespace-nowrap",
                isActive
                  ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/30 shadow-sm"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)] border border-transparent"
              )}
            >
              {tab.label}
            </button>
          );
        })}
      </div>

      {/* Active Tab Workspace - Scrollable within 100vh */}
      <div className="flex-1 overflow-y-auto pt-4 pr-1">
        <ErrorBoundary name={`MemoryProfiler:${activeTab}`}>
          {activeTab === "overview" && (
            <OverviewTab
              latestSnapshot={latestSnapshot}
              history={history}
              jsHeap={jsHeap}
            />
          )}

          {activeTab === "pages" && (
            <PagesTab
              pageRecords={pageRecords}
              currentRoute={currentRoute}
              jsHeap={jsHeap}
              domStats={domStats}
              cssStats={cssStats}
            />
          )}

          {activeTab === "insights" && (
            <InsightsTab
              latestSnapshot={latestSnapshot}
              history={history}
              pageRecords={pageRecords}
              componentTraces={componentTraces}
              domStats={domStats}
              cssStats={cssStats}
              currentRoute={currentRoute}
            />
          )}
        </ErrorBoundary>
      </div>
    </div>
  );
};
