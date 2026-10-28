export const GOVERNOR_LABELS: Record<string, string> = {
  powersave: "Power Saver",
  performance: "High Performance",
  schedutil: "Balanced",
  ondemand: "Adaptive",
};

export const HOME_CONTROLS_COPY = {
  engage: {
    ariaLabel: "Engage Vox",
    stopAriaLabel: "Stop Vox",
  },
  passive: {
    pauseAriaLabel: "Pause Vox",
    resumeAriaLabel: "Resume Vox",
    pauseTooltip: "Pause Audio & Passthrough",
    resumeTooltip: "Resume Audio & Passthrough",
  },
  ptt: {
    micAriaLabel: "Hold to Talk (Push-To-Talk)",
    micTooltip: "Hold or Tap to Talk",
  },
  error: {
    reconnectAriaLabel: "Reconnect Vox Session",
    reconnectLabel: "Reconnect",
    reconnectTooltip: "Attempt Session Reconnection",
  },
  textMode: {
    toggleAriaLabel: "Toggle Text Input Mode",
    toggleTooltip: "Type Query (Text Input)",
    placeholder: "Type a message to Vox...",
    sendAriaLabel: "Send Message",
    sendTooltip: "Send",
    discardAriaLabel: "Discard & Close",
    discardTooltip: "Discard & Close",
    muteSpeakerAriaLabel: "Mute Speaker Output",
    muteSpeakerTooltip: "Mute Speaker Output",
    unmuteSpeakerAriaLabel: "Unmute Speaker Output",
    unmuteSpeakerTooltip: "Unmute Speaker Output",
    muteMicAriaLabel: "Mute Microphone",
    muteMicTooltip: "Mute Microphone",
    unmuteMicAriaLabel: "Unmute Microphone",
    unmuteMicTooltip: "Unmute Microphone",
  },
  temporary: {
    toggleAriaLabel: "Toggle Incognito Mode",
    toggleTooltip: "Incognito Mode (RAM only — nothing saved to disk)",
    activeBadge: "Incognito",
  },
  /** Prefix on the Home error banner shown when the rocket dial to the paired host fails. */
  remoteFailurePrefix: "Remote connect failed:",
  /**
   * Host-side status for the remote link (`RemoteLinkState` → badge). The
   * single status badge carries remote state; there is no separate REMOTE
   * kicker.
   */
  remoteStatus: {
    connected: "Remote Session",
    reconnecting: "Reconnecting",
    connecting: "Linking",
    failed: "Remote Failed",
  },
  /** Accessible name for the pipeline status capsule. */
  statusAriaLabel: (label: string) => `Vox Status: ${label}`,
} as const;

export const ERROR_BANNER_COPY = {
  dismissButton: "Dismiss",
} as const;

export const DIALOGUE_COPY = {
  userBadge: "USER",
  assistantBadge: "VOX",
  readMore: "Read more",
  readLess: "Show less",
  toolCallInputs: "Inputs:",
  toolCallOutputs: "Output:",
} as const;

