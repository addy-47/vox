export interface ProfilerTabItem {
  id: "overview" | "pages" | "insights";
  label: string;
  description: string;
}

export const PROFILER_TABS: ProfilerTabItem[] = [
  {
    id: "overview",
    label: "Overview & Processes",
    description: "System RSS overview, live memory telemetry area chart, and OS process tree breakdown",
  },
  {
    id: "pages",
    label: "Page Attributions & Resources",
    description: "Per-route memory baseline, peak, retained deltas, DOM tree stats, and WebGL layer counts",
  },
  {
    id: "insights",
    label: "RCA & Event Timeline",
    description: "Heuristic leak analysis, memory diagnostics, and real-time transition event stream",
  },
];

export const TRACKED_PAGES = [
  { name: "Home", route: "/" },
  { name: "History", route: "/history" },
  { name: "Memory", route: "/memory" },
  { name: "Settings", route: "/settings" },
  { name: "Monitoring", route: "/monitoring" },
  { name: "Profiler", route: "/memory-profiler" },
];

export const PROFILER_COPY = {
  headerTitle: "UI Memory Attribution & RCA Profiler",
  headerSubtitle: "Developer diagnostics for multi-WebView memory attribution, page deltas, and resource lifecycles",
  snapshotButton: "Snapshot Now",
  noTraces: "No component lifecycle traces recorded yet.",
  noTimeline: "No memory events recorded in this session yet. Navigate across pages or capture a snapshot.",
  accuracy: {
    measured: "Measured",
    estimated: "Estimated",
    correlated: "Correlated",
    unattributed: "Unattributed",
  },
  drawer: {
    triggerAria: "UI Memory Profiler",
    onDemand: "On-Demand",
    onDemandHint: "Trigger immediate on-demand OS process sample & write snapshot to temp/",
    resizeHint: "Drag to resize · double-click to expand",
    capturing: "Capturing...",
  },
  overview: {
    mainWebView: "Main WebView",
    rustCore: "Rust Core",
    trayWebView: "Tray WebView",
    trayHud: "Tray HUD",
    otherNetwork: "Other / Network",
    totalVoxRss: "Total Vox RSS",
    processTreeAggregate: "Process tree aggregate",
    primaryUiSurface: "Primary UI surface",
    hudTrayOverlay: "HUD tray overlay",
    jsHeap: "JS Heap",
    heapFallback: "V8/WebKit heap",
    memoryOverTime: "Memory Over Time",
    sampling: "Sampling telemetry...",
    totalProcessTreeRss: "Total Process Tree RSS",
    memoryDistribution: "Memory Distribution",
    mbTotal: "MB Total",
    processTreeTitle: "OS Process Tree Hierarchy",
    pid: "PID",
    processName: "Process Name",
    inferredRole: "Inferred Role",
    physicalRss: "Physical RSS",
    cpuLoad: "CPU Load",
  },
  pages: {
    domTitle: "DOM Elements",
    domHint: "Active DOM tree elements",
    typographyTitle: "Typography Faces",
    fontsHint: "Loaded @font-face sets",
    fontsValue: "Sora, DM Sans, JetBrains Mono",
    heapTitle: "JS Heap V8/WebKit",
    gpuTitle: "GPU Layers",
    matrixTitle: "Page Lifecycle Attribution Matrix",
    matrixHint: "Standard Page Experiment (Baseline → Peak → Retained)",
    routePage: "Route / Page",
    status: "Status",
    baseline: "Baseline",
    current: "Current",
    peak: "Peak (Δ)",
    retained: "Retained (Δ)",
    riskObservation: "Risk / Observation",
    statusActive: "Active",
    statusUnmounted: "Unmounted",
    statusUnvisited: "Unvisited",
    measuringOnExit: "Measuring on exit...",
    riskNormal: "Normal",
    riskCritical: "Critical Retention",
    riskSuspicious: "Suspicious",
  },
  insights: {
    sectionTitle: "Root Cause Analysis & Heuristics",
    diagnosticsHint: "Automated diagnostics for memory leaks, un-evicted textures, and GPU compositor strain",
    actionLabel: "Action:",
    tracesTitle: "Component Lifecycle Traces",
    colComponent: "Component",
    colMounts: "Mounts",
    colInstances: "Instances",
    colLastActive: "Last Active",
    streamTitle: "Memory Event Stream",
    liveTimeline: "Live Timeline",
  },
};
