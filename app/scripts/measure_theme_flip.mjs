#!/usr/bin/env node
/**
 * Vox Theme Flip — Frame-Cost Measurement (WebKitGTK)
 *
 * Measures the dark<->light flip on the engine the app actually ships on.
 *
 * WHY THIS EXISTS: the existing review-ui scripts drive headless Chrome against
 * the Vite dev server. That is a different engine and a different rasterization
 * path from the WebKitGTK runtime Tauri uses, so it cannot see WebKitGTK-only
 * costs. A 1-second theme-flip stall shipped undetected for exactly that reason.
 * This script attaches to the WebKitGTK remote inspector instead.
 *
 * USAGE
 *   1. Launch the app with the inspector exposed:
 *        WEBKIT_INSPECTOR_SERVER=127.0.0.1:9222 pnpm tauri dev
 *   2. Navigate to the route you want to measure.
 *   3. In another shell:
 *        node .agents/skills/review-ui/scripts/measure_theme_flip.mjs
 *        node .agents/skills/review-ui/scripts/measure_theme_flip.mjs --route=/settings --label=all-cards-open
 *
 * The script does NOT navigate — it measures whatever route is currently open, so
 * the Tauri IPC bridge stays intact. Open the Settings Appearance card yourself
 * before measuring if that is the state you care about; the script records the
 * DOM node and blur-region counts so the state is self-describing.
 *
 * RESULTS -> sandbox/results/theme_flip/<label>.json
 *
 * GATE (assert by reading the output):
 *   - totalBlockingMs must not scale with openCards. Compare the `--label`
 *     runs: if all-cards-open costs materially more than one-card, the flip still
 *     has a per-card term.
 *   - timeToFirstChangedFrameMs should be well under one frame budget (~32ms).
 *     A large value means the flip is not landing on the next paint.
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_DIR = path.resolve(__dirname, "..", "..", "..");
const RESULTS_DIR = path.join(REPO_DIR, "sandbox", "results", "theme_flip");

const args = process.argv.slice(2);
const argValue = (name, fallback) => {
  const hit = args.find((a) => a.startsWith(`--${name}=`));
  return hit ? hit.split("=")[1] : fallback;
};

const ENDPOINT = argValue("endpoint", "127.0.0.1:9222");
const LABEL = argValue("label", "default");
const ROUTE = argValue("route", "unknown");
const FLIPS = Number(argValue("flips", "3"));
const SETTLE_MS = Number(argValue("settle", "700"));

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
          msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result);
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
    try {
      this.ws?.close();
    } catch {
      /* already closed */
    }
  }
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function resolvePageTarget() {
  const res = await fetch(`http://${ENDPOINT}/json`);
  if (!res.ok) throw new Error(`inspector /json returned ${res.status}`);
  const targets = await res.json();
  const page = targets.find((t) => t.type === "page" && t.webSocketDebuggerUrl);
  if (!page) {
    throw new Error(
      `no page target on ${ENDPOINT}. Launch with WEBKIT_INSPECTOR_SERVER=${ENDPOINT} and open the app window.`
    );
  }
  return page;
}

/**
 * Runs in the page. Clicks the real theme control and samples frame timing across
 * the flip window.
 *
 * timeToFirstChangedFrame is measured from the click to the first rAF that runs
 * AFTER `data-theme` has changed — i.e. the first frame that can possibly show
 * the new theme. A large value is the "I clicked and nothing happened" symptom.
 */
