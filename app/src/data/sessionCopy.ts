/**
 * Static copy for the conversation session rail (session continuation).
 * All user-facing strings for session discovery, selection, restore, and
 * recency live here — components must not hardcode session labels.
 */
export const SESSION_COPY = {
  railTitle: "Conversations",
  railAriaLabel: "Conversation list",
  openRailAriaLabel: "Open conversations",
  closeRailAriaLabel: "Close conversations",
  newConversation: "New conversation",
  newConversationAriaLabel: "Start a new conversation",
  untitledSession: "Untitled conversation",
  loadingSessions: "Loading conversations...",
  noSessionsTitle: "No conversations yet",
  noSessionsDesc: "Finished voice sessions will appear here for continuation.",
  sessionsFailed: "Failed to load conversations.",
  retry: "Retry",
  restoreFailedFallback: "Failed to restore conversation.",
  restoringAriaLabel: "Restoring conversation",
  pinAriaLabel: "Pin conversation",
  unpinAriaLabel: "Unpin conversation",
  pinnedSection: "Pinned",
  projectsSection: "Projects",
  conversationsSection: "Conversations",
  uncategorizedSection: "More conversations",
  noPinnedSessions: "No pinned conversations",
  noProjectsYet: "No projects yet",
  noProjectSessions: "No conversations in this project",
  noOtherSessions: "No other conversations",
  newProjectPlaceholder: "New project name",
  createProjectAriaLabel: "Create project",
  turnSingular: "turn",
  turnPlural: "turns",
  recency: {
    justNow: "Just now",
    minutesAgo: "{n}m ago",
    hoursAgo: "{n}h ago",
    yesterday: "Yesterday",
  },
} as const;
