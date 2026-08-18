import { invoke } from "@tauri-apps/api/core";

export type AccuracyLevel = "Measured" | "Estimated" | "Correlated" | "Unattributed";

export interface ProcessMemoryEntry {
  pid: number;
  parent_pid: number | null;
  name: string;
  memory_mb: number;
  cpu_usage: number;
  start_time: number;
  is_main_process: boolean;
  role: string;
}

export interface ProfilerSnapshot {
  total_vox_ram_mb: number;
  main_process_ram_mb: number;
  main_webview_ram_mb: number | null;
  tray_webview_ram_mb: number | null;
  wizard_webview_ram_mb: number | null;
  network_process_ram_mb: number | null;
  other_children_ram_mb: number;
  total_system_ram_mb: number;
  used_system_ram_mb: number;
  system_ram_pct: number;
  process_tree: ProcessMemoryEntry[];
  timestamp_ms: number;
  accuracy: string;
}

export interface JSHeapSample {
  usedMb: number | null;
  totalMb: number | null;
  limitMb: number | null;
  available: boolean;
  accuracy: AccuracyLevel;
}

export interface DOMSample {
  nodeCount: number;
  fontFaceCount: number;
  resourceCount: number;
  estimatedResourceBytesMb: number;
  accuracy: AccuracyLevel;
}

export interface CSSIndicatorsSample {
  backdropFilterCount: number;
  willChangeCount: number;
  canvasCount: number;
  accuracy: AccuracyLevel;
}

export interface MemoryProfileLogEvent {
  route: string;
  event_type: string;
  baseline_ram_mb: number | null;
  current_ram_mb: number;
  peak_ram_mb: number | null;
  peak_delta_mb: number | null;
  retained_ram_mb: number | null;
  retained_delta_mb: number | null;
  main_webview_ram_mb: number | null;
  tray_webview_ram_mb: number | null;
  active_components: string[];
  dom_node_count: number;
  font_face_count: number;
  timestamp_ms: number;
}

/**
 * Calls Tauri backend to fetch a fresh, high-precision process tree snapshot.
 */
export async function getProfilerSnapshot(): Promise<ProfilerSnapshot> {
  return await invoke<ProfilerSnapshot>("get_profiler_snapshot");
}

/**
 * Records a structured memory event to backend tracing and temp/memory_profile_session.jsonl.
 */
export async function recordMemoryProfileEvent(event: MemoryProfileLogEvent): Promise<void> {
  try {
    await invoke("record_memory_profile_event", { event });
  } catch (e) {
    // Non-blocking logging
  }
}

/**
 * Probes browser performance.memory (Chrome/Chromium or custom WebKit builds).
 */
export function sampleJSHeap(): JSHeapSample {
  if (typeof window === "undefined") {
    return { usedMb: null, totalMb: null, limitMb: null, available: false, accuracy: "Unattributed" };
  }

  const perf = window.performance as any;
  if (perf && perf.memory && typeof perf.memory.usedJSHeapSize === "number") {
    const usedMb = Math.round((perf.memory.usedJSHeapSize / 1024 / 1024) * 100) / 100;
    const totalMb = Math.round((perf.memory.totalJSHeapSize / 1024 / 1024) * 100) / 100;
    const limitMb = Math.round((perf.memory.jsHeapSizeLimit / 1024 / 1024) * 100) / 100;
    return { usedMb, totalMb, limitMb, available: true, accuracy: "Measured" };
  }

  return { usedMb: null, totalMb: null, limitMb: null, available: false, accuracy: "Unattributed" };
}

/**
 * Probes current DOM node count, font faces, and resource timing metrics.
 */
export function sampleDOMStats(): DOMSample {
  if (typeof document === "undefined") {
    return { nodeCount: 0, fontFaceCount: 0, resourceCount: 0, estimatedResourceBytesMb: 0, accuracy: "Unattributed" };
  }

  const nodeCount = document.querySelectorAll("*").length;
  let fontFaceCount = 0;
  try {
    fontFaceCount = document.fonts ? document.fonts.size : 0;
  } catch {
    fontFaceCount = 0;
  }

  let resourceCount = 0;
  let estimatedResourceBytes = 0;

  if (typeof performance !== "undefined" && typeof performance.getEntriesByType === "function") {
    const entries = performance.getEntriesByType("resource") as PerformanceResourceTiming[];
    resourceCount = entries.length;
    for (let i = 0; i < entries.length; i++) {
      const e = entries[i];
      if (e.decodedBodySize) {
        estimatedResourceBytes += e.decodedBodySize;
      } else if (e.transferSize) {
        estimatedResourceBytes += e.transferSize;
      }
    }
  }

  const estimatedResourceBytesMb = Math.round((estimatedResourceBytes / 1024 / 1024) * 100) / 100;

  return {
    nodeCount,
    fontFaceCount,
    resourceCount,
    estimatedResourceBytesMb,
    accuracy: "Measured",
  };
}

/**
 * Scans the active document for compositing-heavy CSS indicators.
 */
export function sampleCSSIndicators(): CSSIndicatorsSample {
  if (typeof document === "undefined") {
    return { backdropFilterCount: 0, willChangeCount: 0, canvasCount: 0, accuracy: "Correlated" };
  }

  let backdropFilterCount = 0;
  let willChangeCount = 0;
  const canvasCount = document.querySelectorAll("canvas").length;

  const elements = document.querySelectorAll<HTMLElement>("*");
  // Sample up to first 300 elements to avoid performance stutter
  const sampleLimit = Math.min(elements.length, 300);
  for (let i = 0; i < sampleLimit; i++) {
    const el = elements[i];
    const style = window.getComputedStyle(el);
    if (style.backdropFilter && style.backdropFilter !== "none") {
      backdropFilterCount++;
    }
    if (style.willChange && style.willChange !== "auto") {
      willChangeCount++;
    }
  }

  return {
    backdropFilterCount,
    willChangeCount,
    canvasCount,
    accuracy: "Correlated",
  };
}
