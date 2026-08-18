import React, { useState } from "react";
import {
  Activity,
  Layers,
  Sparkles,
  RefreshCw,
  Trash2,
  Download,
  CheckCircle2,
  AlertTriangle,
  HelpCircle,
  Box,
  Terminal,
  FileCode,
} from "lucide-react";
import { useMemoryProfiler } from "@/shared/hooks/useMemoryProfiler";
import { useMemoryProfilerContext } from "@/shared/context/MemoryProfilerContext";
import { Tooltip } from "@/shared/ui/Tooltip";
import { cn } from "@/shared/lib/utils";

export const MemoryProfiler: React.FC = () => {
  const {
    latestSnapshot,
    pageRecords,
    jsHeap,
    domStats,
    cssStats,
    isSampling,
    currentRoute,
    captureSnapshot,
    clearHistory,
  } = useMemoryProfiler(true, 2500);

  const { componentTraces } = useMemoryProfilerContext();
  const [selectedProcessPid, setSelectedProcessPid] = useState<number | null>(null);

  const exportDiagnostics = () => {
    const data = {
      timestamp: new Date().toISOString(),
      currentRoute,
      latestSnapshot,
      pageRecords,
      jsHeap,
      domStats,
      cssStats,
      componentTraces,
    };
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `vox-memory-profile-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const getAccuracyBadge = (type: "Measured" | "Estimated" | "Correlated" | "Unattributed") => {
    switch (type) {
      case "Measured":
        return (
          <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-mono uppercase bg-emerald-500/15 text-emerald-400 border border-emerald-500/25">
            <CheckCircle2 size={11} /> Measured
          </span>
        );
      case "Estimated":
        return (
          <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-mono uppercase bg-amber-500/15 text-amber-400 border border-amber-500/25">
            <AlertTriangle size={11} /> Estimated
          </span>
        );
      case "Correlated":
        return (
          <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-mono uppercase bg-purple-500/15 text-purple-400 border border-purple-500/25">
            <Sparkles size={11} /> Correlated
          </span>
        );
      default:
        return (
          <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-mono uppercase bg-slate-500/15 text-slate-400 border border-slate-500/25">
            <HelpCircle size={11} /> Unattributed
          </span>
        );
    }
  };

  const trackedPages = [
    { name: "Home", route: "/" },
    { name: "History", route: "/history" },
    { name: "Memory", route: "/memory" },
    { name: "Settings", route: "/settings" },
    { name: "Monitoring", route: "/monitoring" },
    { name: "Profiler", route: "/memory-profiler" },
  ];

  return (
    <div className="flex-1 flex flex-col h-full w-full overflow-y-auto px-6 py-6 space-y-6 text-[rgb(var(--foreground))]">
      {/* Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 border-b border-[rgba(var(--border),0.12)] pb-4">
        <div>
          <div className="flex items-center gap-2.5">
            <div className="p-2 rounded-xl bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20 text-[rgb(var(--accent))]">
              <Activity size={20} />
            </div>
            <div>
              <h1 className="font-display text-lg font-bold tracking-wide">
                UI Memory Attribution & RCA Profiler
              </h1>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] font-sans mt-0.5">
                Developer diagnostics for multi-WebView memory attribution, page deltas, and resource lifecycles
              </p>
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <Tooltip label="Trigger immediate on-demand OS process sample">
            <button
              onClick={() => captureSnapshot()}
              disabled={isSampling}
              className="flex items-center gap-2 px-3.5 py-2 rounded-xl text-[11px] font-sans font-bold uppercase tracking-wider bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/30 transition-all border border-[rgb(var(--accent))]/30 cursor-pointer disabled:opacity-50"
            >
              <RefreshCw size={14} className={cn(isSampling && "animate-spin")} />
              <span>Snapshot Now</span>
            </button>
          </Tooltip>

          <Tooltip label="Export full diagnostic timeline as JSON">
            <button
              onClick={exportDiagnostics}
              className="flex items-center gap-2 px-3 py-2 rounded-xl text-[11px] font-sans font-bold tracking-wider glass-card hover:bg-white/[0.06] transition-all border border-[rgba(var(--border),0.15)] cursor-pointer"
            >
              <Download size={14} />
              <span>Export</span>
            </button>
          </Tooltip>

          <Tooltip label="Reset recorded page baselines and history">
            <button
              onClick={clearHistory}
              className="p-2 rounded-xl glass-card hover:text-red-400 hover:border-red-500/30 transition-all border border-[rgba(var(--border),0.15)] cursor-pointer"
            >
              <Trash2 size={16} />
            </button>
          </Tooltip>
        </div>
      </div>

      {/* Surface-Level Attribution Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {/* Total Vox RAM */}
        <div className="glass-card p-4 rounded-2xl border border-[rgba(var(--accent),0.18)] bg-[rgb(var(--card))]/80 backdrop-blur-xl relative overflow-hidden">
          <div className="flex items-center justify-between">
            <span className="text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
              Total Vox RAM (Tree)
            </span>
            {getAccuracyBadge("Measured")}
          </div>
          <div className="mt-2 flex items-baseline gap-2">
            <span className="font-mono text-2xl font-bold text-[rgb(var(--accent))]">
              {latestSnapshot ? `${latestSnapshot.total_vox_ram_mb.toFixed(1)}` : "--"}
            </span>
            <span className="font-mono text-xs text-[rgb(var(--foreground-muted))]">MB</span>
          </div>
          <p className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] mt-1">
            Parent process + all WebKit child renderers
          </p>
        </div>

        {/* Main WebView */}
        <div className="glass-card p-4 rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgb(var(--card))]/80 backdrop-blur-xl">
          <div className="flex items-center justify-between">
            <span className="text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
              Main WebView
            </span>
            {latestSnapshot?.main_webview_ram_mb != null
              ? getAccuracyBadge("Measured")
              : getAccuracyBadge("Unattributed")}
          </div>
          <div className="mt-2 flex items-baseline gap-2">
            <span className="font-mono text-2xl font-bold text-sky-400">
              {latestSnapshot?.main_webview_ram_mb != null
                ? `${latestSnapshot.main_webview_ram_mb.toFixed(1)}`
                : "--"}
            </span>
            <span className="font-mono text-xs text-[rgb(var(--foreground-muted))]">MB</span>
          </div>
          <p className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] mt-1">
            Primary application surface
          </p>
        </div>

        {/* Tray WebView */}
        <div className="glass-card p-4 rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgb(var(--card))]/80 backdrop-blur-xl">
          <div className="flex items-center justify-between">
            <span className="text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
              Tray WebView
            </span>
            {latestSnapshot?.tray_webview_ram_mb != null
              ? getAccuracyBadge("Measured")
              : getAccuracyBadge("Unattributed")}
          </div>
          <div className="mt-2 flex items-baseline gap-2">
            <span className="font-mono text-2xl font-bold text-indigo-400">
              {latestSnapshot?.tray_webview_ram_mb != null
                ? `${latestSnapshot.tray_webview_ram_mb.toFixed(1)}`
                : "--"}
            </span>
            <span className="font-mono text-xs text-[rgb(var(--foreground-muted))]">MB</span>
          </div>
          <p className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] mt-1">
            Persistent HUD overlay window
          </p>
        </div>

        {/* Rust Core Process */}
        <div className="glass-card p-4 rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgb(var(--card))]/80 backdrop-blur-xl">
          <div className="flex items-center justify-between">
            <span className="text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
              Rust Core Backend
            </span>
            {getAccuracyBadge("Measured")}
          </div>
          <div className="mt-2 flex items-baseline gap-2">
            <span className="font-mono text-2xl font-bold text-emerald-400">
              {latestSnapshot ? `${latestSnapshot.main_process_ram_mb.toFixed(1)}` : "--"}
            </span>
            <span className="font-mono text-xs text-[rgb(var(--foreground-muted))]">MB</span>
          </div>
          <p className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] mt-1">
            Tauri host, audio pipeline, ONNX engines
          </p>
        </div>
      </div>

      {/* Page-by-Page Memory Attribution Matrix */}
      <div className="glass-card rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgb(var(--card))]/80 backdrop-blur-xl p-5 space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Layers size={18} className="text-[rgb(var(--accent))]" />
            <h2 className="font-display text-sm font-bold tracking-wide">
              Page Lifecycle Attribution Matrix
            </h2>
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
              {trackedPages.map((page) => {
                const rec = pageRecords[page.route];
                const isCurrent = currentRoute === page.route;
                const baselineMb = rec?.baseline?.total_vox_ram_mb;
                const currentMb = rec?.current?.total_vox_ram_mb;
                const peakMb = rec?.peak?.total_vox_ram_mb;
                const peakDelta = rec?.peakDeltaMb;
                const retainedMb = rec?.retained?.total_vox_ram_mb;
                const retainedDelta = rec?.retainedDeltaMb;

                let riskBadge = (
                  <span className="text-emerald-400 font-sans text-[11px] font-medium">Normal</span>
                );
                if (retainedDelta !== null && retainedDelta !== undefined) {
                  if (retainedDelta > 40) {
                    riskBadge = (
                      <span className="text-red-400 font-sans text-[11px] font-bold flex items-center gap-1">
                        <AlertTriangle size={12} /> Critical Retention (+{retainedDelta}MB)
                      </span>
                    );
                  } else if (retainedDelta > 15) {
                    riskBadge = (
                      <span className="text-amber-400 font-sans text-[11px] font-semibold flex items-center gap-1">
                        <AlertTriangle size={12} /> Suspicious (+{retainedDelta}MB)
                      </span>
                    );
                  }
                }

                return (
                  <tr
                    key={page.route}
                    className={cn(
                      "hover:bg-white/[0.02] transition-colors",
                      isCurrent && "bg-[rgb(var(--accent))]/5"
                    )}
                  >
                    <td className="py-3 font-sans font-bold flex items-center gap-2">
                      <span
                        className={cn(
                          "w-2 h-2 rounded-full",
                          isCurrent ? "bg-[rgb(var(--accent))]" : "bg-slate-600"
                        )}
                      />
                      <span>{page.name}</span>
                      <span className="text-[11px] text-[rgb(var(--foreground-muted))] font-mono font-normal">
                        ({page.route})
                      </span>
                    </td>
                    <td className="py-3 font-sans">
                      {isCurrent ? (
                        <span className="text-[11px] font-mono px-2 py-0.5 rounded-full bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/25">
                          Active
                        </span>
                      ) : rec?.unmountedAt ? (
                        <span className="text-[11px] font-mono px-2 py-0.5 rounded-full bg-slate-500/10 text-slate-400">
                          Unmounted
                        </span>
                      ) : (
                        <span className="text-[11px] text-slate-500">Unvisited</span>
                      )}
                    </td>
                    <td className="py-3">
                      {baselineMb !== undefined ? `${baselineMb.toFixed(1)} MB` : "--"}
                    </td>
                    <td className="py-3">
                      {currentMb !== undefined ? `${currentMb.toFixed(1)} MB` : "--"}
                    </td>
                    <td className="py-3">
                      {peakMb !== undefined ? (
                        <span>
                          {peakMb.toFixed(1)} MB{" "}
                          {peakDelta !== null && peakDelta !== undefined && peakDelta > 0 && (
                            <span className="text-amber-400 text-[11px]">
                              (+{peakDelta.toFixed(1)})
                            </span>
                          )}
                        </span>
                      ) : (
                        "--"
                      )}
                    </td>
                    <td className="py-3">
                      {retainedMb !== undefined && retainedMb !== null ? (
                        <span>
                          {retainedMb.toFixed(1)} MB{" "}
                          {retainedDelta !== null && retainedDelta !== undefined && (
                            <span
                              className={cn(
                                "text-[11px]",
                                retainedDelta > 15 ? "text-red-400 font-bold" : "text-emerald-400"
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

      {/* Layer Resources & Component Traces Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Browser & Runtime Resource Indicators */}
        <div className="glass-card rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgb(var(--card))]/80 backdrop-blur-xl p-5 space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Box size={18} className="text-indigo-400" />
              <h2 className="font-display text-sm font-bold tracking-wide">
                Browser & Resource Indicators
              </h2>
            </div>
            {getAccuracyBadge("Measured")}
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="p-3.5 rounded-xl bg-black/[0.04] dark:bg-white/[0.03] border border-[rgba(var(--border),0.08)]">
              <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] uppercase">
                DOM Node Count
              </span>
              <div className="mt-1 font-mono text-lg font-bold text-[rgb(var(--foreground))]">
                {domStats.nodeCount.toLocaleString()}
              </div>
              <span className="text-[11px] text-[rgb(var(--foreground-muted))] font-sans">
                Active DOM tree elements
              </span>
            </div>

            <div className="p-3.5 rounded-xl bg-black/[0.04] dark:bg-white/[0.03] border border-[rgba(var(--border),0.08)]">
              <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] uppercase">
                Loaded Font Faces
              </span>
              <div className="mt-1 font-mono text-lg font-bold text-[rgb(var(--foreground))]">
                {domStats.fontFaceCount}
              </div>
              <span className="text-[11px] text-[rgb(var(--foreground-muted))] font-sans">
                Registered typography faces
              </span>
            </div>

            <div className="p-3.5 rounded-xl bg-black/[0.04] dark:bg-white/[0.03] border border-[rgba(var(--border),0.08)]">
              <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] uppercase">
                JS Heap (Chromium/WebKit)
              </span>
              <div className="mt-1 font-mono text-lg font-bold text-[rgb(var(--foreground))]">
                {jsHeap.available && jsHeap.usedMb !== null ? `${jsHeap.usedMb} MB` : "Unavailable"}
              </div>
              <span className="text-[11px] text-[rgb(var(--foreground-muted))] font-sans">
                {jsHeap.available ? `Limit: ${jsHeap.limitMb} MB` : "WebKitGTK flag required"}
              </span>
            </div>

            <div className="p-3.5 rounded-xl bg-black/[0.04] dark:bg-white/[0.03] border border-[rgba(var(--border),0.08)]">
              <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] uppercase">
                Backdrop Filters / Canvases
              </span>
              <div className="mt-1 font-mono text-lg font-bold text-[rgb(var(--foreground))]">
                {cssStats.backdropFilterCount} blur / {cssStats.canvasCount} canvas
              </div>
              <span className="text-[11px] text-[rgb(var(--foreground-muted))] font-sans">
                GPU compositing indicators
              </span>
            </div>
          </div>
        </div>

        {/* Component Lifecycle Tracer */}
        <div className="glass-card rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgb(var(--card))]/80 backdrop-blur-xl p-5 space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <FileCode size={18} className="text-sky-400" />
              <h2 className="font-display text-sm font-bold tracking-wide">
                Component Lifecycle Traces
              </h2>
            </div>
            {getAccuracyBadge("Correlated")}
          </div>

          <div className="overflow-x-auto max-h-[220px]">
            {Object.keys(componentTraces).length === 0 ? (
              <div className="py-8 text-center text-[rgb(var(--foreground-muted))] font-sans text-xs">
                No component lifecycle traces recorded yet.
              </div>
            ) : (
              <table className="w-full text-left text-xs font-mono">
                <thead>
                  <tr className="border-b border-[rgba(var(--border),0.12)] text-[11px] text-[rgb(var(--foreground-muted))] uppercase">
                    <th className="pb-2">Component</th>
                    <th className="pb-2">Mounts</th>
                    <th className="pb-2">Active Instances</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-[rgba(var(--border),0.06)]">
                  {Object.values(componentTraces).map((trace) => (
                    <tr key={trace.componentName}>
                      <td className="py-2 text-[rgb(var(--foreground))] font-medium">
                        {trace.componentName}
                      </td>
                      <td className="py-2 text-slate-400">{trace.mountCount}</td>
                      <td className="py-2">
                        <span
                          className={cn(
                            "px-2 py-0.5 rounded text-[11px]",
                            trace.activeInstances > 0
                              ? "bg-emerald-500/15 text-emerald-400 border border-emerald-500/25"
                              : "text-slate-500"
                          )}
                        >
                          {trace.activeInstances}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
      </div>

      {/* Process Tree Inspector */}
      <div className="glass-card rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgb(var(--card))]/80 backdrop-blur-xl p-5 space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Terminal size={18} className="text-emerald-400" />
            <h2 className="font-display text-sm font-bold tracking-wide">
              OS Process Tree Hierarchy
            </h2>
          </div>
          {getAccuracyBadge("Measured")}
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs font-mono">
            <thead>
              <tr className="border-b border-[rgba(var(--border),0.12)] text-[11px] text-[rgb(var(--foreground-muted))] uppercase">
                <th className="pb-3">PID</th>
                <th className="pb-3">Process Name</th>
                <th className="pb-3">Inferred Role</th>
                <th className="pb-3">Memory (RSS)</th>
                <th className="pb-3">CPU %</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-[rgba(var(--border),0.06)] text-[12px]">
              {latestSnapshot?.process_tree.map((proc) => (
                <tr
                  key={proc.pid}
                  onClick={() => setSelectedProcessPid(proc.pid)}
                  className={cn(
                    "hover:bg-white/[0.03] transition-colors cursor-pointer",
                    selectedProcessPid === proc.pid && "bg-white/[0.05]"
                  )}
                >
                  <td className="py-2.5 text-[rgb(var(--foreground-muted))]">{proc.pid}</td>
                  <td className="py-2.5 font-bold text-[rgb(var(--foreground))]">{proc.name}</td>
                  <td className="py-2.5 font-sans">
                    <span
                      className={cn(
                        "px-2 py-0.5 rounded text-[11px] font-mono",
                        proc.is_main_process
                          ? "bg-emerald-500/15 text-emerald-400 border border-emerald-500/25"
                          : proc.role.includes("Main WebView")
                          ? "bg-sky-500/15 text-sky-400 border border-sky-500/25"
                          : proc.role.includes("Tray")
                          ? "bg-indigo-500/15 text-indigo-400 border border-indigo-500/25"
                          : "bg-slate-500/10 text-slate-400"
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
