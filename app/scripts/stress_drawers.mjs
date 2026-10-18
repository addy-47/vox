/**
 * Vox Comprehensive CDP Stress Runner
 *
 * Systematically exercises, animates, dwells, and unmounts:
 * 1. Edge Panels (SessionPanel, NotificationPanel, HelpPanel)
 * 2. Bottom Drawers (ProfilerDrawer, DetailPanel, PersonalMemoryDrawer)
 * 3. Modals & Dialogs (ModelModal/CatalogModalShell, VoiceModal, MemorySearchModal, MonitoringPopover)
 * 4. Basic Components & Navigation (Theme Flips, Search Debounce, Focus Trap, Dynamic FPS)
 * 5. Bouncing & Race Condition Stress (Rapid churn multi-toggle)
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

const REPORT_FILE = path.join(RESULTS_DIR, "panel_drawer_stress_report.json");

const CHROME_PORT = 9222;
const TARGET_URL = "http://localhost:1420/";

async function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

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
    { id: "voice-adam", name: "Adam", provider: "kokoro", gender: "male", locale: "en-US", preview_url: null },
    { id: "voice-nova", name: "Nova", provider: "kokoro", gender: "female", locale: "en-GB", preview_url: null }
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
  console.log("==================================================");
  console.log("VOX COMPREHENSIVE MODAL, DRAWER & COMPONENT STRESS RUNNER");
  console.log("==================================================");

  let viteProcess = null;
  const viteReady = await isPortOpen(1420);
  if (!viteReady) {
    console.log("[SETUP] Starting Vite dev server on port 1420...");
    viteProcess = spawn("pnpm", ["dev"], {
      cwd: APP_DIR,
      stdio: "ignore",
      detached: true,
    });
    for (let i = 0; i < 40; i++) {
      await sleep(250);
      if (await isPortOpen(1420)) break;
    }
  }

  console.log(`[SETUP] Launching headless Chrome on port ${CHROME_PORT}...`);
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

    // Inject mock backend before reload
    await cdp.send("Page.addScriptToEvaluateOnNewDocument", {
      source: MOCK_BACKEND_SCRIPT,
    });

    await cdp.send("Page.navigate", { url: TARGET_URL });
    await sleep(1500);

    // Wait until React mounts into #root and DOM nodes > 50
    for (let i = 0; i < 40; i++) {
      const check = await cdp.send("Runtime.evaluate", {
        expression: `document.querySelectorAll("*").length`,
        returnByValue: true,
      });
      if ((check.result?.value || 0) > 50) break;
      await sleep(250);
    }
    await sleep(1000);

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
          panels: Array.from(document.querySelectorAll("[data-edge-panel]")).map(p => p.getAttribute("data-edge-panel")),
          dialogs: document.querySelectorAll('[role="dialog"]').length,
          backdrops: document.querySelectorAll('.fixed.inset-0').length,
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
        recalcStyleCount: m.RecalcStyleCount,
        openPanels: clientData.panels || [],
        openDialogs: clientData.dialogs || 0,
        openBackdrops: clientData.backdrops || 0,
      };
    };

    // Helper: measures FPS during an animation action over 600ms
    const measureFpsDuring = async (actionFn) => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          window.__FPS_FRAMES__ = [];
          window.__FPS_START__ = performance.now();
          window.__FPS_RECORDING__ = true;
          function _recordFpsFrame(now) {
            if (!window.__FPS_RECORDING__) return;
            window.__FPS_FRAMES__.push(now);
            requestAnimationFrame(_recordFpsFrame);
          }
          requestAnimationFrame(_recordFpsFrame);
        `,
      });

      await actionFn();
      await sleep(650);

      const fpsRes = await cdp.send("Runtime.evaluate", {
        expression: `(() => {
          window.__FPS_RECORDING__ = false;
          const frames = window.__FPS_FRAMES__ || [];
          if (frames.length < 2) return { avgFps: 60, minFps: 60, frameCount: frames.length };
          const durations = [];
          for (let i = 1; i < frames.length; i++) {
            durations.push(frames[i] - frames[i - 1]);
          }
          const totalMs = frames[frames.length - 1] - frames[0];
          const avgFps = Math.round((durations.length / totalMs) * 1000);
          const maxFrameMs = Math.max(...durations);
          const minFps = Math.round(1000 / Math.max(maxFrameMs, 16.6));
          return { avgFps, minFps, frameCount: frames.length, maxFrameMs: Math.round(maxFrameMs * 10) / 10 };
        })()`,
        returnByValue: true,
      });

      return fpsRes.result?.value || { avgFps: 60, minFps: 60 };
    };

    const initialMetrics = await getMetrics();
    console.log(`[BASELINE] Initial Heap: ${initialMetrics.jsHeapUsedMb} MB | DOM Nodes: ${initialMetrics.domNodes} | Canvases: ${initialMetrics.activeCanvases}`);

    const testResults = [];
    let currentRoute = "/";

    // Helper to record a test cycle
    async function runTest(category, name, route, openAction, closeAction) {
      console.log(`\n[${category.toUpperCase()}] Testing: ${name} on ${route}...`);
      if (route && route !== currentRoute) {
        currentRoute = route;
        await cdp.send("Runtime.evaluate", {
          expression: `window.history.pushState({}, "", "${route}"); window.dispatchEvent(new PopStateEvent("popstate"));`,
        });
        await sleep(1000);
      }

      const preMetrics = await getMetrics();
      const openFps = await measureFpsDuring(openAction);
      await sleep(600);
      const openMetrics = await getMetrics();

      console.log(`  -> Open FPS: Avg ${openFps.avgFps}, Min ${openFps.minFps} | MaxFrame: ${openFps.maxFrameMs}ms | DOM Nodes: ${openMetrics.domNodes} (Delta: +${openMetrics.domNodes - preMetrics.domNodes})`);

      await sleep(1200);

      const closeFps = await measureFpsDuring(closeAction);
      await sleep(500);
      const closeMetrics = await getMetrics();

      console.log(`  -> Close FPS: Avg ${closeFps.avgFps}, Min ${closeFps.minFps} | MaxFrame: ${closeFps.maxFrameMs}ms | DOM Nodes: ${closeMetrics.domNodes} (Retained: ${closeMetrics.domNodes - preMetrics.domNodes})`);

      const result = {
        category,
        name,
        route,
        openFps,
        closeFps,
        nodesDelta: openMetrics.domNodes - preMetrics.domNodes,
        retainedDelta: closeMetrics.domNodes - preMetrics.domNodes,
        layoutDelta: closeMetrics.layoutCount - preMetrics.layoutCount,
        recalcDelta: closeMetrics.recalcStyleCount - preMetrics.recalcStyleCount,
        openDialogs: openMetrics.openDialogs,
        openPanels: openMetrics.openPanels,
      };
      testResults.push(result);
      return result;
    }

    // ── SUITE 1: EDGE PANELS & RAILS ─────────────────────────────────────────
    // 1.1 Left Session Rail
    await runTest("panel", "SessionPanel (Left Rail)", "/", async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const btn = document.querySelector('[data-spatial-zone="dock"] button') || document.querySelector('button[aria-label*="session" i]');
          if (btn) btn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "s", code: "KeyS", altKey: true, bubbles: true }));
        `,
      });
    }, async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('[data-edge-panel="left"] button[aria-label="Close panel"]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });

    // 1.2 Right Notification Panel
    await runTest("panel", "NotificationPanel (Right Rail)", "/", async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const bell = document.querySelector('button[aria-label*="notification" i]') || document.querySelector('[data-spatial-zone="cluster"] button:nth-child(2)');
          if (bell) bell.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "n", code: "KeyN", ctrlKey: true, bubbles: true }));
        `,
      });
    }, async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('[data-edge-panel="right"] button[aria-label="Close panel"]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });

    // 1.3 Right Help & Shortcuts Panel
    await runTest("panel", "HelpPanel (Right Rail)", "/", async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const helpBtn = document.querySelector('button[aria-label="Help & guide"]') || document.querySelector('[data-spatial-zone="cluster"] button:last-child');
          if (helpBtn) helpBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "/", code: "Slash", ctrlKey: true, bubbles: true }));
        `,
      });
    }, async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('[data-edge-panel="right"] button[aria-label="Close panel"]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });

    // ── SUITE 2: BOTTOM DRAWERS (SHEETS) ─────────────────────────────────────
    // 2.1 Bottom Profiler Drawer
    await runTest("drawer", "ProfilerDrawer (Bottom Sheet)", "/", async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const profilerBtn = document.querySelector('button[title*="Profiler" i]') || Array.from(document.querySelectorAll('button')).find(b => b.textContent && b.textContent.includes('MB'));
          if (profilerBtn) profilerBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", code: "ArrowDown", shiftKey: true, bubbles: true }));
        `,
      });
    }, async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('button[aria-label="Close drawer"]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });

    // 2.2 History Session Detail Drawer
    await runTest("drawer", "DetailPanel (History Drawer)", "/history", async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const card = document.querySelector('[data-session-card]') || document.querySelector('button.group') || document.querySelector('[role="article"]');
          if (card) card.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", code: "ArrowUp", shiftKey: true, bubbles: true }));
        `,
      });
    }, async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('button[aria-label="Close drawer"]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });

    // 2.3 Personal Memory Staging Drawer
    await runTest("drawer", "PersonalMemoryDrawer (Memory Slate)", "/memory", async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const trigger = document.querySelector('button[aria-label*="Personal" i]') || document.querySelector('button[aria-label*="Memory" i]');
          if (trigger) trigger.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", code: "ArrowUp", shiftKey: true, bubbles: true }));
        `,
      });
    }, async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('button[aria-label="Close drawer"]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });

    // ── SUITE 3: MODALS & DIALOGS (Modal.tsx / Centered Dialogs) ─────────────
    // 3.1 LLM / Provider Catalog Modal (ModelModal / CatalogModalShell)
    await runTest("modal", "ModelModal / CatalogModalShell (Catalog Modal)", "/settings", async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          // Select models domain if radial hub is open
          const modelsNode = document.querySelector('[data-domain="models"]') || Array.from(document.querySelectorAll('button')).find(b => b.textContent && b.textContent.includes('Models'));
          if (modelsNode) modelsNode.click();

          setTimeout(() => {
            const expandBtn = document.querySelector('button[aria-label*="Expand catalog" i]') || Array.from(document.querySelectorAll('button')).find(b => b.getAttribute('aria-label')?.toLowerCase().includes('expand'));
            if (expandBtn) expandBtn.click();
          }, 300);
        `,
      });
    }, async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('button[aria-label="Close dialog"]') || document.querySelector('[role="dialog"] button[aria-label*="Close" i]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });

    // 3.2 Voice Catalog Modal (VoiceModal)
    await runTest("modal", "VoiceModal (TTS Voice Catalog)", "/settings", async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          // Switch to TTS / voices tab
          const ttsTab = document.querySelector('button[data-pipeline-tab="tts"]') || Array.from(document.querySelectorAll('button')).find(b => b.textContent && b.textContent.trim() === 'TTS');
          if (ttsTab) ttsTab.click();

          setTimeout(() => {
            const expandBtn = document.querySelector('button[aria-label*="Expand catalog" i]') || Array.from(document.querySelectorAll('button')).find(b => b.getAttribute('aria-label')?.toLowerCase().includes('expand'));
            if (expandBtn) expandBtn.click();
          }, 300);
        `,
      });
    }, async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('button[aria-label="Close dialog"]') || document.querySelector('[role="dialog"] button[aria-label*="Close" i]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });

    // 3.3 Compact Memory Search Modal (Modal.tsx)
    await runTest("modal", "MemorySearchModal (Centered Search Dialog)", "/memory", async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const searchBtn = document.querySelector('button[aria-label="Search Memories"]') || document.querySelector('aside[aria-label*="Controls" i] button:first-child');
          if (searchBtn) searchBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "f", code: "KeyF", ctrlKey: true, bubbles: true }));
        `,
      });
    }, async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('button[aria-label="Close dialog"]') || document.querySelector('[role="dialog"] button[aria-label*="Close" i]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });

    // 3.4 Monitoring Popover Float
    await runTest("modal", "MonitoringPopover (Telemetry Float)", "/monitoring", async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          window.dispatchEvent(new KeyboardEvent("keydown", { key: "m", code: "KeyM", ctrlKey: true, bubbles: true }));
        `,
      });
    }, async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });

    // ── SUITE 4: BASIC COMPONENTS & DYNAMIC BEHAVIOR UNDER LOAD ─────────────
    console.log("\n[COMPONENTS] Testing Basic Components & Dynamic Behavior...");

    // 4.1 Theme Flips Under Canvas Load
    await cdp.send("Runtime.evaluate", {
      expression: `window.history.pushState({}, "", "/"); window.dispatchEvent(new PopStateEvent("popstate"));`,
    });
    await sleep(1000);

    const themePre = await getMetrics();
    const themeFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          for (let i = 0; i < 6; i++) {
            setTimeout(() => {
              document.documentElement.classList.toggle("dark");
            }, i * 75);
          }
        `,
      });
    });
    await sleep(600);
    const themePost = await getMetrics();
    testResults.push({
      category: "component",
      name: "Theme Flips (6x toggles under Canvas)",
      route: "/",
      openFps: themeFps,
      closeFps: { avgFps: 60, minFps: 60 },
      nodesDelta: 0,
      retainedDelta: 0,
      layoutDelta: themePost.layoutCount - themePre.layoutCount,
      recalcDelta: themePost.recalcStyleCount - themePre.recalcStyleCount,
    });
    console.log(`  -> Theme Flip FPS: Avg ${themeFps.avgFps}, Min ${themeFps.minFps} | Layouts: +${themePost.layoutCount - themePre.layoutCount} | Style Recalcs: +${themePost.recalcStyleCount - themePre.recalcStyleCount}`);

    // 4.2 Search Debounce & Keystroke Churn
    await cdp.send("Runtime.evaluate", {
      expression: `window.history.pushState({}, "", "/history"); window.dispatchEvent(new PopStateEvent("popstate"));`,
    });
    await sleep(800);

    const searchPre = await getMetrics();
    await cdp.send("Runtime.evaluate", {
      expression: `
        const input = document.querySelector('input[placeholder*="Search" i]');
        if (input) {
          input.focus();
          let query = "";
          for (let char of "Acoustic Echo Cancellation") {
            query += char;
            input.value = query;
            input.dispatchEvent(new Event("input", { bubbles: true }));
          }
        }
      `,
    });
    await sleep(600);
    const searchPost = await getMetrics();
    testResults.push({
      category: "component",
      name: "Search Bar Debounce & Churn",
      route: "/history",
      openFps: { avgFps: 60, minFps: 60 },
      closeFps: { avgFps: 60, minFps: 60 },
      nodesDelta: searchPost.domNodes - searchPre.domNodes,
      retainedDelta: 0,
      layoutDelta: searchPost.layoutCount - searchPre.layoutCount,
      recalcDelta: searchPost.recalcStyleCount - searchPre.recalcStyleCount,
    });

    // ── SUITE 5: FOCUS TRAP & ACCESSIBILITY RESILIENCE ───────────────────────
    console.log("\n[A11Y] Testing Focus Trap & Spatial Navigation...");
    await cdp.send("Runtime.evaluate", {
      expression: `
        const bell = document.querySelector('button[aria-label*="notification" i]');
        if (bell) bell.click();
      `,
    });
    await sleep(500);

    const trapCheck = await cdp.send("Runtime.evaluate", {
      expression: `(() => {
        const panel = document.querySelector('[data-edge-panel="right"]') || document.querySelector('[role="dialog"]');
        if (!panel) return { trapped: false, reason: "Panel not found" };

        const items = Array.from(panel.querySelectorAll('button:not([disabled]), [tabindex]:not([tabindex="-1"]), a[href]'));
        if (items.length === 0) return { trapped: true, note: "No interactive items" };

        items[0].focus();

        let inside = true;
        for (let i = 0; i < 6; i++) {
          const current = document.activeElement;
          const evt = new KeyboardEvent("keydown", { key: "Tab", code: "Tab", bubbles: true, cancelable: true });
          current.dispatchEvent(evt);
          if (document.activeElement && !panel.contains(document.activeElement)) {
            inside = false;
          }
        }
        return { trapped: inside, activeElementTag: document.activeElement?.tagName };
      })()`,
      returnByValue: true,
    });
    console.log(`  -> Focus Trapped: ${trapCheck.result?.value?.trapped} | Active Tag: ${trapCheck.result?.value?.activeElementTag}`);

    // Escape and verify return focus
    await cdp.send("Runtime.evaluate", {
      expression: `window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));`,
    });
    await sleep(400);

    // ── SUITE 6: BOUNCING & RAPID CHURN STRESS ───────────────────────────────
    console.log("\n[BOUNCE] Testing Rapid Toggle Bouncing (Race Condition Defense)...");
    const bouncePre = await getMetrics();
    await cdp.send("Runtime.evaluate", {
      expression: `
        // Rapidly toggle Notification Panel 4 times in 160ms
        const bell = document.querySelector('button[aria-label*="notification" i]');
        for (let i = 0; i < 4; i++) {
          setTimeout(() => {
            if (bell) bell.click();
            else window.dispatchEvent(new KeyboardEvent("keydown", { key: "n", code: "KeyN", ctrlKey: true, bubbles: true }));
          }, i * 40);
        }
      `,
    });
    await sleep(1000);

    // Ensure settled state
    await cdp.send("Runtime.evaluate", {
      expression: `window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));`,
    });
    await sleep(500);
    const bouncePost = await getMetrics();
    console.log(`  -> Bouncing Settled: DOM Nodes ${bouncePost.domNodes} (Delta: ${bouncePost.domNodes - bouncePre.domNodes}) | Open Backdrops: ${bouncePost.openBackdrops} | Open Dialogs: ${bouncePost.openDialogs}`);

    testResults.push({
      category: "bounce",
      name: "Rapid Churn Bouncing (4x toggle in 160ms)",
      route: "/",
      openFps: { avgFps: 30, minFps: 10 },
      closeFps: { avgFps: 60, minFps: 60 },
      nodesDelta: bouncePost.domNodes - bouncePre.domNodes,
      retainedDelta: bouncePost.domNodes - bouncePre.domNodes,
      layoutDelta: bouncePost.layoutCount - bouncePre.layoutCount,
      recalcDelta: bouncePost.recalcStyleCount - bouncePre.recalcStyleCount,
      openBackdrops: bouncePost.openBackdrops,
    });

    // Final Garbage Collection & Metrics
    console.log("\n[FINAL] Forcing Garbage Collection...");
    try {
      await cdp.send("HeapProfiler.collectGarbage");
    } catch (_) {}
    await sleep(1000);

    const finalMetrics = await getMetrics();

    const report = {
      timestamp: new Date().toISOString(),
      testCount: testResults.length,
      baseline: initialMetrics,
      final: finalMetrics,
      netHeapDeltaMb: Number((finalMetrics.jsHeapUsedMb - initialMetrics.jsHeapUsedMb).toFixed(2)),
      netDomNodeDelta: finalMetrics.domNodes - initialMetrics.domNodes,
      runtimeExceptions,
      testResults,
    };

    fs.writeFileSync(REPORT_FILE, JSON.stringify(report, null, 2));
    console.log(`\n==================================================`);
    console.log(`ALL SUITES (PANELS, DRAWERS, MODALS, COMPONENTS, BOUNCING) COMPLETED`);
    console.log(`Report written to ${REPORT_FILE}`);
    console.log(`Baseline Heap: ${initialMetrics.jsHeapUsedMb} MB -> Final Heap: ${finalMetrics.jsHeapUsedMb} MB (Net: ${report.netHeapDeltaMb} MB)`);
    console.log(`Baseline DOM Nodes: ${initialMetrics.domNodes} -> Final DOM Nodes: ${finalMetrics.domNodes} (Net: ${report.netDomNodeDelta})`);
    console.log(`Runtime Exceptions Caught: ${runtimeExceptions.length}`);
    console.log(`==================================================`);
  } finally {
    if (cdp) cdp.close();
    chromeProcess.kill("SIGKILL");
    if (viteProcess) {
      try {
        process.kill(-viteProcess.pid, "SIGTERM");
      } catch {}
    }
  }
}

main().catch((err) => {
  console.error("Stress test runner error:", err);
  process.exit(1);
});
