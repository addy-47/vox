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
    model_ready: "new voice available",
    model_failed: "setup hit a snag",
    memory_issue: "memory needs a look",
    storage_health: "storage needs a look",
  },
} as const;
