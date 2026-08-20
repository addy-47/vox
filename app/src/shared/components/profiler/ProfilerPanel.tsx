import React, { useState } from "react";
import { ErrorBoundary } from "@/shared/components/common";
import { cn } from "@/shared/lib/utils";
import { PROFILER_TABS, type ProfilerTabItem } from "@/data/profilerCopy";
import type { ProfilerSnapshot, JSHeapSample, DOMSample, CSSIndicatorsSample } from "@/services/memoryProfilerService";
import type { PageMemoryRecord } from "@/shared/hooks/useMemoryProfiler";
import type { ComponentTraceData } from "@/shared/context/MemoryProfilerContext";
import { OverviewTab } from "./OverviewTab";
import { PagesTab } from "./PagesTab";
import { InsightsTab } from "./InsightsTab";

export interface ProfilerPanelProps {
  latestSnapshot: ProfilerSnapshot | null;
  history: ProfilerSnapshot[];
  pageRecords: Record<string, PageMemoryRecord>;
  jsHeap: JSHeapSample;
  domStats: DOMSample;
  cssStats: CSSIndicatorsSample;
  componentTraces: Record<string, ComponentTraceData>;
  currentRoute: string;
}

export const ProfilerPanel: React.FC<ProfilerPanelProps> = ({
  latestSnapshot,
  history,
  pageRecords,
  jsHeap,
  domStats,
  cssStats,
  componentTraces,
  currentRoute,
}) => {
  const [activeTab, setActiveTab] = useState<ProfilerTabItem["id"]>("overview");

  return (
    <div className="flex flex-col h-full min-h-0">
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

      {/* Active Tab Workspace */}
      <div className="flex-1 overflow-y-auto pt-4 pr-1 min-h-0">
        <ErrorBoundary name={`MemoryProfiler:${activeTab}`}>
          {activeTab === "overview" && <OverviewTab latestSnapshot={latestSnapshot} history={history} jsHeap={jsHeap} />}

          {activeTab === "pages" && (
            <PagesTab pageRecords={pageRecords} currentRoute={currentRoute} jsHeap={jsHeap} domStats={domStats} cssStats={cssStats} />
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