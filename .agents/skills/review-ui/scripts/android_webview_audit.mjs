#!/usr/bin/env node
// ─── Vox Android WebView Audit (device-attached CDP) ──────────────────────────
//
// Why this exists
// ---------------
// `stress_drawers.mjs` and `soak_test.mjs` spawn `/usr/bin/google-chrome` against
// `http://localhost:1420/`. That is desktop **Blink**. The Android build runs the
// same React tree inside a system **WebKit** WebView, with different compositor,
// different safe-area insets, coarse-pointer input, and a real thermal budget.
// A green desktop run is NOT evidence about the phone.
//
// This script attaches over `adb forward` to the *actual device WebView* and runs
// the Android-specific checks desktop Chrome cannot answer for.
//
// Requirements
// ------------
//   - The app must be a DEBUG build. wry only calls
//     `setWebContentsDebuggingEnabled(true)` under `#[cfg(debug_assertions)]`
//     (wry-0.55.0/src/android/main_pipe.rs:258). A release APK has no devtools
//     socket at all.
//   - The app must be running (not force-stopped), or there is no socket.
//
// Usage
// -----
//   node .agents/skills/review-ui/scripts/android_webview_audit.mjs
//   node .agents/skills/review-ui/scripts/android_webview_audit.mjs --route /history
//   node .agents/skills/review-ui/scripts/android_webview_audit.mjs --soak 180
//
// No npm dependencies: Node >= 22 supplies a global WebSocket.

import { spawnSync } from "node:child_process";

const PKG = process.env.VOX_PKG || "com.addy.Vox";
const LOCAL_PORT = Number(process.env.VOX_CDP_PORT || 9222);

// ── arg parsing ───────────────────────────────────────────────────────────────
const argv = process.argv.slice(2);
function arg(flag, fallback = null) {
  const i = argv.indexOf(flag);
  return i === -1 ? fallback : argv[i + 1];
}
const ROUTE = arg("--route", null);
const SOAK_SECS = Number(arg("--soak", 0)) || 0;

// ── adb helpers ───────────────────────────────────────────────────────────────
function adb(args, { quiet = false } = {}) {
  const r = spawnSync("adb", args, { encoding: "utf8" });
  if (r.status !== 0) {
    if (!quiet) {
      process.stderr.write(`adb ${args.join(" ")} failed: ${r.stderr}\n`);
    }
    return null;
  }
  return (r.stdout || "").trim();
}

function die(msg, hint) {
  process.stderr.write(`\nFATAL: ${msg}\n`);
  if (hint) process.stderr.write(`HINT:  ${hint}\n\n`);
  process.exit(1);
}

/**
 * Resolves the WebView devtools abstract socket. The suffix is the app PID, which
 * Android restarts on every cold launch — so it is re-read every invocation
 * rather than cached.
 */
function findWebviewSocket() {
  const pid = adb(["shell", "pidof", PKG]);
  if (!pid) {
    die(`${PKG} is not running.`,
        "Launch it first: ./app/src-tauri/scripts/android-dev.sh restart");
  }
  const unix = adb(["shell", "cat", "/proc/net/unix"]) || "";
  const suffix = `webview_devtools_remote_${pid}`;
  const line = unix.split("\n").find((l) => l.includes(suffix));
  if (!line) {
    die(`No WebView devtools socket for pid ${pid}.`,
        "Almost always means a RELEASE apk — wry gates devtools behind " +
        "debug_assertions. Build with ./scripts/android-dev.sh debug");
  }
  return { pid: pid.trim(), socket: `localabstract:${suffix.trim()}` };
}

/** Forwards the device socket to a localhost CDP port. */
function forwardPort(socket) {
  // Clear any stale mapping first; adb refuses to overwrite one.
  adb(["forward", "--remove", `tcp:${LOCAL_PORT}`], { quiet: true });
  const ok = adb(["forward", `tcp:${LOCAL_PORT}`, socket]);
  if (ok === null) {
    die(`adb forward failed for ${socket}.`,
        "Check `adb devices` and the udev rule for Xiaomi vendor 2717.");
  }
}

/** Polls the CDP HTTP endpoint until the WebView target is listed. */
async function listTargets(timeoutMs = 10000) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`http://127.0.0.1:${LOCAL_PORT}/json/list`);
      if (res.ok) {
        const targets = await res.json();
        const page = targets.find((t) => t.type === "page" && t.webSocketDebuggerUrl);
        if (page) return { page, targets };
        last = `no page target among ${targets.length}`;
      } else {
        last = `HTTP ${res.status}`;
      }
    } catch (e) {
      last = e.message;
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  die(`CDP endpoint never listed a page target (${last}).`,
      "The WebView may still be booting. Re-run once the UI is visible.");
}

