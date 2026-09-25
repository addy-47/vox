import { invoke } from "@tauri-apps/api/core";

export interface RuntimeSnapshot {
  pipeline_state: string;
  current_turn_id: number;
  conversation_id: number;
  playback_active: boolean;
  system_cpu_usage: number;
  system_ram_mb: number;
  vox_cpu_usage: number;
  vox_ram_mb: number;
  total_ram_mb: number;
  cpu_cores: number;
  vad_energy: number;
  vad_probability: number;
  stt_latency_ms: number | null;
  ttft_ms: number | null;
  total_voice_latency_ms: number | null;
  persistence_queue_depth: number;
  dropped_persistence_events: number;
  playback_buffer_samples: number;
  playback_underruns: number;
  active_owner: string;
  active_threads: number;
  tts_rtf: number | null;
  playback_start_ms: number | null;
  persistence_writes_per_sec: number;
  is_db_healthy: boolean;
  is_llm_loaded: boolean;
  llm_provider_kind: string;
  is_tts_loaded: boolean;
  is_stt_loaded: boolean;
  is_vad_loaded: boolean;
  is_embedder_loaded: boolean;
  is_query_classifier_loaded: boolean;
  is_intra_edge_classifier_loaded: boolean;
  is_inter_edge_classifier_loaded: boolean;
  is_translit_loaded: boolean;
  cpu_governor: string;
  cpu_governor_optimal: boolean;
  timestamp_ms?: number;
}

/** RuntimeSnapshot with a local performance.now() timestamp for sparkline age calc. */
export type LocalSnapshot = RuntimeSnapshot & { localTime: number };

export function getRuntimeSnapshot(): Promise<RuntimeSnapshot | null> {
  return invoke("get_runtime_snapshot");
}

export type AccuracyLevel = "Measured" | "Estimated" | "Correlated" | "Unattributed";

export interface ProcessMemoryEntry {
  pid: number;
  parent_pid: number | null;
  name: string;
  memory_mb: number;
  private_ram_mb?: number;
  shared_ram_mb?: number;
  pss_mb?: number;
  is_devtools?: boolean;
  is_dev_tool?: boolean;
  cpu_usage: number;
  start_time: number;
  is_main_process: boolean;
  role: string;
}

export interface ProfilerSnapshot {
  total_vox_ram_mb: number;
  core_app_ram_mb: number;
  devtools_ram_mb: number | null;
  development_tools_ram_mb: number;
  scope_kind: string;
  runtime_pss_mb: number;
  dev_tool_pss_mb: number;
  total_pss_mb: number | null;
  cgroup_current_mb: number | null;
  cgroup_swap_mb: number | null;
  cgroup_anon_mb: number | null;
  cgroup_file_mb: number | null;
  cgroup_kernel_mb: number | null;
  cgroup_shmem_mb: number | null;
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
  core_app_ram_mb?: number;
  devtools_ram_mb?: number | null;
  development_tools_ram_mb?: number;
  scope_kind?: string;
  runtime_pss_mb?: number;
  dev_tool_pss_mb?: number;
  cgroup_current_mb?: number | null;
  cgroup_swap_mb?: number | null;
  cgroup_anon_mb?: number | null;
  cgroup_file_mb?: number | null;
  cgroup_kernel_mb?: number | null;
  cgroup_shmem_mb?: number | null;
  total_pss_mb?: number | null;
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
  css_indicators?: CSSIndicatorsSample;
  three_metrics?: Record<string, unknown>;
  js_heap?: JSHeapSample;
  process_tree?: ProcessMemoryEntry[];
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
 * Scans the active document for compositing-heavy CSS indicators without forcing layout recalculation.
 */
export function sampleCSSIndicators(): CSSIndicatorsSample {
  if (typeof document === "undefined") {
    return { backdropFilterCount: 0, willChangeCount: 0, canvasCount: 0, accuracy: "Estimated" };
  }

  const canvasCount = document.querySelectorAll("canvas").length;
  const backdropFilterCount = document.querySelectorAll(
    '[style*="backdrop-filter"], [class*="backdrop-blur"], .glass-card, .glass-panel'
  ).length;
  const willChangeCount = document.querySelectorAll(
    '[style*="will-change"], [class*="will-change"]'
  ).length;

  return {
    backdropFilterCount,
    willChangeCount,
    canvasCount,
    accuracy: "Estimated",
  };
}
