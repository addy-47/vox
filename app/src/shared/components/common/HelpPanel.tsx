import { memo, useMemo, useState, useEffect, lazy, Suspense } from "react";
import { useLocation } from "react-router-dom";
import {
  Sparkles,
  History as HistoryIcon,
  Brain,
  SlidersHorizontal,
  Activity,
  Key,
} from "lucide-react";
import { ErrorBoundary } from "./ErrorBoundary";
import { getShortcutsGroupedByRoute } from "@/data/shortcuts";

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
  initialShortcuts?: boolean;
}

export const HelpPanel = memo(({ onClose: _onClose, initialShortcuts = false }: HelpPanelProps) => {
  const { pathname } = useLocation();
  const [showShortcuts, setShowShortcuts] = useState(initialShortcuts);

  useEffect(() => {
    if (initialShortcuts) {
      setShowShortcuts(true);
    }
  }, [initialShortcuts]);

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
  const grouped = useMemo(() => getShortcutsGroupedByRoute(), []);

  // Escape exits the shortcut map (back to route guide); overlayStack closes panel after
  useEffect(() => {
    if (!showShortcuts) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        setShowShortcuts(false);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [showShortcuts]);

  return (
    <div className="flex flex-col h-full min-h-0 select-none">
      {/* ── Minimal Context Header ── */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-[rgba(var(--border),0.08)] bg-[rgba(var(--background),0.25)] shrink-0">
        <div className="flex items-center gap-2">
          <span className="w-6 h-6 rounded-md border border-[rgba(var(--accent),0.25)] bg-[rgba(var(--accent),0.08)] text-[rgb(var(--accent))] flex items-center justify-center shrink-0">
            <Icon size={13} strokeWidth={2} />
          </span>
          <h2 className="font-display text-[13.5px] font-bold tracking-wide text-[rgb(var(--foreground))]">
            {showShortcuts ? "Keyboard Shortcuts" : pageMeta.title}
          </h2>
        </div>
        <div className="flex items-center gap-2">
          {!showShortcuts && (
            <span className="font-mono text-[10.5px] px-2 py-0.5 rounded-full border border-[rgba(var(--accent),0.2)] bg-[rgba(var(--accent),0.06)] text-[rgb(var(--accent))] font-semibold">
              {pageMeta.routeBadge}
            </span>
          )}
          <button
            type="button"
            onClick={() => setShowShortcuts((v) => !v)}
            aria-label={showShortcuts ? "Show route guide" : "Show all shortcuts"}
            aria-pressed={showShortcuts}
            className="flex items-center justify-center w-6 h-6 rounded-md border border-[rgba(var(--border),0.15)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-colors cursor-pointer shrink-0"
            title={showShortcuts ? "Show guide" : "Show shortcuts"}
          >
            <Key size={13} strokeWidth={2} />
          </button>
        </div>
      </div>

      {/* ── Scrollable Content ── */}
      <div className="flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar px-4 pt-4 pb-28">
        {showShortcuts ? (
          <ErrorBoundary name="HelpShortcutsMap">
            <div className="space-y-4">
              {Object.entries(grouped).map(([route, shortcuts]) => (
                <div key={route}>
                  <h3 className="font-display text-[12px] font-bold uppercase tracking-[0.15em] text-[rgb(var(--accent))] mb-2">
                    {route}
                  </h3>
                  <div className="space-y-1.5">
                    {shortcuts.map((s) => (
                      <div key={s.id} className="flex items-center gap-3 py-1">
                        <kbd className="font-mono text-[11px] px-2 py-0.5 rounded bg-[rgba(var(--accent),0.1)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.2)] font-semibold shrink-0">
                          {s.keys}
                        </kbd>
                        <span className="text-[12.5px] text-[rgb(var(--foreground))]">{s.label}</span>
                        {s.note && (
                          <span className="text-[11px] text-[rgb(var(--foreground-muted))] italic">— {s.note}</span>
                        )}
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </ErrorBoundary>
        ) : (
          <Suspense fallback={<div className="p-4 text-[12px] font-mono text-[rgb(var(--foreground-muted))]/60">Loading guide...</div>}>
            {pageMeta.id === "history" && <HistoryHelpContent />}
            {pageMeta.id === "memory" && <MemoryHelpContent />}
            {pageMeta.id === "settings" && <SettingsHelpContent />}
            {(pageMeta.id === "home" || pageMeta.id === "monitoring") && <HomeHelpContent />}
          </Suspense>
        )}
      </div>
    </div>
  );
});

HelpPanel.displayName = "HelpPanel";
