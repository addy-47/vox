#!/usr/bin/env node
/**
 * Theme-flip cost probe (CDP / Chromium).
 *
 * Answers the question the WebKit harness cannot be asked right now: does the
 * flip cost scale with the number of open glass cards?
 *
 * Method
 *   1. Navigate to a route.
 *   2. Count the things that should drive flip cost:
 *        - `.glass-card` / `.glass` count          (blur regions)
 *        - `[class*="backdrop-blur"]` count        (raw blur regions)
 *        - elements matched by the flip gate       (style-recalc fan-out)
 *   3. Apply N synthetic `.glass-card` overlays to simulate "all cards open"
 *      WITHOUT changing app state, so the only variable is blur-region count.
 *   4. Flip the theme with real rAF frame sampling: record every frame's
 *      duration for the length of the flip window, plus long tasks.
 *
 * Why synthetic overlays: the real card count is whatever the user has open,
 * which is not reproducible. Injecting glass cards directly isolates
 * "blur region count" as the sole independent variable.
 *
 * CAVEAT: this is Chromium, not the WebKitGTK runtime Tauri ships on. Absolute
 * frame times do not transfer. The SCALING does: if cost is a function of blur
 * region count here, the same term exists on WebKit.
 *
 * USAGE
 *   node .agents/skills/review-ui/scripts/probe_theme_flip.mjs --route=/settings
 *   node .agents/skills/review-ui/scripts/probe_theme_flip.mjs --steps=0,3,12,40
 */

import { spawn } from "child_process";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_DIR = path.resolve(__dirname, "..", "..", "..");
const RESULTS_DIR = path.join(REPO_DIR, "sandbox", "results", "theme_flip");
fs.mkdirSync(RESULTS_DIR, { recursive: true });

const args = process.argv.slice(2);
const arg = (n, d) => {
  const hit = args.find((a) => a.startsWith(`--${n}=`));
  return hit ? hit.split("=")[1] : d;
};

const PORT = Number(arg("port", 9333));
const ROUTE = arg("route", "/settings");
const LABEL = arg("label", "probe");
const BASE = arg("base", "http://localhost:1420");
const STEPS = arg("steps", "0,2,6,16,40").split(",").map(Number);
const KILL_BLUR = arg("killblur", "") === "1";
const FLIPS = Number(arg("flips", 5));
const WINDOW_MS = 700;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

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
      this.ws.onerror = reject;
      this.ws.onmessage = (e) => {
        const m = JSON.parse(e.data);
        if (m.id && this.pending.has(m.id)) {
          const { resolve, reject } = this.pending.get(m.id);
          this.pending.delete(m.id);
          m.error ? reject(new Error(m.error.message)) : resolve(m.result);
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
  async evaluate(expression, awaitPromise = false) {
    const r = await this.send("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise,
    });
    if (r.exceptionDetails) {
      throw new Error(r.exceptionDetails.exception?.description || "eval failed");
    }
    return r.result.value;
  }
  close() {
    try {
      this.ws?.close();
    } catch {}
  }
}

/** Injected into the page. Counts the cost drivers + flips + samples frames. */
const PAGE_PROBE = `
window.__probe = {
  async census() {
    const glassCards = document.querySelectorAll(".glass-card").length;
    const glass = document.querySelectorAll(".glass").length;
    const rawBlur = document.querySelectorAll('[class*="backdrop-blur"]').length;
    const inlineBlur = document.querySelectorAll('[style*="backdrop-filter"]').length;
    let gateMatched = 0;
    try {
      gateMatched = document.querySelectorAll(
        '[data-theme-transition] *, [data-theme-transition] *:not(.glass):not(.glass-card)'
      ).length;
    } catch (e) { gateMatched = -1; }
    return {
      glassCards, glass, rawBlur, inlineBlur,
      blurRegions: document.querySelectorAll(
        '[style*="backdrop-filter"], [class*="backdrop-blur"], .glass, .glass-card'
      ).length,
      gateMatched,
      total: document.querySelectorAll("*").length,
    };
  },

  injectGlassCards(n) {
    document.getElementById("__probe_layer")?.remove();
    if (n <= 0) return 0;
    const layer = document.createElement("div");
    layer.id = "__probe_layer";
    layer.style.cssText =
      "position:fixed;inset:0;z-index:1;pointer-events:none;display:flex;flex-wrap:wrap;align-content:flex-start;gap:8px;padding:8px;overflow:hidden";
    for (let i = 0; i < n; i++) {
      const c = document.createElement("div");
      // Match a real Settings card: .glass-card geometry and a nested control.
      c.className = "glass-card rounded-2xl";
      c.style.cssText = "width:340px;height:180px";
      c.innerHTML =
        '<div class="glass rounded-lg" style="width:200px;height:40px"></div>' +
        '<button class="rounded-xl border px-2 py-1">btn</button>';
      layer.appendChild(c);
    }
    document.body.appendChild(layer);
    return layer.querySelectorAll(".glass-card").length;
  },

  // A/B: same flip, but every blur region is dropped for the window. This is
  // the decisive test of "is backdrop-filter the cost driver, or is it the
  // style-recalc fan-out?" If TBT collapses with blur off, blur is the driver.
  armBlurSuppression(on) {
    let el = document.getElementById("__probe_killblur");
    if (!on) { el?.remove(); return false; }
    if (!el) {
      el = document.createElement("style");
      el.id = "__probe_killblur";
      el.textContent =
        '[class*="backdrop-blur"],[style*="backdrop-filter"],.glass,.glass-card{' +
        "backdrop-filter:none!important;-webkit-backdrop-filter:none!important}";
      document.head.appendChild(el);
    }
    return true;
  },

  async flip(frames) {
    const root = document.documentElement;
    const next = root.getAttribute("data-theme") === "light" ? "dark" : "light";
    const longTasks = [];
    const obs = new PerformanceObserver((list) => {
      for (const e of list.getEntries()) longTasks.push(Math.round(e.duration));
    });
    try { obs.observe({ entryTypes: ["longtask"] }); } catch {}

    const deltas = [];
    let last = performance.now();
    let done = false;
    const tick = () => {
      const now = performance.now();
      deltas.push(now - last);
      last = now;
      if (!done) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);

    root.setAttribute("data-theme-transition", "true");
    root.setAttribute("data-theme", next);
    await new Promise((r) => setTimeout(r, 400));
    root.removeAttribute("data-theme-transition");
    done = true;
    obs.disconnect();

    await new Promise((r) => setTimeout(r, 250));
    const d = deltas.slice(2).filter((x) => x > 0).sort((a, b) => a - b);
    const pct = (p) => (d.length ? d[Math.min(d.length - 1, Math.floor(d.length * p))] : 0);
    return {
      frames: d.length,
      medianFrameMs: +pct(0.5).toFixed(2),
      p95FrameMs: +pct(0.95).toFixed(2),
      worstFrameMs: +(d[d.length - 1] || 0).toFixed(2),
      totalBlockingMs: +d
        .filter((x) => x > 50)
        .reduce((a, b) => a + (b - 50), 0)
        .toFixed(2),
      longTasks,
    };
  },
};
"probe-installed";
`;

