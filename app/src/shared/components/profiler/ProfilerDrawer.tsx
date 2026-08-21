import React, { createContext, useContext, useState } from "react";
import { Activity, RefreshCw, CheckCircle2 } from "lucide-react";
import { useMemoryProfiler } from "@/shared/hooks/useMemoryProfiler";
import { useMemoryProfilerContext } from "@/shared/context/MemoryProfilerContext";
import { Tooltip } from "@/shared/ui/Tooltip";
import { Drawer } from "@/shared/ui/Drawer";
import { cn } from "@/shared/lib/utils";
import { PROFILER_COPY } from "@/data/profilerCopy";
import { ProfilerPanel } from "./ProfilerPanel";

interface ProfilerDrawerContextValue {
  /** Open the global memory-profiler bottom drawer from anywhere. */
  openProfiler: () => void;
}

const ProfilerDrawerContext = createContext<ProfilerDrawerContextValue | null>(null);

export function useProfilerDrawer(): ProfilerDrawerContextValue {
  const ctx = useContext(ProfilerDrawerContext);
  if (!ctx) throw new Error("useProfilerDrawer must be used within ProfilerDrawerProvider");
  return ctx;
}

interface ProfilerDrawerProps {
  open: boolean;
  onClose: () => void;
}

/**
 * App-level bottom drawer hosting the memory profiler. Sampling is lazy — it
 * starts when the drawer opens and stops when it closes (active = `open`), but
 * the sampled state persists across open/close cycles because the hook lives in
 * this always-mounted component.
 */
const ProfilerDrawer: React.FC<ProfilerDrawerProps> = ({ open, onClose }) => {
  const {
    latestSnapshot,
    history,
    pageRecords,
    jsHeap,
    domStats,
    cssStats,
    isSampling,
    lastManualSnapshot,
    currentRoute,
    captureSnapshot,
  } = useMemoryProfiler(open);

  const { componentTraces } = useMemoryProfilerContext();

  return (
    <Drawer
      open={open}
      onClose={onClose}
      position="global"
      ariaLabel="UI Memory Profiler"
      height={75}
      resizeHint="Drag to resize · double-click to expand"
      icon={
        <div className="p-2.5 rounded-xl bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/25 text-[rgb(var(--accent))]">
          <Activity size={20} />
        </div>
      }
      title={
        <h2 className="font-display text-[15px] font-bold tracking-wide text-[rgb(var(--foreground))]">
          {PROFILER_COPY.headerTitle}
        </h2>
      }
      subtitle={
        <p className="text-[11px] text-[rgb(var(--foreground-muted))] font-sans mt-0.5">
          {PROFILER_COPY.headerSubtitle}
        </p>
      }
      headerActions={
        <>
          {lastManualSnapshot && (
            <div className="hidden sm:flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 text-[11px] font-mono animate-fade-in">
              <CheckCircle2 size={12} className="shrink-0 text-emerald-400" />
              <span className="truncate max-w-[260px]">
                temp/{lastManualSnapshot.filename} ({new Date(lastManualSnapshot.timestampMs).toLocaleTimeString()})
              </span>
            </div>
          )}

          <div className="flex items-center gap-2 px-3 py-1.5 rounded-xl bg-[rgba(var(--card),0.8)] border border-[rgba(var(--border),0.15)] text-[11px] font-mono">
            <span
              className={cn(
                "w-2 h-2 rounded-full",
                isSampling ? "bg-[rgb(var(--accent))] animate-ping" : "bg-[rgb(var(--accent))]"
              )}
            />
            <span className="text-[rgb(var(--foreground-muted))]">On-Demand</span>
          </div>

          <Tooltip label="Trigger immediate on-demand OS process sample & write snapshot to temp/">
            <button
              onClick={() => captureSnapshot({ isManual: true })}
              disabled={isSampling}
              className="flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-sans font-bold uppercase tracking-wider bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/30 transition-all border border-[rgb(var(--accent))]/30 cursor-pointer disabled:opacity-50"
            >
              <RefreshCw size={14} className={cn(isSampling && "animate-spin")} />
              <span>{isSampling ? "Capturing..." : PROFILER_COPY.snapshotButton}</span>
            </button>
          </Tooltip>
        </>
      }
      bodyClassName="px-6"
    >
      <ProfilerPanel
        latestSnapshot={latestSnapshot}
        history={history}
        pageRecords={pageRecords}
        jsHeap={jsHeap}
        domStats={domStats}
        cssStats={cssStats}
        componentTraces={componentTraces}
        currentRoute={currentRoute}
      />
    </Drawer>
  );
};

/**
 * Provides the global profiler drawer + an imperative `openProfiler()` handle.
 * Place near the top of the layout so any routed page can open the drawer.
 */
export const ProfilerDrawerProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [open, setOpen] = useState(false);

  return (
    <ProfilerDrawerContext.Provider value={{ openProfiler: () => setOpen(true) }}>
      {children}
      <ProfilerDrawer open={open} onClose={() => setOpen(false)} />
    </ProfilerDrawerContext.Provider>
  );
};