/**
 * Shortcuts registry — single SSOT for all keyboard bindings.
 * Every shortcut referenced in Help panels, tooltips, and inline
 * copy MUST be defined here. Adding a binding without a registry
 * entry is a spec violation.
 *
 * Fields:
 *  - id:    unique key (e.g. "home.ptt")
 *  - keys:  display string (e.g. "Space", "Ctrl+M")
 *  - label: human description
 *  - scope: "global" (all pages) or "page:<route>" (Home, History, Memory, Settings, Monitoring, Wizard)
 *  - note:  optional clarification (shown in Help map, omitted from tooltips)
 */
export interface ShortcutDef {
  id: string;
  keys: string;
  label: string;
  scope: "global" | `page:${string}`;
  note?: string;
}

export const SHORTCUTS: ShortcutDef[] = [
  // ── Application & Window Lifecycle ──
  { id: "app.close", keys: "Ctrl+W", label: "Close window to tray", scope: "global" },
  { id: "app.quit", keys: "Ctrl+Q", label: "Quit application", scope: "global" },

  // ── Global Rails & Panels ──
  { id: "global.monitor", keys: "Ctrl+M", label: "Toggle monitoring popover", scope: "global" },
  { id: "global.notifications", keys: "Ctrl+N", label: "Toggle notifications rail", scope: "global" },
  { id: "global.sessions", keys: "Ctrl+S", label: "Toggle sessions rail", scope: "global", note: "prevents browser Save" },
  { id: "global.help", keys: "Ctrl+/", label: "Toggle help panel", scope: "global" },
  { id: "global.help-shortcuts", keys: "?", label: "Toggle keyboard shortcuts cheat sheet", scope: "global" },

  // ── Page Navigation & Context Drawers ──
  { id: "global.page-nav-left", keys: "Shift+←", label: "Navigate to previous page", scope: "global" },
  { id: "global.page-nav-right", keys: "Shift+→", label: "Navigate to next page", scope: "global" },
  { id: "global.page-drawer-open", keys: "Shift+Up", label: "Open page drawer", scope: "global", note: "Home: profiler · History: detail panel · Memory: dossier · Settings: all cards" },
  { id: "global.page-drawer-close", keys: "Shift+Down", label: "Close page drawer", scope: "global" },
  { id: "global.spatial-move", keys: "↑ ↓ ← →", label: "Move focus spatially across elements", scope: "global" },
  { id: "global.escape", keys: "Escape", label: "Close topmost overlay / cancel edit", scope: "global" },
  { id: "global.tab", keys: "Tab", label: "Move focus to next control", scope: "global" },

  // ── Voice Pipeline ──
  { id: "home.ptt", keys: "Space", label: "Hold to talk (push-to-talk)", scope: "page:home", note: "only when engaged + PTT mode; not when paused/sleeping/error" },
  { id: "home.mute", keys: "M", label: "Toggle microphone mute", scope: "page:home" },
  { id: "home.speaker-mute", keys: "Shift+M", label: "Toggle speaker / playback mute", scope: "page:home" },
  { id: "home.pause-resume", keys: "P", label: "Pause / resume voice pipeline", scope: "page:home" },
  { id: "home.text-mode", keys: "T", label: "Open text input mode", scope: "page:home" },
  { id: "home.engage", keys: "Ctrl+Space", label: "Engage / sleep voice agent", scope: "page:home" },
  { id: "home.profiler", keys: "Shift+Up", label: "Open profiler drawer", scope: "page:home" },

  // ── History ──
  { id: "history.navigate", keys: "← / →", label: "Navigate and orbit sessions", scope: "page:history" },
  { id: "history.select", keys: "Enter / Space", label: "Select session / open detail panel", scope: "page:history" },
  { id: "history.delete", keys: "Delete", label: "Arm delete on focused session", scope: "page:history", note: "Enter confirms, Escape cancels" },
  { id: "history.page-drawer", keys: "Shift+Up", label: "Open detail panel (when session selected)", scope: "page:history", note: "no-op when no session selected" },

  // ── Memory ──
  { id: "memory.search", keys: "Ctrl+K", label: "Focus search bar", scope: "page:memory" },
  { id: "memory.recenter", keys: "Ctrl+R", label: "Recenter graph view", scope: "page:memory" },
  { id: "memory.zoomIn", keys: "Shift+=", label: "Zoom in graph", scope: "page:memory" },
  { id: "memory.zoomOut", keys: "Shift+-", label: "Zoom out graph", scope: "page:memory" },
  { id: "memory.select", keys: "Enter / Space", label: "Select node / open dossier", scope: "page:memory" },
  { id: "memory.page-drawer", keys: "Shift+Up", label: "Open personal memory dossier", scope: "page:memory" },

  // ── Settings ──
  { id: "settings.all-cards", keys: "Shift+Up", label: "Open all domain cards", scope: "page:settings" },
  { id: "settings.close-cards", keys: "Shift+Down", label: "Close all domain cards", scope: "page:settings" },

  // ── Wizard ──
  { id: "wizard.next", keys: "Enter", label: "Advance when CTA focused", scope: "page:wizard" },

  // ── Universal shortcuts for controls ──
  { id: "ctrl-enter-save", keys: "Ctrl+Enter", label: "Save / commit in multi-line editors", scope: "global" },
];

export function getShortcutById(id: string): ShortcutDef | undefined {
  return SHORTCUTS.find((s) => s.id === id);
}

export function getShortcutsGroupedByRoute(): Record<string, ShortcutDef[]> {
  const routes = new Set<string>();
  for (const s of SHORTCUTS) {
    if (s.scope === "global") routes.add("Global");
    else routes.add(s.scope.replace("page:", ""));
  }
  const result: Record<string, ShortcutDef[]> = {};
  for (const route of routes) {
    result[route] = SHORTCUTS.filter(
      (s) => (route === "Global" ? s.scope === "global" : s.scope === `page:${route}`)
    );
  }
  return result;
}

export function getShortcutsForRoute(route: string): ShortcutDef[] {
  if (route === "Global") return SHORTCUTS.filter((s) => s.scope === "global");
  return SHORTCUTS.filter((s) => s.scope === `page:${route}`);
}

/** Build tooltip suffix like " (Ctrl+M)" from a shortcut id */
export function shortcutSuffix(id: string): string {
  const s = getShortcutById(id);
  return s ? ` (${s.keys})` : "";
}