// ── minimal CDP client ────────────────────────────────────────────────────────
class Cdp {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    this.consoleErrors = [];
    ws.addEventListener("message", (ev) => {
      let msg;
      try { msg = JSON.parse(ev.data); } catch { return; }
      if (msg.id !== undefined && this.pending.has(msg.id)) {
        const { resolve, reject } = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result);
        return;
      }
      if (msg.method === "Runtime.exceptionThrown") {
        this.consoleErrors.push(
          msg.params?.exceptionDetails?.exception?.description ||
          msg.params?.exceptionDetails?.text || "unknown exception");
      }
    });
  }

  send(method, params = {}) {
    const id = ++this.id;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      setTimeout(() => {
        if (this.pending.delete(id)) reject(new Error(`${method} timed out`));
      }, 20000);
    });
  }

  /** Evaluates an async expression in the page and returns its JSON value. */
  async evalJson(expression) {
    const r = await this.send("Runtime.evaluate", {
      expression: `(async () => { ${expression} })()`,
      awaitPromise: true,
      returnByValue: true,
    });
    if (r.exceptionDetails) {
      throw new Error(
        r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    }
    return r.result.value;
  }
}

function connect(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    ws.addEventListener("open", () => resolve(new Cdp(ws)));
    ws.addEventListener("error", () => reject(new Error(`cannot open ${wsUrl}`)));
    setTimeout(() => reject(new Error("websocket open timed out")), 10000);
  });
}

// ── the probe, evaluated inside the device WebView ────────────────────────────
const PROBE = `
  const out = {};
  const sleep = (ms) => new Promise(r => setTimeout(r, ms));

  // ── viewport & insets ────────────────────────────────────────────────────
  out.viewport = {
    innerWidth: window.innerWidth,
    innerHeight: window.innerHeight,
    dpr: window.devicePixelRatio,
    screenW: screen.width,
    screenH: screen.height,
    orientation: screen.orientation?.type ?? "unknown",
  };
  // env() resolves to a px string; a zero value means the inset is absent, which
  // on an edge-to-edge Android build would mean content sits under the status
  // bar or the gesture handle.
  const cs = getComputedStyle(document.documentElement);
  const probe = document.createElement("div");
  probe.style.cssText =
    "position:fixed;top:0;left:0;visibility:hidden;" +
    "padding-top:env(safe-area-inset-top);padding-bottom:env(safe-area-inset-bottom);";
  document.body.appendChild(probe);
  const pcs = getComputedStyle(probe);
  out.safeArea = {
    top: parseFloat(pcs.paddingTop) || 0,
    bottom: parseFloat(pcs.paddingBottom) || 0,
  };
  probe.remove();

  // ── horizontal overflow ──────────────────────────────────────────────────
  // 480px-class clipping is a known Android defect class; catch it generically.
  out.overflow = {
    docScrollW: document.documentElement.scrollWidth,
    docClientW: document.documentElement.clientWidth,
    overflowing: document.documentElement.scrollWidth >
                 document.documentElement.clientWidth + 1,
  };

  // ── touch-target audit (style guide §5.2: >= 48x48) ─────────────────────
  const SEL = "button, a[href], [role=button], input, select, textarea, [tabindex]";
  const small = [];
  const seen = new Set();
  for (const el of document.querySelectorAll(SEL)) {
    if (seen.has(el)) continue;
    seen.add(el);
    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) continue;      // hidden
    const cs2 = getComputedStyle(el);
    if (cs2.visibility === "hidden" || cs2.display === "none") continue;
    const label = (el.getAttribute("aria-label") ||
                   el.textContent?.trim().slice(0, 28) || el.tagName)
                   .replace(/\\s+/g, " ");
    if (r.width < 48 || r.height < 48) {
      small.push({ label, w: Math.round(r.width), h: Math.round(r.height) });
    }
  }
  out.touchTargets = { violations: small.length, worst: small.slice(0, 12) };

  // ── hover-only affordances (style guide §5.3) ────────────────────────────
  // On a coarse pointer nothing is hoverable. Count interactive elements whose
  // only affordance is a CSS :hover rule.
  out.hoverRisk = {
    coarsePointer: matchMedia("(pointer: coarse)").matches,
    // Any element styled with a :hover rule but no always-visible state cannot
    // be reported statically; instead count elements that carry a title attr
    // (native tooltip — useless on touch) as a proxy.
    titleTooltips: document.querySelectorAll("[title]").length,
  };

  // ── WebGL / orb renderer ─────────────────────────────────────────────────
  try {
    const c = document.createElement("canvas");
    const gl = c.getContext("webgl2") || c.getContext("webgl");
    if (!gl) {
      out.webgl = { available: false };
    } else {
      const dbg = gl.getExtension("WEBGL_debug_renderer_info");
      out.webgl = {
        available: true,
        version: gl.getParameter(gl.VERSION),
        renderer: dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : "hidden",
        maxTexture: gl.getParameter(gl.MAX_TEXTURE_SIZE),
      };
    }
  } catch (e) { out.webgl = { available: false, error: String(e) }; }

  // ── DOM size (leak baseline) ─────────────────────────────────────────────
  out.dom = {
    nodes: document.getElementsByTagName("*").length,
    listenersHint: document.body ? "n/a" : "no body",
  };

  // ── frame timing over 3s ─────────────────────────────────────────────────
  const frames = [];
  await new Promise((done) => {
    let last = performance.now();
    const t0 = last;
    const tick = (now) => {
      frames.push(now - last);
      last = now;
      if (now - t0 < 3000) requestAnimationFrame(tick);
      else done();
    };
    requestAnimationFrame(tick);
  });
  frames.sort((a, b) => a - b);
  const pct = (p) => frames[Math.min(frames.length - 1, Math.floor(frames.length * p))] || 0;
  out.fps = {
    samples: frames.length,
    avgMs: +(frames.reduce((a, b) => a + b, 0) / frames.length).toFixed(2),
    medianMs: +pct(0.5).toFixed(2),
    p95Ms: +pct(0.95).toFixed(2),
    worstMs: +frames[frames.length - 1].toFixed(2),
    longFrames_gt50ms: frames.filter((f) => f > 50).length,
  };

  // ── route + title, for the report header ─────────────────────────────────
  out.route = { href: location.pathname, title: document.title };
  return out;
`;

