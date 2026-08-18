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
};
