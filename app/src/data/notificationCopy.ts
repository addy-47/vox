/**
 * Static copy for notifications. All user-facing strings live here —
 * components must not hardcode notification labels. Copy stays layman:
 * no session ids, no engine jargon.
 */
export const NOTIFICATION_COPY = {
  bellAriaLabel: "Notifications",
  title: "Notifications",
  markAllRead: "Mark all read",
  emptyTitle: "All caught up",
  emptySubtitle: "Session reminders and background updates will appear here.",
  dismiss: "Dismiss",
  view: "View",
  tidyNow: "Tidy now",
  tidying: "Tidying...",
  consolidate: "Consolidate",
  consolidating: "Consolidating...",
  openSettings: "Open settings",
  retry: "Retry",
  retrying: "Retrying...",
  openSetup: "Open setup",
  justNow: "Just now",
  turnSingular: "turn",
  turnPlural: "turns",
  statusLabels: {
    in_progress: "Working on it",
    completed: "Done",
    failed: "Needs attention",
    pending: "New",
  },
  categoryBlurb: {
    session_compaction: "unsaved memories",
    memory_consolidation: "daily memory profile",
    pipeline: "voice pipeline notice",
    dictation: "dictation notice",
    hardware: "hardware alert",
    models: "model asset alert",
    storage: "storage health",
  },
} as const;