// ── navigation helper ─────────────────────────────────────────────────────────
async function gotoRoute(cdp, route) {
  await cdp.evalJson(`history.pushState({}, "", ${JSON.stringify(route)});
    window.dispatchEvent(new PopStateEvent("popstate"));
    await sleep(600);
    return true;`).catch(async () => {
    // pushState is a no-op under some routers; fall back to a real navigation.
    await cdp.send("Page.navigate", { url: route });
    await new Promise((r) => setTimeout(r, 1500));
    return true;
  });
}

// ── main ──────────────────────────────────────────────────────────────────────
const { pid, socket } = findWebviewSocket();
forwardPort(socket);

const { page, targets } = await listTargets();
console.log(`\n══ Vox Android WebView Audit ══`);
console.log(`   pid       ${pid}`);
console.log(`   socket    ${socket} -> 127.0.0.1:${LOCAL_PORT}`);
console.log(`   target    ${page.id} ${page.url}`);
console.log(`   targets   ${targets.length}\n`);

const cdp = await connect(page.webSocketDebuggerUrl);
await cdp.send("Runtime.enable");
await cdp.send("Page.enable");

if (ROUTE) {
  console.log(`── navigating to ${ROUTE}`);
  await gotoRoute(cdp, ROUTE);
}

console.log("── running probe");
const result = await cdp.evalJson(PROBE);

const report = {
  when: new Date().toISOString(),
  device: adb(["shell", "getprop", "ro.product.model"]),
  android: adb(["shell", "getprop", "ro.build.version.release"]),
  sdk: adb(["shell", "getprop", "ro.build.version.sdk"]),
  abi: adb(["shell", "getprop", "ro.product.cpu.abi"]),
  ...result,
  consoleErrors: cdp.consoleErrors,
};

if (SOAK_SECS > 0) {
  console.log(`── soak ${SOAK_SECS}s (route cycling + DOM drift)`);
  const routes = ["/", "/history", "/memory", "/settings"];
  const samples = [];
  const baseline = result.dom.nodes;
  const t0 = Date.now();
  let i = 0;
  while (Date.now() - t0 < SOAK_SECS * 1000) {
    const r = routes[i++ % routes.length];
    await gotoRoute(cdp, r).catch(() => {});
    const nodes = await cdp.evalJson(
      "return document.getElementsByTagName('*').length;");
    samples.push({ t: Math.round((Date.now() - t0) / 1000), route: r, nodes });
    await new Promise((res) => setTimeout(res, 4000));
  }
  const last = samples[samples.length - 1]?.nodes ?? baseline;
  report.soak = {
    seconds: SOAK_SECS,
    samples: samples.length,
    baselineNodes: baseline,
    finalNodes: last,
    drift: last - baseline,
  };
}

console.log(JSON.stringify(report, null, 2));

const verdict = [];
if (result.overflow.overflowing) verdict.push("FAIL horizontal overflow");
if (result.touchTargets.violations > 0)
  verdict.push(`WARN ${result.touchTargets.violations} touch targets < 48px`);
if (result.fps.p95Ms > 32) verdict.push(`WARN p95 frame ${result.fps.p95Ms}ms`);
if (report.soak && report.soak.drift > 3000)
  verdict.push(`WARN DOM drift +${report.soak.drift} nodes`);
if (cdp.consoleErrors.length)
  verdict.push(`WARN ${cdp.consoleErrors.length} console exceptions`);

console.log("\n── verdict");
console.log(verdict.length ? verdict.map((v) => `   ${v}`).join("\n")
                           : "   clean");
console.log();

adb(["forward", "--remove", `tcp:${LOCAL_PORT}`], { quiet: true });
process.exit(verdict.some((v) => v.startsWith("FAIL")) ? 1 : 0);