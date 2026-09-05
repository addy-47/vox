import React from "react";
import { Lightbulb, AlertTriangle, CheckCircle2, ShieldCheck, Clock, Navigation, Activity, FileCode } from "lucide-react";
import type { ProfilerSnapshot, DOMSample, CSSIndicatorsSample } from "@/services/memoryProfilerService";
import type { PageMemoryRecord } from "@/shared/hooks/useMemoryProfiler";
import type { ComponentTraceData } from "@/shared/context/MemoryProfilerContext";
import { AccuracyBadge } from "./AccuracyBadge";
import { PROFILER_COPY } from "@/data/profilerCopy";

interface InsightsTabProps {
  latestSnapshot: ProfilerSnapshot | null;
  history: ProfilerSnapshot[];
  pageRecords: Record<string, PageMemoryRecord>;
  componentTraces: Record<string, ComponentTraceData>;
  domStats: DOMSample;
  cssStats: CSSIndicatorsSample;
  currentRoute: string;
}

export const InsightsTab: React.FC<InsightsTabProps> = ({
  latestSnapshot,
  history,
  pageRecords,
  componentTraces,
  domStats,
  cssStats,
  currentRoute,
}) => {
  const traces = Object.values(componentTraces);
  const totalRss = latestSnapshot?.total_vox_ram_mb ?? 0;
  const trayWebview = latestSnapshot?.tray_webview_ram_mb ?? 0;

  const insights: {
    id: string;
    title: string;
    description: string;
    severity: "high" | "medium" | "low" | "good";
    recommendation: string;
  }[] = [];

  // 1. Process tree evaluation
  if (totalRss > 600) {
    insights.push({
      id: "rss-high",
      title: "Elevated Vox Process Tree RSS",
      description: `Current process tree memory is ${totalRss.toFixed(1)} MB, exceeding normal desktop baseline (<400 MB).`,
      severity: "high",
      recommendation: "Audit loaded ONNX sessions, V8 heap usage, or active WebGL textures. Evict dormant models.",
    });
  } else {
    insights.push({
      id: "rss-nominal",
      title: "Nominal Process Memory Allocation",
      description: `Process tree RSS is stable at ${totalRss.toFixed(1)} MB.`,
      severity: "good",
      recommendation: "System operating within 8GB RAM target budget.",
    });
  }

  // 2. Tray WebView footprint
  if (trayWebview > 80) {
    insights.push({
      id: "tray-retention",
      title: "Persistent Tray HUD WebKit Allocation",
      description: `Tray WebKit process is holding ${trayWebview.toFixed(1)} MB of physical RSS.`,
      severity: "medium",
      recommendation: "Disable Tray HUD in Settings if background overlay is not in active use.",
    });
  }

  // 3. Retained Page Deltas
  Object.values(pageRecords).forEach((rec) => {
    if (rec.retainedDeltaMb && rec.retainedDeltaMb > 15) {
      insights.push({
        id: `page-leak-${rec.route}`,
        title: `Retained Memory Leak on Route (${rec.route})`,
        description: `Leaving ${rec.route} left +${rec.retainedDeltaMb.toFixed(1)} MB uncollected after 2.5s post-unmount.`,
        severity: rec.retainedDeltaMb > 40 ? "high" : "medium",
        recommendation: `Check useEffect subscriptions, event listeners, or WebGL geometries/materials in ${rec.route}.`,
      });
    }
  });

  // 4. Compositor indicators
  if (cssStats.backdropFilterCount > 15) {
    insights.push({
      id: "css-backdrop-filters",
      title: "Excessive Backdrop Filter Layers",
      description: `Detected ${cssStats.backdropFilterCount} elements with CSS backdrop-filter, increasing compositor GPU buffers.`,
      severity: "medium",
      recommendation: "Replace nested backdrop-filter elements with semi-opaque card backgrounds.",
    });
  }

  // 5. DOM tree size
  if (domStats.nodeCount > 3000) {
    insights.push({
      id: "dom-size-large",
      title: "Large DOM Tree Count",
      description: `DOM contains ${domStats.nodeCount.toLocaleString()} nodes.`,
      severity: "low",
      recommendation: "Consider virtualizing long lists or truncating unrendered nodes.",
    });
  }

  return (
    <div className="space-y-6 pb-20">
      {/* Root Cause Analysis & Leak Heuristics */}
      <div className="p-5 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] space-y-4 shadow-sm">
        <div className="flex items-center justify-between pb-3 border-b border-[rgba(var(--border),0.08)]">
          <div className="flex items-center gap-2">
            <Lightbulb size={17} className="text-[rgb(var(--accent))]" />
            <div>
              <h3 className="font-display text-sm font-bold tracking-wide text-[rgb(var(--foreground))]">
                {PROFILER_COPY.insights.sectionTitle}
              </h3>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] mt-0.5">
                {PROFILER_COPY.insights.diagnosticsHint}
              </p>
            </div>
          </div>
          <AccuracyBadge type="Correlated" />
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-3.5 pt-1">
          {insights.map((item) => (
            <div
              key={item.id}
              className="p-4 rounded-xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--card),0.6)] flex flex-col justify-between space-y-2.5"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  {item.severity === "high" || item.severity === "medium" ? (
                    <AlertTriangle
                      size={16}
                      className={item.severity === "high" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]"}
                    />
                  ) : (
                    <ShieldCheck size={16} className="text-[rgb(var(--accent))]" />
                  )}
                  <span className="text-xs font-bold text-[rgb(var(--foreground))]">{item.title}</span>
                </div>
                <span
                  className={`text-[11px] font-mono uppercase px-2 py-0.5 rounded border ${
                    item.severity === "high"
                      ? "bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] border-[rgb(var(--accent))]/40"
                      : item.severity === "medium"
                      ? "bg-[rgba(var(--foreground),0.1)] text-[rgb(var(--foreground))] border-[rgba(var(--foreground),0.2)]"
                      : "bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] border-[rgb(var(--accent))]/20"
                  }`}
                >
                  {item.severity}
                </span>
              </div>
              <p className="text-xs text-[rgb(var(--foreground-muted))] font-sans leading-relaxed">
                {item.description}
              </p>
              <div className="pt-2 border-t border-[rgba(var(--border),0.06)] flex items-start gap-1.5 text-[11px] text-[rgb(var(--foreground))] font-sans">
                <CheckCircle2 size={13} className="text-[rgb(var(--accent))] shrink-0 mt-0.5" />
                <span>
                  <strong>{PROFILER_COPY.insights.actionLabel}</strong> {item.recommendation}
                </span>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Two Column Row: Component Traces & Chronological Event Stream */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-5">
        {/* Component Lifecycle Tracer */}
        <div className="p-5 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] space-y-4 shadow-sm">
          <div className="flex items-center justify-between pb-3 border-b border-[rgba(var(--border),0.08)]">
            <div className="flex items-center gap-2">
              <FileCode size={17} className="text-[rgb(var(--accent))]" />
              <h3 className="font-display text-sm font-bold tracking-wide text-[rgb(var(--foreground))]">
                {PROFILER_COPY.insights.tracesTitle}
              </h3>
            </div>
            <AccuracyBadge type="Correlated" />
          </div>

          {traces.length === 0 ? (
            <div className="py-12 text-center text-[rgb(var(--foreground-muted))] font-sans text-xs">
              {PROFILER_COPY.noTraces}
            </div>
          ) : (
            <div className="overflow-x-auto max-h-[360px] overflow-y-auto pr-1">
              <table className="w-full text-left text-xs font-mono">
                <thead>
                  <tr className="border-b border-[rgba(var(--border),0.12)] text-[11px] text-[rgb(var(--foreground-muted))] uppercase">
                    <th className="pb-2.5 font-semibold">{PROFILER_COPY.insights.colComponent}</th>
                    <th className="pb-2.5 font-semibold">{PROFILER_COPY.insights.colMounts}</th>
                    <th className="pb-2.5 font-semibold">{PROFILER_COPY.insights.colInstances}</th>
                    <th className="pb-2.5 font-semibold">{PROFILER_COPY.insights.colLastActive}</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-[rgba(var(--border),0.06)] text-[12px]">
                  {traces.map((trace) => (
                    <tr key={trace.componentName} className="hover:bg-[rgba(var(--foreground),0.02)] transition-colors">
                      <td className="py-2 text-[rgb(var(--foreground))] font-medium font-sans truncate max-w-[160px]">
                        {trace.componentName}
                      </td>
                      <td className="py-2 text-[rgb(var(--foreground-muted))]">{trace.mountCount}</td>
                      <td className="py-2">
                        <span
                          className={`px-2 py-0.5 rounded text-[11px] font-mono ${
                            trace.activeInstances > 0
                              ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/30"
                              : "text-[rgb(var(--foreground-muted))]"
                          }`}
                        >
                          {trace.activeInstances}
                        </span>
                      </td>
                      <td className="py-2 text-[rgb(var(--foreground-muted))]">
                        {trace.lastMountedAt > 0 ? `${(trace.lastMountedAt / 1000).toFixed(1)}s` : "--"}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>

        {/* Real-time Timeline Stream */}
        <div className="p-5 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] space-y-4 shadow-sm">
          <div className="flex items-center justify-between pb-3 border-b border-[rgba(var(--border),0.08)]">
            <div className="flex items-center gap-2">
              <Clock size={17} className="text-[rgb(var(--accent))]" />
              <h3 className="font-display text-sm font-bold tracking-wide text-[rgb(var(--foreground))]">
                {PROFILER_COPY.insights.streamTitle}
              </h3>
            </div>
            <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
              {PROFILER_COPY.insights.liveTimeline}
            </span>
          </div>

          {history.length === 0 ? (
            <div className="py-12 text-center text-[rgb(var(--foreground-muted))] font-sans text-xs">
              {PROFILER_COPY.noTimeline}
            </div>
          ) : (
            <div className="space-y-2 max-h-[360px] overflow-y-auto pr-1">
              {history
                .slice()
                .reverse()
                .slice(0, 20)
                .map((item, idx) => {
                  const date = new Date(item.timestamp_ms);
                  const timeStr = date.toLocaleTimeString([], { hour12: false });

                  return (
                    <div
                      key={item.timestamp_ms || idx}
                      className="p-2.5 rounded-xl border border-[rgba(var(--border),0.08)] bg-[rgba(var(--card),0.6)] flex items-center justify-between text-xs font-mono"
                    >
                      <div className="flex items-center gap-2.5">
                        <span className="text-[rgb(var(--foreground-muted))] text-[11px]">{timeStr}</span>
                        <div className="flex items-center gap-1 font-bold text-[rgb(var(--foreground))]">
                          <Activity size={13} className="text-[rgb(var(--accent))]" />
                          <span>#{history.length - idx}</span>
                        </div>
                        <div className="hidden sm:flex items-center gap-1 text-[11px] text-[rgb(var(--foreground-muted))] font-sans">
                          <Navigation size={11} />
                          <span>{currentRoute}</span>
                        </div>
                      </div>

                      <div className="flex items-center gap-3">
                        <span className="text-[rgb(var(--foreground-muted))] text-[11px]">
                          Main: {item.main_webview_ram_mb ? `${item.main_webview_ram_mb.toFixed(1)}M` : "--"}
                        </span>
                        <span className="font-bold text-[rgb(var(--accent))] text-xs">
                          {item.total_vox_ram_mb.toFixed(1)} MB
                        </span>
                      </div>
                    </div>
                  );
                })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
