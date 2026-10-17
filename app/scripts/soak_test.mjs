/**
 * Vox Long-Running CDP Soak Runner
 * 
 * Conducts extended soak testing over 10-15 minutes with realistic dwell times (4-8s per page).
 * Evaluates whether long-running rAF loops, background polling, or WebGL contexts drift over time.
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

const SOAK_TELEMETRY_FILE = path.join(RESULTS_DIR, "soak_telemetry.jsonl");
const SOAK_REPORT_FILE = path.join(RESULTS_DIR, "soak_test_report.json");

// Clear existing soak telemetry
fs.writeFileSync(SOAK_TELEMETRY_FILE, "");

const CHROME_PORT = 9223; // use distinct port from rapid stress runner
const TARGET_URL = "http://localhost:1420/";

// Parse --duration=<minutes> (default 10 mins)
const args = process.argv.slice(2);
const durationArg = args.find((a) => a.startsWith("--duration="));
const DURATION_MINUTES = durationArg ? parseFloat(durationArg.split("=")[1]) : 10;
const DURATION_MS = DURATION_MINUTES * 60 * 1000;

console.log(`[SOAK RUNNER] Configured for ${DURATION_MINUTES} minutes soak test (${DURATION_MS / 1000}s total)...`);

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
    }
  }
}

async function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

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

    await sleep(2000);

    const routes = [
      { path: "/", name: "Home", dwellSec: 6 },
      { path: "/history", name: "History", dwellSec: 5 },
      { path: "/memory", name: "Memory", dwellSec: 6 },
      { path: "/settings", name: "Settings", dwellSec: 5 },
      { path: "/monitoring", name: "Monitoring", dwellSec: 5 },
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
      };
    };

    const startTime = Date.now();
    const baseline = await getMetrics();
    console.log(`[SOAK BASELINE] Heap: ${baseline.jsHeapUsedMb} MB | Nodes: ${baseline.domNodes} | Listeners: ${baseline.jsEventListeners}`);

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

        // Dwell with natural interaction beats
        const dwellHalf = Math.floor((r.dwellSec * 1000) / 2);
        await sleep(dwellHalf);

        // Interaction beat: exercise real panel/drawer for current view
        await cdp.send("Runtime.evaluate", {
          expression: `
            if (location.pathname === "/") {
              const bell = document.querySelector('button[aria-label*="notification" i]');
              if (bell) bell.click();
            } else if (location.pathname === "/history") {
              const card = document.querySelector('[data-session-card]') || document.querySelector('button.group');
              if (card) card.click();
            } else if (location.pathname === "/memory") {
              const staging = document.querySelector('button[aria-label*="Personal" i]');
              if (staging) staging.click();
            }
          `,
        });
        await sleep(1500);

        // Close overlay
        await cdp.send("Runtime.evaluate", {
          expression: `
            const closeBtn = document.querySelector('button[aria-label="Close panel"]') || document.querySelector('button[aria-label="Close drawer"]');
            if (closeBtn) closeBtn.click();
            else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
          `,
        });

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
    const driftMbPerMin = Number((deltaHeapMb / actualElapsedMins).toFixed(3));

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
    };

    fs.writeFileSync(SOAK_REPORT_FILE, JSON.stringify(report, null, 2));
    console.log("==================================================");
    console.log("[SOAK RESULT] Soak Test Finished:");
    console.log(`              Duration:         ${actualElapsedMins} mins`);
    console.log(`              Post-GC Heap:     ${postGc.jsHeapUsedMb} MB (Delta: ${deltaHeapMb >= 0 ? "+" : ""}${deltaHeapMb} MB)`);
    console.log(`              Drift Rate:       ${driftMbPerMin >= 0 ? "+" : ""}${driftMbPerMin} MB/min (${report.analysis.soakStability})`);
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
