export const GOVERNOR_LABELS: Record<string, string> = {
  powersave: "Power Saver",
  performance: "High Performance",
  schedutil: "Balanced",
  ondemand: "Adaptive",
};

export const HOME_CONTROLS_COPY = {
  engage: {
    ariaLabel: "Engage Vox",
    resumeAriaLabel: "Resume Vox Session",
    resumeBadge: "Resume Session",
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
    sendTooltip: "Send (Enter)",
    discardAriaLabel: "Discard & Close",
    discardTooltip: "Discard & Close (Esc)",
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
    toggleAriaLabel: "Toggle Temporary Session",
    toggleTooltip: "Temporary Session (Memory Only)",
    activeBadge: "Temporary",
  },
} as const;

export const ERROR_BANNER_COPY = {
  dismissButton: "Dismiss",
} as const;

export const DIALOGUE_COPY = {
  userBadge: "USER",
  assistantBadge: "VOX",
} as const;

