import React, { useState } from "react";
import { Activity, Layers, Terminal } from "lucide-react";
import type { ProfilerSnapshot, JSHeapSample } from "@/services/memoryProfilerService";
import { AccuracyBadge } from "./AccuracyBadge";
import { cn } from "@/shared/lib/utils";

interface OverviewTabProps {
  latestSnapshot: ProfilerSnapshot | null;
  history: ProfilerSnapshot[];
  jsHeap: JSHeapSample;
}

export const OverviewTab: React.FC<OverviewTabProps> = ({
  latestSnapshot,
  history,
  jsHeap,
}) => {
  const [selectedPid, setSelectedPid] = useState<number | null>(null);

  const totalRss = latestSnapshot?.total_vox_ram_mb ?? 0;
  const mainWebview = latestSnapshot?.main_webview_ram_mb ?? 0;
  const trayWebview = latestSnapshot?.tray_webview_ram_mb ?? 0;
  const rustCore = latestSnapshot?.main_process_ram_mb ?? 0;
  const otherRss = Math.max(0, totalRss - (mainWebview + trayWebview + rustCore));

  // Dynamic Chart coordinate calculations
  const chartWidth = 700;
  const chartHeight = 220;
  const paddingX = 24;
  const paddingY = 24;

  const historyPoints = history.length > 0 ? history : latestSnapshot ? [latestSnapshot] : [];
  const maxHistoryRss = Math.max(...historyPoints.map((h) => h.total_vox_ram_mb), 100);
  const minHistoryRss = Math.max(0, Math.min(...historyPoints.map((h) => h.total_vox_ram_mb)) - 25);

  const getSvgCoordinates = (index: number, val: number) => {
    if (historyPoints.length <= 1) {
      return { x: chartWidth / 2, y: chartHeight / 2 };
    }
    const x = paddingX + (index / (historyPoints.length - 1)) * (chartWidth - paddingX * 2);
    const range = maxHistoryRss - minHistoryRss || 1;
    const y = chartHeight - paddingY - ((val - minHistoryRss) / range) * (chartHeight - paddingY * 2);
    return { x, y };
  };

  const totalPathPoints = historyPoints.map((h, i) => getSvgCoordinates(i, h.total_vox_ram_mb));
  const totalLinePath =
    totalPathPoints.length > 0
      ? `M ${totalPathPoints.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(" L ")}`
      : "";

  const totalAreaPath =
    totalPathPoints.length > 0
      ? `${totalLinePath} L ${totalPathPoints[totalPathPoints.length - 1].x.toFixed(
          1
        )},${(chartHeight - paddingY).toFixed(1)} L ${totalPathPoints[0].x.toFixed(
          1
        )},${(chartHeight - paddingY).toFixed(1)} Z`
      : "";

  // Donut chart calculations
  const totalAllocated = totalRss || 1;
  const segments = [
    { label: "Main WebView", val: mainWebview, color: "rgb(var(--accent))", pct: (mainWebview / totalAllocated) * 100 },
    { label: "Rust Core", val: rustCore, color: "rgba(var(--accent), 0.7)", pct: (rustCore / totalAllocated) * 100 },
    { label: "Tray HUD", val: trayWebview, color: "rgba(var(--accent), 0.45)", pct: (trayWebview / totalAllocated) * 100 },
    { label: "Other / Network", val: otherRss, color: "rgba(var(--foreground-muted), 0.35)", pct: (otherRss / totalAllocated) * 100 },
  ];

  const radius = 48;
  const circumference = 2 * Math.PI * radius;
  let cumulativeOffset = 0;

  const sortedProcesses = latestSnapshot?.process_tree
    ? [...latestSnapshot.process_tree].sort((a, b) => b.memory_mb - a.memory_mb)
    : [];

  return (
    <div className="space-y-6 pb-20">
      {/* 5 Top KPI Cards */}
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-3.5">
        <div className="p-4 rounded-2xl border border-[rgba(var(--accent),0.25)] bg-[rgba(var(--card),0.92)] shadow-sm">
          <div className="flex items-center justify-between">
            <span className="text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
              Total Vox RSS
            </span>
            <AccuracyBadge type="Measured" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-1.5">
            <span className="font-mono text-2xl lg:text-3xl font-bold text-[rgb(var(--accent))]">
              {totalRss > 0 ? totalRss.toFixed(1) : "--"}
            </span>
            <span className="font-mono text-xs text-[rgb(var(--foreground-muted))]">MB</span>
          </div>
          <p className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] mt-1 truncate">
            Process tree aggregate
          </p>
        </div>

        <div className="p-4 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] shadow-sm">
          <div className="flex items-center justify-between">
            <span className="text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
              Main WebView
            </span>
            <AccuracyBadge type={mainWebview > 0 ? "Measured" : "Unattributed"} />
          </div>
          <div className="mt-2.5 flex items-baseline gap-1.5">
            <span className="font-mono text-2xl lg:text-3xl font-bold text-[rgb(var(--foreground))]">
              {mainWebview > 0 ? mainWebview.toFixed(1) : "--"}
            </span>
            <span className="font-mono text-xs text-[rgb(var(--foreground-muted))]">MB</span>
          </div>
          <p className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] mt-1 truncate">
            Primary UI surface
          </p>
        </div>

        <div className="p-4 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] shadow-sm">
          <div className="flex items-center justify-between">
            <span className="text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
              Rust Core
            </span>
            <AccuracyBadge type="Measured" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-1.5">
            <span className="font-mono text-2xl lg:text-3xl font-bold text-[rgb(var(--foreground))]">
              {rustCore > 0 ? rustCore.toFixed(1) : "--"}
            </span>
            <span className="font-mono text-xs text-[rgb(var(--foreground-muted))]">MB</span>
          </div>
          <p className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] mt-1 truncate">
            Tauri host & pipelines
          </p>
        </div>

        <div className="p-4 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] shadow-sm">
          <div className="flex items-center justify-between">
            <span className="text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
              JS Heap
            </span>
            <AccuracyBadge type={jsHeap.available ? "Measured" : "Unattributed"} />
          </div>
          <div className="mt-2.5 flex items-baseline gap-1.5">
            <span className="font-mono text-2xl lg:text-3xl font-bold text-[rgb(var(--foreground))]">
              {jsHeap.available && jsHeap.usedMb !== null ? jsHeap.usedMb.toFixed(1) : "--"}
            </span>
            <span className="font-mono text-xs text-[rgb(var(--foreground-muted))]">MB</span>
          </div>
          <p className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] mt-1 truncate">
            {jsHeap.available && jsHeap.limitMb ? `Limit: ${jsHeap.limitMb.toFixed(0)} MB` : "V8/WebKit heap"}
          </p>
        </div>

        <div className="p-4 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] shadow-sm col-span-2 md:col-span-1">
          <div className="flex items-center justify-between">
            <span className="text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
              Tray WebView
            </span>
            <AccuracyBadge type={trayWebview > 0 ? "Measured" : "Unattributed"} />
          </div>
          <div className="mt-2.5 flex items-baseline gap-1.5">
            <span className="font-mono text-2xl lg:text-3xl font-bold text-[rgb(var(--foreground))]">
              {trayWebview > 0 ? trayWebview.toFixed(1) : "0.0"}
            </span>
            <span className="font-mono text-xs text-[rgb(var(--foreground-muted))]">MB</span>
          </div>
          <p className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] mt-1 truncate">
            HUD tray overlay
          </p>
        </div>
      </div>

      {/* Main Visuals Row: Time Series Area Chart & Donut Chart */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-5 items-stretch">
        {/* SVG Multi-Layer Time Series Area Chart */}
        <div className="lg:col-span-8 p-5 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] flex flex-col justify-between shadow-sm">
          <div className="flex items-center justify-between pb-3 border-b border-[rgba(var(--border),0.08)]">
            <div className="flex items-center gap-2">
              <Activity size={17} className="text-[rgb(var(--accent))]" />
              <h3 className="font-display text-sm font-bold tracking-wide text-[rgb(var(--foreground))]">
                Memory Over Time
              </h3>
            </div>
            <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
              Last {historyPoints.length} samples ({minHistoryRss.toFixed(0)} - {maxHistoryRss.toFixed(0)} MB)
            </span>
          </div>

          <div className="w-full h-56 relative flex items-center justify-center my-2">
            {historyPoints.length === 0 ? (
              <div className="text-xs text-[rgb(var(--foreground-muted))]">Sampling telemetry...</div>
            ) : (
              <svg
                viewBox={`0 0 ${chartWidth} ${chartHeight}`}
                className="w-full h-full overflow-visible"
                preserveAspectRatio="none"
              >
                <defs>
                  <linearGradient id="memAreaGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="rgb(var(--accent))" stopOpacity="0.32" />
                    <stop offset="100%" stopColor="rgb(var(--accent))" stopOpacity="0.0" />
                  </linearGradient>
                </defs>

                {/* Horizontal Guide lines */}
                {[0.2, 0.5, 0.8].map((pct, i) => {
                  const y = paddingY + (chartHeight - paddingY * 2) * pct;
                  return (
                    <line
                      key={i}
                      x1={paddingX}
                      y1={y}
                      x2={chartWidth - paddingX}
                      y2={y}
                      stroke="rgba(var(--border), 0.15)"
                      strokeDasharray="4 4"
                    />
                  );
                })}

                {/* Area fill */}
                {totalAreaPath && <path d={totalAreaPath} fill="url(#memAreaGrad)" />}

                {/* Main line */}
                {totalLinePath && (
                  <path
                    d={totalLinePath}
                    fill="none"
                    stroke="rgb(var(--accent))"
                    strokeWidth="2.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                )}

                {/* Latest point dot */}
                {totalPathPoints.length > 0 && (
                  <circle
                    cx={totalPathPoints[totalPathPoints.length - 1].x}
                    cy={totalPathPoints[totalPathPoints.length - 1].y}
                    r="5"
                    fill="rgb(var(--accent))"
                    stroke="rgb(var(--card))"
                    strokeWidth="2.5"
                  />
                )}
              </svg>
            )}
          </div>

          <div className="flex items-center justify-between pt-3 border-t border-[rgba(var(--border),0.08)] text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
            <span className="flex items-center gap-1.5">
              <span className="w-2 h-2 rounded-full bg-[rgb(var(--accent))]" />
              Total Process Tree RSS
            </span>
            <span className="text-[rgb(var(--foreground))] font-bold">Current: {totalRss.toFixed(1)} MB</span>
          </div>
        </div>

        {/* Donut Memory Breakdown */}
        <div className="lg:col-span-4 p-5 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] flex flex-col justify-between shadow-sm">
          <div className="flex items-center justify-between pb-3 border-b border-[rgba(var(--border),0.08)]">
            <div className="flex items-center gap-2">
              <Layers size={17} className="text-[rgb(var(--accent))]" />
              <h3 className="font-display text-sm font-bold tracking-wide text-[rgb(var(--foreground))]">
                Memory Distribution
              </h3>
            </div>
          </div>

          <div className="flex items-center justify-center my-2">
            <div className="relative w-36 h-36 flex items-center justify-center">
              <svg viewBox="0 0 120 120" className="w-full h-full -rotate-90">
                {segments.map((seg, i) => {
                  const dashArray = (seg.pct / 100) * circumference;
                  const offset = cumulativeOffset;
                  cumulativeOffset += dashArray;

                  return (
                    <circle
                      key={i}
                      cx="60"
                      cy="60"
                      r={radius}
                      fill="transparent"
                      stroke={seg.color}
                      strokeWidth="13"
                      strokeDasharray={`${dashArray} ${circumference - dashArray}`}
                      strokeDashoffset={-offset}
                      className="transition-all duration-500"
                    />
                  );
                })}
              </svg>
              <div className="absolute flex flex-col items-center justify-center text-center">
                <span className="font-mono text-base font-bold text-[rgb(var(--foreground))]">
                  {totalRss.toFixed(0)}
                </span>
                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">MB Total</span>
              </div>
            </div>
          </div>

          <div className="space-y-2 pt-3 border-t border-[rgba(var(--border),0.08)]">
            {segments.map((seg, i) => (
              <div key={i} className="flex items-center justify-between text-[11px] font-mono">
                <div className="flex items-center gap-2">
                  <span className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: seg.color }} />
                  <span className="text-[rgb(var(--foreground-muted))]">{seg.label}</span>
                </div>
                <div className="flex items-center gap-1.5">
                  <span className="font-bold text-[rgb(var(--foreground))]">{seg.val.toFixed(1)} MB</span>
                  <span className="text-[rgb(var(--foreground-muted))]">({seg.pct.toFixed(0)}%)</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* OS Process Tree Hierarchy (Complete Process Ledger) */}
      <div className="p-5 rounded-2xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.92)] space-y-4 shadow-sm">
        <div className="flex items-center justify-between pb-3 border-b border-[rgba(var(--border),0.08)]">
          <div className="flex items-center gap-2">
            <Terminal size={17} className="text-[rgb(var(--accent))]" />
            <h3 className="font-display text-sm font-bold tracking-wide text-[rgb(var(--foreground))]">
              OS Process Tree Hierarchy
            </h3>
          </div>
          <AccuracyBadge type="Measured" />
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs font-mono">
            <thead>
              <tr className="border-b border-[rgba(var(--border),0.12)] text-[11px] text-[rgb(var(--foreground-muted))] uppercase">
                <th className="pb-3 font-semibold">PID</th>
                <th className="pb-3 font-semibold">Process Name</th>
                <th className="pb-3 font-semibold">Inferred Role</th>
                <th className="pb-3 font-semibold">Physical RSS</th>
                <th className="pb-3 font-semibold">CPU Load</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-[rgba(var(--border),0.06)] text-[12px]">
              {sortedProcesses.map((proc) => (
                <tr
                  key={proc.pid}
                  onClick={() => setSelectedPid(proc.pid)}
                  className={cn(
                    "hover:bg-[rgba(var(--foreground),0.03)] transition-colors cursor-pointer",
                    selectedPid === proc.pid && "bg-[rgb(var(--accent))]/10"
                  )}
                >
                  <td className="py-2.5 text-[rgb(var(--foreground-muted))]">{proc.pid}</td>
                  <td className="py-2.5 font-bold text-[rgb(var(--foreground))]">{proc.name}</td>
                  <td className="py-2.5 font-sans">
                    <span
                      className={cn(
                        "px-2 py-0.5 rounded text-[11px] font-mono",
                        proc.is_main_process
                          ? "bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/30"
                          : proc.role.includes("Main WebView")
                          ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--foreground))] border border-[rgba(var(--border),0.25)]"
                          : proc.role.includes("Tray")
                          ? "bg-[rgba(var(--foreground-muted),0.15)] text-[rgb(var(--foreground-muted))]"
                          : "bg-[rgba(var(--foreground-muted),0.1)] text-[rgb(var(--foreground-muted))]"
                      )}
                    >
                      {proc.role}
                    </span>
                  </td>
                  <td className="py-2.5 font-bold text-[rgb(var(--accent))]">
                    {proc.memory_mb.toFixed(2)} MB
                  </td>
                  <td className="py-2.5 text-[rgb(var(--foreground-muted))]">
                    {proc.cpu_usage.toFixed(1)}%
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
