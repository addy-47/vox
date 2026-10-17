import React, { useState } from "react";
import { ErrorBoundary } from "@/shared/components/common";
import { TabStrip } from "@/shared/ui";
import { PROFILER_TABS, type ProfilerTabItem } from "@/data/profilerCopy";
import type { ProfilerSnapshot, JSHeapSample, DOMSample, CSSIndicatorsSample } from "@/services/monitoringService";
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
      <TabStrip
        tabs={PROFILER_TABS.map((tab) => ({ id: tab.id, label: tab.label }))}
        activeTab={activeTab}
        onChange={(id) => setActiveTab(id as ProfilerTabItem["id"])}
        variant="pill"
        size="md"
        ariaLabel="Profiler tabs"
        className="gap-2 py-3 border-b border-[rgba(var(--border),0.1)] overflow-x-auto shrink-0 no-scrollbar"
      />

      {/* Active Tab Workspace */}
      <div className="flex-1 overflow-y-auto pt-4 pr-1 min-h-0">
        <ErrorBoundary name={`MemoryProfiler:${activeTab}`}>
          <div
            id={`profiler-tabpanel-${activeTab}`}
            role="tabpanel"
            aria-labelledby={`profiler-tab-${activeTab}`}
          >
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
          </div>
        </ErrorBoundary>
      </div>
    </div>
  );
};