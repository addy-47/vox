export const LAYOUT_COPY = {
  nav: {
    monitor: "System Monitor (Ctrl+M)",
    engineMonitor: "Engine Monitor",
    openProfiler: "Open UI Memory Profiler (Shift+Up)",
    loadingSurface: "Loading surface...",
    preparingEnvironment: "Preparing neural environment",
  },
  layoutFlip: {
    title: "Adjusting layout…",
    subtitle: "Settling into the new view",
  },
  titleBar: {
    close: "Close",
    minimize: "Minimize",
    maximize: "Maximize",
    appUpdate: "App Update Available",
    modelUpdates: "Model Updates",
    modelsUpdate: "Models Update",
    copyCommand: "Copy command",
    whatsNew: "What's New:",
  },
  errorBoundary: {
    title: "Render Error",
    fallback: "An unexpected error occurred",
    stackTrace: "Stack Trace",
    retry: "Retry",
    home: "Home",
  },
  toast: {
    dismiss: "Dismiss",
  },
  boot: {
    title: "SYNCHRONIZING",
    subtitle: "Preparing neural models and interface",
    status: "VOX RUNTIME READY",
  },
  /**
   * Onboarding status could not be read. Shown instead of silently routing a
   * configured install into the setup wizard: a failed IPC must never be
   * interpreted as "this machine has never been set up".
   */
  /**
   * Placeholder rendered in the `main` webview while setup is incomplete. The
   * real setup flow runs in a dedicated `wizard` webview; this exists so the
   * setup UI is not mounted a second time inside `main`.
   */
  setupWindow: {
    title: "SETUP RUNNING",
    body: "Vox's setup window is a separate window. If it isn't visible, bring it forward to continue.",
    bringForward: "Show setup window",
  },
  setupUnreachable: {
    title: "Couldn't reach the Vox backend",
    body: "Vox couldn't read your setup status, so it doesn't know whether this machine is configured. Retrying automatically.",
    bodyFinal: "Vox still can't reach the backend. Starting setup is usually safe on a configured machine — your existing settings are not deleted.",
    retrying: (attempt: number) => `Retrying… (attempt ${attempt})`,
    retry: "Retry now",
    startSetup: "Start setup anyway",
    continueAnyway: "Continue to the app",
  },
  drawer: {
    defaultAria: "Drawer",
    resizeFallback: "Resize drawer",
    close: "Close drawer",
  },
  modal: {
    defaultAria: "Dialog",
    close: "Close dialog",
  },
  panel: {
    close: "Close panel",
    fallback: "Panel",
  },
  carousel: {
    next: "Next",
    nextItem: "Next item",
    previous: "Previous",
    previousItem: "Previous item",
  },
  knob: {
    decrease: "Decrease Value",
    increase: "Increase Value",
    hint: "Drag up/down or use - / + steppers. Double-click to reset",
  },
} as const;