async function main() {
  const chrome = spawn(
    "google-chrome",
    [
      `--remote-debugging-port=${PORT}`,
      "--headless=new",
      "--no-sandbox",
      "--disable-gpu",
      "--hide-scrollbars",
      "--window-size=1600,1000",
      "about:blank",
    ],
    { stdio: "ignore" }
  );

  let wsUrl = null;
  for (let i = 0; i < 40 && !wsUrl; i++) {
    await sleep(250);
    try {
      const r = await fetch(`http://127.0.0.1:${PORT}/json/list`);
      const tabs = await r.json();
      const page = tabs.find((t) => t.type === "page");
      if (page?.webSocketDebuggerUrl) wsUrl = page.webSocketDebuggerUrl;
    } catch {}
  }
  if (!wsUrl) {
    chrome.kill();
    throw new Error("chrome did not expose a CDP page");
  }

  const cdp = new CDPClient(wsUrl);
  await cdp.connect();
  await cdp.send("Page.enable");
  await cdp.send("Runtime.enable");

  const consoleErrors = [];
  cdp.ws.addEventListener("message", (e) => {
    const m = JSON.parse(e.data);
    if (m.method === "Runtime.exceptionThrown") {
      consoleErrors.push(
        m.params.exceptionDetails?.exception?.description?.slice(0, 200) || "exception"
      );
    }
  });

  await cdp.send("Page.navigate", { url: `${BASE}${ROUTE}` });
  await sleep(5000);
  await cdp.evaluate(PAGE_PROBE);

  const installed = await cdp.evaluate("typeof window.__probe === 'object'");
  const booted = await cdp.evaluate(
    "({ title: document.title, rootChildren: document.getElementById('root')?.children.length ?? -1 })"
  );

  await cdp.evaluate(`window.__probe.armBlurSuppression(${KILL_BLUR})`, true);

  const rows = [];
  for (const n of STEPS) {
    const injected = await cdp.evaluate(`window.__probe.injectGlassCards(${n})`);
    await sleep(300);
    const census = await cdp.evaluate("window.__probe.census()", true);
    const runs = [];
    for (let f = 0; f < FLIPS; f++) runs.push(await cdp.evaluate("window.__probe.flip()", true));
    const med = (k) => {
      const v = runs.map((r) => r[k]).sort((a, b) => a - b);
      return +v[Math.floor(v.length / 2)].toFixed(2);
    };
    rows.push({
      injectedCards: injected,
      ...census,
      medianFrameMs: med("medianFrameMs"),
      p95FrameMs: med("p95FrameMs"),
      worstFrameMs: med("worstFrameMs"),
      totalBlockingMs: med("totalBlockingMs"),
    });
    console.log(
      `${KILL_BLUR ? "[blur OFF]" : "[blur ON ]"} cards+${String(n).padStart(2)}  blur=${String(census.blurRegions).padStart(3)}  ` +
        `nodes=${String(census.total).padStart(5)}  gate=${String(census.gateMatched).padStart(5)}  ` +
        `medianFrame=${String(rows.at(-1).medianFrameMs).padStart(7)}ms  ` +
        `p95=${String(rows.at(-1).p95FrameMs).padStart(7)}ms  ` +
        `worst=${String(rows.at(-1).worstFrameMs).padStart(7)}ms  ` +
        `tbt=${String(rows.at(-1).totalBlockingMs).padStart(7)}ms`
    );
  }

  const out = {
    route: ROUTE,
    base: BASE,
    engine: "chromium (headless) — NOT WebKitGTK; scaling only",
    blurSuppressed: KILL_BLUR,
    booted,
    probeInstalled: installed,
    consoleErrors: consoleErrors.slice(0, 5),
    steps: rows,
  };
  const file = path.join(RESULTS_DIR, `${LABEL}.json`);
  fs.writeFileSync(file, JSON.stringify(out, null, 2));

  console.log(`\nbooted: ${JSON.stringify(booted)}  probe:${installed}`);
  if (consoleErrors.length) console.log(`console errors: ${consoleErrors.slice(0, 3)}`);
  console.log(`-> ${file}`);

  cdp.close();
  chrome.kill();
}

main().catch((e) => {
  console.error("PROBE FAILED:", e.message);
  process.exit(1);
});