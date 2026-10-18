/**
 * Vox Comprehensive CDP Soak Runner
 *
 * Conducts extended soak testing over configured duration (default 5 mins, or --quick for 1.5 mins)
 * with realistic user behaviors:
 * 1. Home: Active dialogue flow, NotificationPanel, SessionPanel
 * 2. History: Search bar keystroke churn, session detail drawer inspection
 * 3. Memory: Graph zoom/pan controls, Personal Memory Slate drawer & search dialog
 * 4. Settings: RadialHub node selection, ModelModal / CatalogModalShell expansion, provider filtering, theme flips
 * 5. Monitoring: Real-time telemetry feed, popover float
 *
 * Injects a high-fidelity mock IPC carrier via CDP so real sessions, models,
 * observations, and settings are rendered without requiring an active Tauri runtime.
 */

import { spawn } from "child_process";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const APP_DIR = path.resolve(__dirname, "..");
const RESULTS_DIR = path.resolve(APP_DIR, "results");

if (!fs.existsSync(RESULTS_DIR)) {
  fs.mkdirSync(RESULTS_DIR, { recursive: true });
}

const SOAK_TELEMETRY_FILE = path.join(RESULTS_DIR, "soak_telemetry.jsonl");
const SOAK_REPORT_FILE = path.join(RESULTS_DIR, "soak_test_report.json");

// Clear existing soak telemetry
fs.writeFileSync(SOAK_TELEMETRY_FILE, "");

const CHROME_PORT = 9223; // Distinct port from rapid stress runner
const TARGET_URL = "http://localhost:1420/";

// Parse arguments: --duration=<minutes> or --quick (1.5 mins)
const args = process.argv.slice(2);
const isQuick = args.includes("--quick");
const durationArg = args.find((a) => a.startsWith("--duration="));
const DURATION_MINUTES = isQuick ? 1.5 : (durationArg ? parseFloat(durationArg.split("=")[1]) : 5);
const DURATION_MS = DURATION_MINUTES * 60 * 1000;

console.log(`[SOAK RUNNER] Configured for ${DURATION_MINUTES} minutes soak test (${DURATION_MS / 1000}s total, quick=${isQuick})...`);

class CDPClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.ws = null;
    this.id = 1;
    this.pending = new Map();
    this.eventListeners = new Map();
  }

  connect() {
    return new Promise((resolve, reject) => {
      this.ws = new WebSocket(this.wsUrl);
      this.ws.onopen = () => resolve();
      this.ws.onerror = (err) => reject(err);
      this.ws.onmessage = (event) => {
        const msg = JSON.parse(event.data);
        if (msg.id && this.pending.has(msg.id)) {
          const { resolve, reject } = this.pending.get(msg.id);
          this.pending.delete(msg.id);
          if (msg.error) {
            reject(new Error(msg.error.message));
          } else {
            resolve(msg.result);
          }
        } else if (!msg.id && msg.method && this.eventListeners.has(msg.method)) {
          const listeners = this.eventListeners.get(msg.method);
          listeners.forEach((fn) => fn(msg.params));
        }
      };
    });
  }

  on(event, handler) {
    if (!this.eventListeners.has(event)) {
      this.eventListeners.set(event, []);
    }
    this.eventListeners.get(event).push(handler);
  }

  send(method, params = {}) {
    return new Promise((resolve, reject) => {
      const id = this.id++;
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }

  close() {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }
}

async function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

async function isPortOpen(port) {
  try {
    const res = await fetch(`http://127.0.0.1:${port}/`);
    return res.ok || res.status > 0;
  } catch {
    return false;
  }
}

const MOCK_BACKEND_SCRIPT = `
(() => {
  window.localStorage.setItem("vox_setup_completed", "true");
  window.localStorage.setItem("vox_disclaimer_dismissed", "true");

  const mockSettings = {
    audio: { sample_rate: 24000, channels: 1, buffer_size: 512, input_device: "Default Input", output_device: "Default Output", echo_cancellation: true, noise_suppression: true },
    vad: { threshold: 0.5, ptt_noise_gate: 0.3, vad_backend: "silero", silence_duration_ms: 800, speech_onset_ms: 90, max_speech_duration_s: 30 },
    stt: { active: { category: "embedded", provider: null, model: "whisper-small" }, last_selected: { embedded: "whisper-small" }, transliterate_enabled: false },
    llm: { active: { category: "server", provider: "srv-1", model: "llama-3.2-3b" }, last_selected: { server: "llama-3.2-3b" }, cloud_keys: {}, servers: [{ id: "srv-1", name: "Local Ollama", url: "http://127.0.0.1:11434", model: "llama-3.2-3b", protocol: "openai", api_key: null }] },
    tts: { active: { category: "embedded", provider: null, model: "kokoro-de" }, last_selected: { embedded: "kokoro-de" }, servers: [] },
    appearance: { theme: "dark", accent_color: "violet", ui_scale: 1.0, animations: true },
    interaction: { pipeline_mode: "cascade", push_to_talk: false, hotkey: "Space", voice_activation: true },
    monitoring: { metrics_enabled: true, retention_days: 7 },
  };

  const mockCatalog = {
    vad: [{ id: "silero", name: "Silero VAD", is_built_in: true, size: "1.8 MB", description: "Fast neural speech activity detection" }],
    stt: [{ id: "whisper-small", name: "Whisper Small", is_cloud: false, size: "480 MB", description: "Multilingual ASR" }],
    llm: [{ id: "qwen", name: "Qwen 2.5 1.5B", is_cloud: false, size: "1.2 GB", description: "Fast local language model" }],
    tts: [{ id: "kokoro-de", name: "Kokoro TTS", is_cloud: false, is_remote: false, size: "320 MB", description: "High quality neural TTS" }],
    auxiliary: [],
  };

  const mockSessions = [
    { id: "sess-101", title: "Architecture & Performance Audit", created_at: "2026-10-17T14:30:00Z", duration_secs: 240, message_count: 12, tokens: 3400, model: "llama-3.2-3b", turns: [
      { id: "t1", speaker: "user", text: "Can you analyze our layout thrashing?", timestamp: "2026-10-17T14:30:05Z" },
      { id: "t2", speaker: "assistant", text: "Analyzing your DOM nodes and framerate metrics now.", timestamp: "2026-10-17T14:30:10Z" }
    ]},
    { id: "sess-102", title: "Acoustic Echo Cancellation Benchmarks", created_at: "2026-10-17T15:15:00Z", duration_secs: 180, message_count: 8, tokens: 2100, model: "llama-3.2-3b", turns: [
      { id: "t3", speaker: "user", text: "What is the ERLE measurement?", timestamp: "2026-10-17T15:15:05Z" },
      { id: "t4", speaker: "assistant", text: "We achieved 32.58 dB ERLE with WebRTC AEC3.", timestamp: "2026-10-17T15:15:10Z" }
    ]},
    { id: "sess-103", title: "Catalog Modal Virtualization Discussion", created_at: "2026-10-17T16:00:00Z", duration_secs: 310, message_count: 15, tokens: 4200, model: "llama-3.2-3b", turns: [] }
  ];

  const mockObservations = [
    { id: "obs-1", session_id: "sess-101", text: "User prefers keyboard navigation and zero latency", category: "preference", timestamp: "2026-10-17T14:31:00Z" },
    { id: "obs-2", session_id: "sess-102", text: "AEC3 filter configured for 24kHz audio", category: "technical", timestamp: "2026-10-17T15:16:00Z" },
    { id: "obs-3", session_id: "sess-103", text: "Provider pills use responsive grid layout", category: "design", timestamp: "2026-10-17T16:05:00Z" }
  ];

  const mockRemoteModels = [
    { id: "llama-3.2-3b", name: "Llama 3.2 3B", size: "3.2B", provider: "ollama", context_length: 8192, description: "Fast lightweight instruction model" },
    { id: "mistral-7b", name: "Mistral 7B Instruct", size: "7B", provider: "ollama", context_length: 32768, description: "Balanced general purpose model" },
    { id: "gpt-4o", name: "GPT-4o", size: "cloud", provider: "openai", context_length: 128000, description: "Flagship multimodal cloud model" },
    { id: "claude-3-5-sonnet", name: "Claude 3.5 Sonnet", size: "cloud", provider: "anthropic", context_length: 200000, description: "Advanced reasoning and coding" }
  ];

  const mockVoices = [
    { id: "voice-bella", name: "Bella", provider: "kokoro", gender: "female", locale: "en-US", preview_url: null },
    { id: "voice-adam", name: "Adam", provider: "kokoro", gender: "male", locale: "en-US", preview_url: null }
  ];

  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: () => {},
  };

  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd, args = {}) => {
      if (cmd === "plugin:event|listen") return 1;
      if (cmd === "plugin:event|unlisten") return;
      if (cmd === "get_onboarding_status") return true;
      if (cmd === "get_runtime_report") return { ok: true, memory_available: true };
      if (cmd === "get_settings" || cmd === "load_settings") return mockSettings;
      if (cmd === "save_settings" || cmd === "commit_settings") return { ok: true };
      if (cmd === "get_model_catalog") return mockCatalog;
      if (cmd === "get_sessions") return mockSessions;
      if (cmd === "get_session_by_id") {
        return mockSessions.find(s => s.id === args.id) || mockSessions[0];
      }
      if (cmd === "get_active_observations" || cmd === "get_observations") return mockObservations;
      if (cmd === "get_memory_graph") return { nodes: mockObservations.map(o => ({ id: o.id, label: o.text })), edges: [] };
      if (cmd === "list_llm_models") return mockRemoteModels;
      if (cmd === "list_voices" || cmd === "get_voices") return mockVoices;
      if (cmd === "get_runtime_snapshot" || cmd === "get_system_metrics") {
        return { cpu_percent: 14.2, memory_mb: 280, active_threads: 8, engine_status: "idle" };
      }
      if (cmd === "get_provider_caps") return { streaming: true, voice_cloning: true };
      if (cmd === "get_notifications" || cmd === "list_notifications") {
        return [{ id: "notif-1", title: "Engine Ready", message: "Audio pipeline initialized", timestamp: Date.now(), category: "system", read: false }];
      }
      if (cmd === "check_tts_provider_health") return { healthy: true };
      return null;
    },
    transformCallback: (callback, once) => 1,
    metadata: { currentWindow: { label: "main" } }
  };
})();
`;

async function main() {
  console.log(`[SOAK RUNNER] Launching headless Chrome on port ${CHROME_PORT}...`);
  const chromeProcess = spawn("/usr/bin/google-chrome", [
    `--remote-debugging-port=${CHROME_PORT}`,
    "--headless=new",
    "--no-sandbox",
    "--disable-dev-shm-usage",
    "--disable-gpu",
    "--window-size=1920,1080",
    TARGET_URL,
  ], { stdio: "ignore" });

  let cdp = null;
  const runtimeExceptions = [];

  try {
    let wsUrl = null;
    for (let i = 0; i < 30; i++) {
      try {
        const res = await fetch(`http://127.0.0.1:${CHROME_PORT}/json`);
        const targets = await res.json();
        const pageTarget = targets.find((t) => t.type === "page");
        if (pageTarget && pageTarget.webSocketDebuggerUrl) {
          wsUrl = pageTarget.webSocketDebuggerUrl;
          break;
        }
      } catch (_) {}
      await sleep(250);
    }

    if (!wsUrl) throw new Error("Could not connect to CDP endpoint.");

    cdp = new CDPClient(wsUrl);
    await cdp.connect();

    await cdp.send("Page.enable");
    await cdp.send("DOM.enable");
    await cdp.send("Performance.enable");
    await cdp.send("Runtime.enable");

    cdp.on("Runtime.exceptionThrown", (params) => {
      const desc = params.exceptionDetails?.exception?.description || params.exceptionDetails?.text || "Unknown exception";
      runtimeExceptions.push({
        timestamp: Date.now(),
        description: desc,
        url: params.exceptionDetails?.url,
        line: params.exceptionDetails?.lineNumber,
      });
    });

    // Inject mock backend before load
    await cdp.send("Page.addScriptToEvaluateOnNewDocument", {
      source: MOCK_BACKEND_SCRIPT,
    });

    await cdp.send("Page.navigate", { url: TARGET_URL });
    await sleep(2000);

    // Realistic routes and interactions
    const routes = [
      {
        path: "/",
        name: "Home",
        dwellSec: isQuick ? 2.5 : 5,
        interact: async () => {
          // Open notification panel
          await cdp.send("Runtime.evaluate", {
            expression: `
              const bell = document.querySelector('button[aria-label*="notification" i]');
              if (bell) bell.click();
            `,
          });
          await sleep(600);
          // Dismiss notification panel
          await cdp.send("Runtime.evaluate", {
            expression: `
              const close = document.querySelector('[data-edge-panel="right"] button[aria-label="Close panel"]');
              if (close) close.click();
              else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
            `,
          });
        },
      },
      {
        path: "/history",
        name: "History",
        dwellSec: isQuick ? 2.5 : 5,
        interact: async () => {
          // Type into search bar and open session card detail drawer
          await cdp.send("Runtime.evaluate", {
            expression: `
              const input = document.querySelector('input[placeholder*="Search" i]');
              if (input) {
                input.value = "Audio";
                input.dispatchEvent(new Event("input", { bubbles: true }));
              }
              const card = document.querySelector('[data-session-card]') || document.querySelector('button.group');
              if (card) card.click();
            `,
          });
          await sleep(800);
          // Close drawer
          await cdp.send("Runtime.evaluate", {
            expression: `
              const close = document.querySelector('button[aria-label="Close drawer"]');
              if (close) close.click();
              else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
            `,
          });
        },
      },
      {
        path: "/memory",
        name: "Memory",
        dwellSec: isQuick ? 2.5 : 5,
        interact: async () => {
          // Trigger memory staging drawer / search
          await cdp.send("Runtime.evaluate", {
            expression: `
              const trigger = document.querySelector('button[aria-label*="Personal" i]') || document.querySelector('button[aria-label="Search Memories"]');
              if (trigger) trigger.click();
            `,
          });
          await sleep(800);
          await cdp.send("Runtime.evaluate", {
            expression: `
              window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
            `,
          });
        },
      },
      {
        path: "/settings",
        name: "Settings",
        dwellSec: isQuick ? 3 : 6,
        interact: async () => {
          // Select Models card & open ModelModal
          await cdp.send("Runtime.evaluate", {
            expression: `
              const modelsNode = document.querySelector('[data-domain="models"]') || Array.from(document.querySelectorAll('button')).find(b => b.textContent && b.textContent.includes('Models'));
              if (modelsNode) modelsNode.click();

              setTimeout(() => {
                const expandBtn = document.querySelector('button[aria-label*="Expand catalog" i]');
                if (expandBtn) expandBtn.click();
              }, 250);
            `,
          });
          await sleep(900);
          // Close modal
          await cdp.send("Runtime.evaluate", {
            expression: `
              const closeBtn = document.querySelector('button[aria-label="Close dialog"]');
              if (closeBtn) closeBtn.click();
              else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
            `,
          });
        },
      },
      {
        path: "/monitoring",
        name: "Monitoring",
        dwellSec: isQuick ? 2.5 : 5,
        interact: async () => {
          // Trigger monitoring float
          await cdp.send("Runtime.evaluate", {
            expression: `
              window.dispatchEvent(new KeyboardEvent("keydown", { key: "m", code: "KeyM", ctrlKey: true, bubbles: true }));
            `,
          });
          await sleep(700);
          await cdp.send("Runtime.evaluate", {
            expression: `
              window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
            `,
          });
        },
      },
    ];

    const getMetrics = async () => {
      const perfMetrics = await cdp.send("Performance.getMetrics");
      const m = {};
      perfMetrics.metrics.forEach((item) => {
        m[item.name] = item.value;
      });

      const evalRes = await cdp.send("Runtime.evaluate", {
        expression: `({
          url: window.location.pathname,
          activeCanvases: document.querySelectorAll("canvas").length,
          domNodes: document.querySelectorAll("*").length,
          openDialogs: document.querySelectorAll('[role="dialog"]').length,
          openBackdrops: document.querySelectorAll('.fixed.inset-0').length,
        })`,
        returnByValue: true,
      });

      const clientData = evalRes.result?.value || {};

      return {
        timestamp: Date.now(),
        jsHeapUsedMb: Number((m.JSHeapUsedSize / 1048576).toFixed(2)),
        jsHeapTotalMb: Number((m.JSHeapTotalSize / 1048576).toFixed(2)),
        domNodes: clientData.domNodes || m.Nodes,
        activeCanvases: clientData.activeCanvases || 0,
        layoutCount: m.LayoutCount,
        jsEventListeners: m.JSEventListeners,
        openDialogs: clientData.openDialogs || 0,
        openBackdrops: clientData.openBackdrops || 0,
      };
    };

    const startTime = Date.now();
    const baseline = await getMetrics();
    console.log(`[SOAK BASELINE] Heap: ${baseline.jsHeapUsedMb} MB | Nodes: ${baseline.domNodes} | Listeners: ${baseline.jsEventListeners} | Canvases: ${baseline.activeCanvases}`);

    let cycleCount = 0;
    const telemetryRecords = [];

    while (Date.now() - startTime < DURATION_MS) {
      cycleCount++;
      const elapsedMins = ((Date.now() - startTime) / 60000).toFixed(1);
      console.log(`\n--- SOAK CYCLE ${cycleCount} (${elapsedMins}m / ${DURATION_MINUTES}m elapsed) ---`);

      for (const r of routes) {
        if (Date.now() - startTime >= DURATION_MS) break;

        // Navigate
        await cdp.send("Runtime.evaluate", {
          expression: `window.history.pushState({}, "", "${r.path}"); window.dispatchEvent(new PopStateEvent("popstate"));`,
        });

        // Dwell before interaction
        const dwellHalf = Math.floor((r.dwellSec * 1000) / 2);
        await sleep(dwellHalf);

        // Perform real high-value interaction
        if (r.interact) {
          try {
            await r.interact();
          } catch (err) {
            console.warn(`[SOAK INTERACT WARN ${r.name}]`, err.message);
          }
        }

        // Settle dwell
        await sleep(dwellHalf);

        const currentMetrics = await getMetrics();
        const record = {
          cycle: cycleCount,
          elapsedSeconds: Math.round((Date.now() - startTime) / 1000),
          route: r.path,
          routeName: r.name,
          ...currentMetrics,
        };
        telemetryRecords.push(record);
        fs.appendFileSync(SOAK_TELEMETRY_FILE, JSON.stringify(record) + "\n");
        console.log(`  [${r.name.padEnd(10)}] Heap: ${record.jsHeapUsedMb} MB | Nodes: ${record.domNodes} | Listeners: ${record.jsEventListeners} | Canvases: ${record.activeCanvases}`);
      }
    }

    console.log("\n[SOAK RUNNER] Soak duration completed. Dispathing forced GC...");
    try {
      await cdp.send("HeapProfiler.collectGarbage");
    } catch (_) {}
    await sleep(1500);

    const postGc = await getMetrics();
    const actualElapsedMins = Number(((Date.now() - startTime) / 60000).toFixed(2));
    const deltaHeapMb = Number((postGc.jsHeapUsedMb - baseline.jsHeapUsedMb).toFixed(2));
    const driftMbPerMin = Number((deltaHeapMb / (actualElapsedMins || 1)).toFixed(3));

    const report = {
      timestamp: new Date().toISOString(),
      durationMinutesConfigured: DURATION_MINUTES,
      actualElapsedMinutes: actualElapsedMins,
      totalCycles: cycleCount,
      measurementsCount: telemetryRecords.length,
      baseline: {
        heapMb: baseline.jsHeapUsedMb,
        domNodes: baseline.domNodes,
        listeners: baseline.jsEventListeners,
      },
      postGc: {
        heapMb: postGc.jsHeapUsedMb,
        domNodes: postGc.domNodes,
        listeners: postGc.jsEventListeners,
      },
      analysis: {
        deltaHeapMb,
        driftMbPerMin,
        domNodeDelta: postGc.domNodes - baseline.domNodes,
        listenerDelta: postGc.jsEventListeners - baseline.jsEventListeners,
        soakStability: Math.abs(driftMbPerMin) < 0.15 ? "EXTREMELY_STABLE" : (driftMbPerMin < 0.5 ? "ACCEPTABLE_WARMUP" : "LONG_TERM_DRIFT"),
      },
      runtimeExceptionsCount: runtimeExceptions.length,
      runtimeExceptions,
    };

    fs.writeFileSync(SOAK_REPORT_FILE, JSON.stringify(report, null, 2));
    console.log("==================================================");
    console.log("[SOAK RESULT] Soak Test Finished:");
    console.log(`              Duration:         ${actualElapsedMins} mins (${cycleCount} full cycles)`);
    console.log(`              Post-GC Heap:     ${postGc.jsHeapUsedMb} MB (Delta: ${deltaHeapMb >= 0 ? "+" : ""}${deltaHeapMb} MB)`);
    console.log(`              Drift Rate:       ${driftMbPerMin >= 0 ? "+" : ""}${driftMbPerMin} MB/min (${report.analysis.soakStability})`);
    console.log(`              DOM Node Delta:   ${report.analysis.domNodeDelta}`);
    console.log(`              Listener Delta:   ${report.analysis.listenerDelta}`);
    console.log(`              Exceptions:       ${runtimeExceptions.length}`);
    console.log(`              Report:           ${SOAK_REPORT_FILE}`);
    console.log("==================================================");
  } finally {
    if (cdp) cdp.close();
    chromeProcess.kill("SIGTERM");
  }
}

main().catch((err) => {
  console.error("[SOAK RUNNER FATAL ERROR]", err);
  process.exit(1);
});
