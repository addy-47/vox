import React from "react";
import { Layers, AlertTriangle, Code, Palette, Box, Activity } from "lucide-react";
import type { PageMemoryRecord } from "@/shared/hooks/useMemoryProfiler";
import type { JSHeapSample, DOMSample, CSSIndicatorsSample } from "@/services/memoryProfilerService";
import { AccuracyBadge } from "./AccuracyBadge";
import { cn } from "@/shared/lib/utils";
import { TRACKED_PAGES } from "@/data/profilerCopy";

interface PagesTabProps {
  pageRecords: Record<string, PageMemoryRecord>;
  currentRoute: string;
  jsHeap: JSHeapSample;
  domStats: DOMSample;
  cssStats: CSSIndicatorsSample;
}

export const PagesTab: React.FC<PagesTabProps> = ({
  pageRecords,
  currentRoute,
  jsHeap,
  domStats,
  cssStats,
}) => {
  return (
    <div className="space-y-6 pb-20">
      {/* Resource & Browser Diagnostic Indicators Row */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3.5">
        {/* DOM Elements */}
        <div className="p-4 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] space-y-2.5 shadow-sm">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Code size={16} className="text-[rgb(var(--accent))]" />
              <span className="text-xs font-bold text-[rgb(var(--foreground))]">DOM Elements</span>
            </div>
            <AccuracyBadge type={domStats.accuracy} />
          </div>
          <div>
            <div className="font-mono text-2xl font-bold text-[rgb(var(--foreground))]">
              {domStats.nodeCount.toLocaleString()}
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))] mt-0.5">
              Active DOM tree elements
            </p>
          </div>
          <div className="pt-2 border-t border-[rgba(var(--border),0.08)] text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
            Target: &lt; 2,500 elements
          </div>
        </div>

        {/* Font Faces */}
        <div className="p-4 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] space-y-2.5 shadow-sm">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Palette size={16} className="text-[rgb(var(--accent))]" />
              <span className="text-xs font-bold text-[rgb(var(--foreground))]">Typography Faces</span>
            </div>
            <AccuracyBadge type={domStats.accuracy} />
          </div>
          <div>
            <div className="font-mono text-2xl font-bold text-[rgb(var(--foreground))]">
              {domStats.fontFaceCount}
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))] mt-0.5">
              Loaded @font-face sets
            </p>
          </div>
          <div className="pt-2 border-t border-[rgba(var(--border),0.08)] text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
            Sora, DM Sans, JetBrains Mono
          </div>
        </div>

        {/* JS Heap */}
        <div className="p-4 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] space-y-2.5 shadow-sm">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Box size={16} className="text-[rgb(var(--accent))]" />
              <span className="text-xs font-bold text-[rgb(var(--foreground))]">JS Heap V8/WebKit</span>
            </div>
            <AccuracyBadge type={jsHeap.accuracy} />
          </div>
          <div>
            <div className="font-mono text-2xl font-bold text-[rgb(var(--foreground))]">
              {jsHeap.available && jsHeap.usedMb !== null ? `${jsHeap.usedMb.toFixed(1)} MB` : "Unavailable"}
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))] mt-0.5">
              {jsHeap.available && jsHeap.limitMb ? `Limit: ${jsHeap.limitMb.toFixed(0)} MB` : "V8 heap telemetry"}
            </p>
          </div>
          <div className="pt-2 border-t border-[rgba(var(--border),0.08)] text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
            Allocated: {jsHeap.totalMb ? `${jsHeap.totalMb.toFixed(1)} MB` : "Standard flags"}
          </div>
        </div>

        {/* Compositing Layers */}
        <div className="p-4 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] space-y-2.5 shadow-sm">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Activity size={16} className="text-[rgb(var(--accent))]" />
              <span className="text-xs font-bold text-[rgb(var(--foreground))]">GPU Layers</span>
            </div>
            <AccuracyBadge type={cssStats.accuracy} />
          </div>
          <div>
            <div className="font-mono text-2xl font-bold text-[rgb(var(--foreground))]">
              {cssStats.backdropFilterCount} blur / {cssStats.canvasCount} canvas
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))] mt-0.5">
              Backdrop-filter & Canvas tags
            </p>
          </div>
          <div className="pt-2 border-t border-[rgba(var(--border),0.08)] text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
            will-change tags: {cssStats.willChangeCount}
          </div>
        </div>
      </div>

      {/* Main Page Lifecycle Attribution Matrix */}
      <div className="p-5 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] space-y-4 shadow-sm">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 pb-3 border-b border-[rgba(var(--border),0.08)]">
          <div className="flex items-center gap-2">
            <Layers size={17} className="text-[rgb(var(--accent))]" />
            <h3 className="font-display text-sm font-bold tracking-wide text-[rgb(var(--foreground))]">
              Page Lifecycle Attribution Matrix
            </h3>
          </div>
          <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
            Standard Page Experiment (Baseline → Peak → Retained)
          </span>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs font-sans">
            <thead>
              <tr className="border-b border-[rgba(var(--border),0.12)] text-[11px] font-mono text-[rgb(var(--foreground-muted))] uppercase">
                <th className="pb-3 font-semibold">Route / Page</th>
                <th className="pb-3 font-semibold">Status</th>
                <th className="pb-3 font-semibold">Baseline</th>
                <th className="pb-3 font-semibold">Current</th>
                <th className="pb-3 font-semibold">Peak (Δ)</th>
                <th className="pb-3 font-semibold">Retained (Δ)</th>
                <th className="pb-3 font-semibold">Risk / Observation</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-[rgba(var(--border),0.06)] font-mono text-[12px]">
              {TRACKED_PAGES.map((page) => {
                const rec = pageRecords[page.route];
                const isCurrent = currentRoute === page.route;
                const baselineMb = rec?.baseline?.total_vox_ram_mb;
                const currentMb = rec?.current?.total_vox_ram_mb;
                const peakMb = rec?.peak?.total_vox_ram_mb;
                const peakDelta = rec?.peakDeltaMb;
                const retainedMb = rec?.retained?.total_vox_ram_mb;
                const retainedDelta = rec?.retainedDeltaMb;

                let riskBadge = (
                  <span className="text-[rgb(var(--accent))] font-sans text-[11px] font-medium">Normal</span>
                );
                if (retainedDelta !== null && retainedDelta !== undefined) {
                  if (retainedDelta > 40) {
                    riskBadge = (
                      <span className="text-[rgb(var(--accent))] font-sans text-[11px] font-bold flex items-center gap-1">
                        <AlertTriangle size={12} /> Critical Retention (+{retainedDelta}MB)
                      </span>
                    );
                  } else if (retainedDelta > 15) {
                    riskBadge = (
                      <span className="text-[rgb(var(--foreground))] font-sans text-[11px] font-semibold flex items-center gap-1">
                        <AlertTriangle size={12} /> Suspicious (+{retainedDelta}MB)
                      </span>
                    );
                  }
                }

                return (
                  <tr
                    key={page.route}
                    className={cn(
                      "hover:bg-[rgba(var(--foreground),0.02)] transition-colors",
                      isCurrent && "bg-[rgb(var(--accent))]/10"
                    )}
                  >
                    <td className="py-3 font-sans font-bold flex items-center gap-2">
                      <span
                        className={cn(
                          "w-2 h-2 rounded-full",
                          isCurrent ? "bg-[rgb(var(--accent))]" : "bg-[rgba(var(--foreground-muted),0.4)]"
                        )}
                      />
                      <span className="text-[rgb(var(--foreground))]">{page.name}</span>
                      <span className="text-[11px] text-[rgb(var(--foreground-muted))] font-mono font-normal">
                        ({page.route})
                      </span>
                    </td>
                    <td className="py-3 font-sans">
                      {isCurrent ? (
                        <span className="text-[11px] font-mono px-2 py-0.5 rounded-full bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/30">
                          Active
                        </span>
                      ) : rec?.unmountedAt ? (
                        <span className="text-[11px] font-mono px-2 py-0.5 rounded-full bg-[rgba(var(--foreground-muted),0.15)] text-[rgb(var(--foreground-muted))]">
                          Unmounted
                        </span>
                      ) : (
                        <span className="text-[11px] text-[rgb(var(--foreground-muted))]">Unvisited</span>
                      )}
                    </td>
                    <td className="py-3 text-[rgb(var(--foreground))]">
                      {baselineMb !== undefined ? `${baselineMb.toFixed(1)} MB` : "--"}
                    </td>
                    <td className="py-3 text-[rgb(var(--foreground))]">
                      {currentMb !== undefined ? `${currentMb.toFixed(1)} MB` : "--"}
                    </td>
                    <td className="py-3 text-[rgb(var(--foreground))]">
                      {peakMb !== undefined ? (
                        <span>
                          {peakMb.toFixed(1)} MB{" "}
                          {peakDelta !== null && peakDelta !== undefined && peakDelta > 0 && (
                            <span className="text-[rgb(var(--accent))] text-[11px]">
                              (+{peakDelta.toFixed(1)})
                            </span>
                          )}
                        </span>
                      ) : (
                        "--"
                      )}
                    </td>
                    <td className="py-3 text-[rgb(var(--foreground))]">
                      {retainedMb !== undefined && retainedMb !== null ? (
                        <span>
                          {retainedMb.toFixed(1)} MB{" "}
                          {retainedDelta !== null && retainedDelta !== undefined && (
                            <span
                              className={cn(
                                "text-[11px]",
                                retainedDelta > 15 ? "text-[rgb(var(--accent))] font-bold" : "text-[rgb(var(--foreground-muted))]"
                              )}
                            >
                              ({retainedDelta >= 0 ? `+${retainedDelta.toFixed(1)}` : retainedDelta.toFixed(1)})
                            </span>
                          )}
                        </span>
                      ) : isCurrent ? (
                        <span className="text-[11px] text-[rgb(var(--foreground-muted))] font-sans italic">
                          Measuring on exit...
                        </span>
                      ) : (
                        "--"
                      )}
                    </td>
                    <td className="py-3">{riskBadge}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
