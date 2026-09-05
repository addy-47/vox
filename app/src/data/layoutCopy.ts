export const LAYOUT_COPY = {
  nav: {
    monitor: "Monitor",
    engineMonitor: "Engine Monitor",
    openProfiler: "Open UI Memory Profiler",
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
  drawer: {
    defaultAria: "Drawer",
    resizeFallback: "Resize drawer",
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
