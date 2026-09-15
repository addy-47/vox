import { memo, useMemo, lazy, Suspense } from "react";
import { useLocation } from "react-router-dom";
import {
  Sparkles,
  History as HistoryIcon,
  Brain,
  SlidersHorizontal,
  Activity,
} from "lucide-react";
import { ErrorBoundary } from "./ErrorBoundary";

const HomeHelpContent = lazy(() =>
  import("@/shared/components/help/HomeHelpContent").then((m) => ({ default: m.HomeHelpContent }))
);
const HistoryHelpContent = lazy(() =>
  import("@/shared/components/help/HistoryHelpContent").then((m) => ({ default: m.HistoryHelpContent }))
);
const MemoryHelpContent = lazy(() =>
  import("@/shared/components/help/MemoryHelpContent").then((m) => ({ default: m.MemoryHelpContent }))
);
const SettingsHelpContent = lazy(() =>
  import("@/shared/components/help/SettingsHelpContent").then((m) => ({ default: m.SettingsHelpContent }))
);

export interface HelpPanelProps {
  onClose?: () => void;
  deepLink?: string | null;
}

export const HelpPanel = memo(({ onClose: _onClose }: HelpPanelProps) => {
  const { pathname } = useLocation();

  // Route-based exclusive page configuration
  const pageMeta = useMemo(() => {
    if (pathname.startsWith("/history")) {
      return {
        id: "history",
        title: "History Guide",
        routeBadge: "/history",
        icon: HistoryIcon,
      };
    }
    if (pathname.startsWith("/memory")) {
      return {
        id: "memory",
        title: "Memory Guide",
        routeBadge: "/memory",
        icon: Brain,
      };
    }
    if (pathname.startsWith("/settings")) {
      return {
        id: "settings",
        title: "Settings Guide",
        routeBadge: "/settings",
        icon: SlidersHorizontal,
      };
    }
    if (pathname.startsWith("/monitoring")) {
      return {
        id: "monitoring",
        title: "Monitoring Guide",
        routeBadge: "/monitoring",
        icon: Activity,
      };
    }
    return {
      id: "home",
      title: "Workspace Guide",
      routeBadge: "/",
      icon: Sparkles,
    };
  }, [pathname]);

  const Icon = pageMeta.icon;

  return (
    <div className="flex flex-col h-full min-h-0 select-none">
      {/* ── Minimal Context Header ── */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-[rgba(var(--border),0.08)] bg-[rgba(var(--background),0.25)] shrink-0">
        <div className="flex items-center gap-2">
          <span className="w-6 h-6 rounded-md border border-[rgba(var(--accent),0.25)] bg-[rgba(var(--accent),0.08)] text-[rgb(var(--accent))] flex items-center justify-center shrink-0">
            <Icon size={13} strokeWidth={2} />
          </span>
          <h2 className="font-display text-[13.5px] font-bold tracking-wide text-[rgb(var(--foreground))]">
            {pageMeta.title}
          </h2>
        </div>
        <span className="font-mono text-[10.5px] px-2 py-0.5 rounded-full border border-[rgba(var(--accent),0.2)] bg-[rgba(var(--accent),0.06)] text-[rgb(var(--accent))] font-semibold">
          {pageMeta.routeBadge}
        </span>
      </div>

      {/* ── Scrollable Exclusive Page Content ── */}
      <div className="flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar px-4 pt-4 pb-28">
        <ErrorBoundary name={`HelpGuide:${pageMeta.id}`}>
          <Suspense fallback={<div className="p-4 text-[12px] font-mono text-[rgb(var(--foreground-muted))]/60">Loading guide...</div>}>
            {pageMeta.id === "history" && <HistoryHelpContent />}
            {pageMeta.id === "memory" && <MemoryHelpContent />}
            {pageMeta.id === "settings" && <SettingsHelpContent />}
            {(pageMeta.id === "home" || pageMeta.id === "monitoring") && <HomeHelpContent />}
          </Suspense>
        </ErrorBoundary>
      </div>
    </div>
  );
});

HelpPanel.displayName = "HelpPanel";
