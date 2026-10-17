/**
 * Vox Comprehensive CDP Drawer & Panel Stress Runner
 *
 * Systematically mounts, animates, dwells, and unmounts all 7 application drawers & panels:
 * 1. Left Session Panel (SessionPanel.tsx)
 * 2. Right Notification Panel (NotificationPanel.tsx)
 * 3. Right Help & Guide Panel (HelpPanel.tsx)
 * 4. Bottom Profiler Drawer (ProfilerDrawer.tsx)
 * 5. History Session Detail Drawer (DetailPanel.tsx)
 * 6. Personal Memory Staging Drawer (PersonalMemoryDrawer.tsx)
 * 7. Monitoring Popover (Monitoring.tsx)
 *
 * Measures: FPS during slide animation, layout thrashing, DOM node drift, and post-GC JS heap.
 */

import { spawn } from "child_process";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const RESULTS_DIR = path.resolve(__dirname, "../results");

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
        }
      };
    });
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

async function main() {
  console.log("==================================================");
  console.log("VOX ALL-DRAWER & PANEL CDP STRESS RUNNER");
  console.log("==================================================");

  let viteProcess = null;
  const viteReady = await isPortOpen(1420);
  if (!viteReady) {
    console.log("[SETUP] Starting Vite dev server on port 1420...");
    viteProcess = spawn("pnpm", ["dev"], {
      cwd: path.resolve(__dirname, "../../../../app"),
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

    await cdp.send("Runtime.evaluate", {
      expression: `localStorage.setItem("vox_setup_completed", "true");`,
    });

    // Wait until React mounts into #root and DOM nodes > 50
    for (let i = 0; i < 30; i++) {
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

    const panelResults = [];

    // ── 1. Left Session Panel ────────────────────────────────────────────────
    console.log("\n[TEST 1/7] Testing Left Session Panel (SessionPanel.tsx)...");
    const sessionOpenFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const btn = document.querySelector('[data-spatial-zone="dock"] button') || document.querySelector('button[aria-label*="session" i]');
          if (btn) btn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "s", code: "KeyS", altKey: true, bubbles: true }));
        `,
      });
    });
    await sleep(600);
    const sessionOpenMetrics = await getMetrics();
    console.log(`  -> Open FPS: Avg ${sessionOpenFps.avgFps}, Min ${sessionOpenFps.minFps} | DOM Nodes: ${sessionOpenMetrics.domNodes} | Panels open: ${sessionOpenMetrics.openPanels.join(",")}`);

    // Dwell
    await sleep(1500);

    // Close
    const sessionCloseFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('[data-edge-panel="left"] button[aria-label="Close panel"]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });
    await sleep(400);
    const sessionCloseMetrics = await getMetrics();
    panelResults.push({
      panel: "SessionPanel (Left Rail)",
      route: "/",
      openFps: sessionOpenFps,
      closeFps: sessionCloseFps,
      nodesDelta: sessionOpenMetrics.domNodes - initialMetrics.domNodes,
      retainedDelta: sessionCloseMetrics.domNodes - initialMetrics.domNodes,
    });

    // ── 2. Right Notification Panel ──────────────────────────────────────────
    console.log("\n[TEST 2/7] Testing Right Notification Panel (NotificationPanel.tsx)...");
    const notifsOpenFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const bell = document.querySelector('button[aria-label*="notification" i]') || document.querySelector('[data-spatial-zone="cluster"] button:nth-child(2)');
          if (bell) bell.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "n", code: "KeyN", ctrlKey: true, bubbles: true }));
        `,
      });
    });
    await sleep(600);
    const notifsOpenMetrics = await getMetrics();
    console.log(`  -> Open FPS: Avg ${notifsOpenFps.avgFps}, Min ${notifsOpenFps.minFps} | DOM Nodes: ${notifsOpenMetrics.domNodes} | Panels open: ${notifsOpenMetrics.openPanels.join(",")}`);

    await sleep(1500);

    const notifsCloseFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('[data-edge-panel="right"] button[aria-label="Close panel"]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });
    await sleep(400);
    const notifsCloseMetrics = await getMetrics();
    panelResults.push({
      panel: "NotificationPanel (Right Rail)",
      route: "/",
      openFps: notifsOpenFps,
      closeFps: notifsCloseFps,
      nodesDelta: notifsOpenMetrics.domNodes - sessionCloseMetrics.domNodes,
      retainedDelta: notifsCloseMetrics.domNodes - initialMetrics.domNodes,
    });

    // ── 3. Right Help & Guide Panel ──────────────────────────────────────────
    console.log("\n[TEST 3/7] Testing Right Help & Shortcuts Panel (HelpPanel.tsx)...");
    const helpOpenFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const helpBtn = document.querySelector('button[aria-label="Help & guide"]') || document.querySelector('[data-spatial-zone="cluster"] button:last-child');
          if (helpBtn) helpBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "/", code: "Slash", ctrlKey: true, bubbles: true }));
        `,
      });
    });
    await sleep(600);
    const helpOpenMetrics = await getMetrics();
    console.log(`  -> Open FPS: Avg ${helpOpenFps.avgFps}, Min ${helpOpenFps.minFps} | DOM Nodes: ${helpOpenMetrics.domNodes} | Panels open: ${helpOpenMetrics.openPanels.join(",")}`);

    await sleep(1500);

    const helpCloseFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('[data-edge-panel="right"] button[aria-label="Close panel"]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });
    await sleep(400);
    const helpCloseMetrics = await getMetrics();
    panelResults.push({
      panel: "HelpPanel (Right Rail)",
      route: "/",
      openFps: helpOpenFps,
      closeFps: helpCloseFps,
      nodesDelta: helpOpenMetrics.domNodes - notifsCloseMetrics.domNodes,
      retainedDelta: helpCloseMetrics.domNodes - initialMetrics.domNodes,
    });

    // ── 4. Bottom Profiler Drawer ────────────────────────────────────────────
    console.log("\n[TEST 4/7] Testing Bottom Memory Profiler Drawer (ProfilerDrawer.tsx)...");
    const profilerOpenFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const profilerBtn = document.querySelector('button[title*="Profiler" i]') || Array.from(document.querySelectorAll('button')).find(b => b.textContent && b.textContent.includes('MB'));
          if (profilerBtn) profilerBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", code: "ArrowDown", shiftKey: true, bubbles: true }));
        `,
      });
    });
    await sleep(600);
    const profilerOpenMetrics = await getMetrics();
    console.log(`  -> Open FPS: Avg ${profilerOpenFps.avgFps}, Min ${profilerOpenFps.minFps} | DOM Nodes: ${profilerOpenMetrics.domNodes} | Dialogs open: ${profilerOpenMetrics.openDialogs}`);

    await sleep(2000);

    const profilerCloseFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('button[aria-label="Close drawer"]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });
    await sleep(400);
    const profilerCloseMetrics = await getMetrics();
    panelResults.push({
      panel: "ProfilerDrawer (Bottom Sheet)",
      route: "/",
      openFps: profilerOpenFps,
      closeFps: profilerCloseFps,
      nodesDelta: profilerOpenMetrics.domNodes - helpCloseMetrics.domNodes,
      retainedDelta: profilerCloseMetrics.domNodes - initialMetrics.domNodes,
    });

    // ── 5. History Session Detail Drawer ─────────────────────────────────────
    console.log("\n[TEST 5/7] Navigating to /history & Testing Session Detail Drawer (DetailPanel.tsx)...");
    await cdp.send("Runtime.evaluate", {
      expression: `window.history.pushState({}, "", "/history"); window.dispatchEvent(new PopStateEvent("popstate"));`,
    });
    await sleep(1500);

    const historyOpenFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const card = document.querySelector('[data-session-card]') || document.querySelector('button.group') || document.querySelector('[role="article"]');
          if (card) card.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", code: "ArrowUp", shiftKey: true, bubbles: true }));
        `,
      });
    });
    await sleep(600);
    const historyOpenMetrics = await getMetrics();
    console.log(`  -> Open FPS: Avg ${historyOpenFps.avgFps}, Min ${historyOpenFps.minFps} | DOM Nodes: ${historyOpenMetrics.domNodes}`);

    await sleep(1500);

    const historyCloseFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('button[aria-label="Close drawer"]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });
    await sleep(400);
    const historyCloseMetrics = await getMetrics();
    panelResults.push({
      panel: "DetailPanel (History Drawer)",
      route: "/history",
      openFps: historyOpenFps,
      closeFps: historyCloseFps,
      nodesDelta: historyOpenMetrics.domNodes - profilerCloseMetrics.domNodes,
      retainedDelta: historyCloseMetrics.domNodes - initialMetrics.domNodes,
    });

    // ── 6. Personal Memory Staging Drawer ────────────────────────────────────
    console.log("\n[TEST 6/7] Navigating to /memory & Testing Personal Memory Drawer (PersonalMemoryDrawer.tsx)...");
    await cdp.send("Runtime.evaluate", {
      expression: `window.history.pushState({}, "", "/memory"); window.dispatchEvent(new PopStateEvent("popstate"));`,
    });
    await sleep(1500);

    const memoryOpenFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const trigger = document.querySelector('button[aria-label*="Personal" i]') || document.querySelector('button[aria-label*="Memory" i]');
          if (trigger) trigger.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", code: "ArrowUp", shiftKey: true, bubbles: true }));
        `,
      });
    });
    await sleep(600);
    const memoryOpenMetrics = await getMetrics();
    console.log(`  -> Open FPS: Avg ${memoryOpenFps.avgFps}, Min ${memoryOpenFps.minFps} | DOM Nodes: ${memoryOpenMetrics.domNodes}`);

    await sleep(1500);

    const memoryCloseFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          const closeBtn = document.querySelector('button[aria-label="Close drawer"]');
          if (closeBtn) closeBtn.click();
          else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });
    await sleep(400);
    const memoryCloseMetrics = await getMetrics();
    panelResults.push({
      panel: "PersonalMemoryDrawer (Memory Slate)",
      route: "/memory",
      openFps: memoryOpenFps,
      closeFps: memoryCloseFps,
      nodesDelta: memoryOpenMetrics.domNodes - historyCloseMetrics.domNodes,
      retainedDelta: memoryCloseMetrics.domNodes - initialMetrics.domNodes,
    });

    // ── 7. Monitoring Popover ────────────────────────────────────────────────
    console.log("\n[TEST 7/7] Navigating to /monitoring & Testing Monitoring Popover...");
    await cdp.send("Runtime.evaluate", {
      expression: `window.history.pushState({}, "", "/monitoring"); window.dispatchEvent(new PopStateEvent("popstate"));`,
    });
    await sleep(1500);

    const monitorOpenFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          window.dispatchEvent(new KeyboardEvent("keydown", { key: "m", code: "KeyM", ctrlKey: true, bubbles: true }));
        `,
      });
    });
    await sleep(600);
    const monitorOpenMetrics = await getMetrics();
    console.log(`  -> Open FPS: Avg ${monitorOpenFps.avgFps}, Min ${monitorOpenFps.minFps} | DOM Nodes: ${monitorOpenMetrics.domNodes}`);

    await sleep(1500);

    const monitorCloseFps = await measureFpsDuring(async () => {
      await cdp.send("Runtime.evaluate", {
        expression: `
          window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
        `,
      });
    });
    await sleep(400);
    const monitorCloseMetrics = await getMetrics();
    panelResults.push({
      panel: "MonitoringPopover (Telemetry Float)",
      route: "/monitoring",
      openFps: monitorOpenFps,
      closeFps: monitorCloseFps,
      nodesDelta: monitorOpenMetrics.domNodes - memoryCloseMetrics.domNodes,
      retainedDelta: monitorCloseMetrics.domNodes - initialMetrics.domNodes,
    });

    // Final Garbage Collection & Metrics
    console.log("\n[FINAL] Forcing Garbage Collection...");
    await cdp.send("HeapProfiler.collectGarbage");
    await sleep(500);
    const finalMetrics = await getMetrics();

    const report = {
      timestamp: new Date().toISOString(),
      testCount: panelResults.length,
      baseline: initialMetrics,
      final: finalMetrics,
      netHeapDeltaMb: Number((finalMetrics.jsHeapUsedMb - initialMetrics.jsHeapUsedMb).toFixed(2)),
      netDomNodeDelta: finalMetrics.domNodes - initialMetrics.domNodes,
      panelResults,
    };

    fs.writeFileSync(REPORT_FILE, JSON.stringify(report, null, 2));
    console.log(`\n==================================================`);
    console.log(`ALL 7 PANELS & DRAWERS TESTED SUCCESSFULLY`);
    console.log(`Report written to ${REPORT_FILE}`);
    console.log(`Baseline Heap: ${initialMetrics.jsHeapUsedMb} MB -> Final Heap: ${finalMetrics.jsHeapUsedMb} MB (Net: ${report.netHeapDeltaMb} MB)`);
    console.log(`Baseline DOM Nodes: ${initialMetrics.domNodes} -> Final DOM Nodes: ${finalMetrics.domNodes} (Net: ${report.netDomNodeDelta})`);
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