function measureInPage(flipCount, settleMs) {
  return new Promise((resolve) => {
    // Self-contained: this function is stringified and evaluated inside the page,
    // so it cannot close over anything from this module.
    const r2 = (n) => Math.round(n * 100) / 100;
    const root = document.documentElement;
    const blurSelector =
      '[style*="backdrop-filter"], [class*="backdrop-blur"], .glass-card, .glass-panel';

    const stats = {
      nodeCount: document.querySelectorAll("*").length,
      blurRegionCount: document.querySelectorAll(blurSelector).length,
      theme: root.getAttribute("data-theme"),
      hasAmbientField: !!document.querySelector(".amb-base"),
      hasToggle: false,
      flips: [],
    };

    const toggle =
      document.querySelector('button[aria-label*="light mode" i]') ||
      document.querySelector('button[aria-label*="dark mode" i]');
    if (!toggle) {
      resolve({ ...stats, error: "no theme toggle on this route" });
      return;
    }
    stats.hasToggle = true;

    const longTasks = [];
    let observer = null;
    try {
      observer = new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          longTasks.push({ start: entry.startTime, duration: entry.duration });
        }
      });
      observer.observe({ type: "longtask", buffered: false });
    } catch {
      observer = null;
    }

    const runs = [];
    let run = 0;

    const runOne = () => {
      const longTaskMark = longTasks.length;
      const t0 = performance.now();
      let firstChangedFrame = null;
      const frames = [];
      let last = t0;
      let sampling = true;

      const onMutate = () => {
        if (firstChangedFrame !== null) return;
        // Next frame after the attribute flip is the first frame that can paint
        // the new theme.
        requestAnimationFrame(() => {
          firstChangedFrame = performance.now() - t0;
        });
      };
      const mo = new MutationObserver(onMutate);
      mo.observe(root, { attributes: true, attributeFilter: ["data-theme"] });

      const tick = () => {
        const now = performance.now();
        frames.push(now - last);
        last = now;
        if (sampling && now - t0 < 400) requestAnimationFrame(tick);
        else sampling = false;
      };
      requestAnimationFrame(tick);

      toggle.click();

      setTimeout(() => {
        mo.disconnect();
        const window400 = longTasks.slice(longTaskMark);
        runs.push({
          timeToFirstChangedFrameMs: firstChangedFrame === null ? null : r2(firstChangedFrame),
          framesOver400Ms: frames.length,
          maxFrameDeltaMs: r2(Math.max(0, ...frames)),
          framesOver32ms: frames.filter((f) => f > 32).length,
          longTaskCount: window400.length,
          totalBlockingMs: r2(
            window400.reduce((sum, t) => sum + Math.max(0, t.duration - 50), 0)
          ),
          worstLongTaskMs: r2(Math.max(0, ...window400.map((t) => t.duration))),
        });
        run += 1;
        if (run < flipCount) setTimeout(runOne, settleMs);
        else {
          observer?.disconnect();
          resolve({ ...stats, route: location.pathname, themeAfter: root.getAttribute("data-theme"), runs });
        }
      }, 450);
    };

    setTimeout(runOne, 60);
  });
}

const round = (n) => Math.round(n * 100) / 100;

async function main() {
  console.log(`[THEME FLIP] attaching to inspector at ${ENDPOINT}`);
  const target = await resolvePageTarget();
  const cdp = new CDPClient(target.webSocketDebuggerUrl);
  await cdp.connect();
  console.log(`[THEME FLIP] attached to ${target.url}`);

  const result = await cdp.send("Runtime.evaluate", {
    expression: `(${measureInPage.toString()})(${FLIPS}, ${SETTLE_MS})`,
    awaitPromise: true,
    returnByValue: true,
  });

  if (result.exceptionDetails) {
    throw new Error(result.exceptionDetails.exception?.description ?? "in-page evaluation failed");
  }

  const data = result.result.value;
  fs.mkdirSync(RESULTS_DIR, { recursive: true });
  const outFile = path.join(RESULTS_DIR, `${LABEL}.json`);
  fs.writeFileSync(outFile, JSON.stringify({ endpoint: ENDPOINT, route: ROUTE, ...data }, null, 2));

  if (data.error) {
    console.error(`[THEME FLIP] ${data.error}`);
    console.error("            Open the Settings > Appearance card, or use a route that has a header toggle.");
  } else {
    console.log("");
    console.log(`  route                ${data.route}   (requested ${ROUTE})`);
    console.log(`  ambient field        ${data.hasAmbientField ? "MOUNTED" : "disabled"}`);
    console.log(`  DOM nodes            ${data.nodeCount}`);
    console.log(`  blur regions         ${data.blurRegionCount}`);
    console.log(`  theme                ${data.theme} -> ${data.themeAfter}`);
    console.log("");
    console.log("  run  firstChanged  maxFrame  >32ms  longTasks  blockingMs  worstTask");
    data.runs.forEach((r, i) => {
      console.log(
        `  ${String(i + 1).padEnd(4)}${String(r.timeToFirstChangedFrameMs ?? "n/a").padEnd(13)}` +
          `${String(r.maxFrameDeltaMs).padEnd(10)}${String(r.framesOver32ms).padEnd(8)}` +
          `${String(r.longTaskCount).padEnd(11)}${String(r.totalBlockingMs).padEnd(12)}${r.worstLongTaskMs}`
      );
    });
    console.log("");
    const blocking = Math.max(...data.runs.map((r) => r.totalBlockingMs));
    const firstChanged = Math.max(...data.runs.map((r) => r.timeToFirstChangedFrameMs ?? 0));
    console.log(`  worst blocking across runs: ${blocking}ms`);
    console.log(`  worst first-changed-frame: ${firstChanged}ms`);
    if (firstChanged > 32) {
      console.log("  [!] flip is not landing on the next paint");
    }
    if (blocking > 100) {
      console.log("  [!] heavy main-thread blocking during the flip");
    }
    console.log("");
    console.log(`  saved -> ${path.relative(REPO_DIR, outFile)}`);
    console.log("  compare labels: blocking time must NOT scale with blur-region count.");
  }

  cdp.close();
}

main().catch((err) => {
  console.error(`[THEME FLIP] ${err.message}`);
  console.error(
    "  Launch the app first:  WEBKIT_INSPECTOR_SERVER=127.0.0.1:9222 pnpm tauri dev"
  );
  process.exit(1);
});
