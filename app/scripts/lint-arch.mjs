#!/usr/bin/env node
/**
 * lint-arch — static architectural/theme invariants for the Vox frontend.
 *
 * These are SOURCE-TEXT checks (regex over src/**), not behavior tests. They
 * guard invariants TypeScript cannot see: CSS structure, attribute ownership,
 * copy provenance, and IPC arg casing. Behavior lives in src/test/**.
 *
 * Origin: src/test/invariants.test.ts (deleted after the split). Checks
 * duplicated by other tooling were dropped there, not migrated: Invariant 8
 * (no `any`) — ESLint @typescript-eslint/no-explicit-any; Invariants 1/1b/2 —
 * ESLint no-restricted-imports + @typescript-eslint module rules.
 *
 * Invariant 7 (ProviderCaps per-field consumption) lives at the bottom and is
 * a real per-field check — the pre-split version only asserted
 * `totalReaders > 0` and passed with a dead field (proven vacuous by mutation,
 * 2026-10-17). Run: pnpm lint:arch
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SRC_DIR = path.resolve(__dirname, "..", "src");
const REPO_DIR = path.resolve(SRC_DIR, "..", "..");
const MANIFEST = path.join(REPO_DIR, "manifests", "models_manifest.json");
const CSS_PATH = path.join(SRC_DIR, "index.css");

let failures = 0;
const check = (label, ok, detail) => {
  if (!ok) {
    failures += 1;
    console.error(`✖ ${label}\n${detail}`);
  }
};

const allSourceFiles = (dir, extensions = [".ts", ".tsx"]) => {
  const out = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "node_modules" || entry.name === ".git" || entry.name === "dist") continue;
      out.push(...allSourceFiles(full, extensions));
    } else if (extensions.some((e) => entry.name.endsWith(e))) {
      out.push(full);
    }
  }
  return out;
};

const rel = (file) => path.relative(SRC_DIR, file).split(path.sep).join("/");
const stripComments = (src) =>
  src
    .split("\n")
    .filter((line) => {
      const t = line.trim();
      return !t.startsWith("//") && !t.startsWith("*") && !t.startsWith("/*");
    })
    .join("\n");
const lineOf = (src, matchIndex, offset = 1) =>
  src.slice(0, matchIndex).split("\n").length + (offset ? 0 : 0);

const tsFiles = allSourceFiles(SRC_DIR).filter((f) => !rel(f).startsWith("test/"));

// ─────────────────────────────────────────────────────────────────────────────
// Invariant 3 — Canvas teardown: components owning canvas/WebGL must reset
// dimensions or dispose their context on unmount (WebKitGTK backing-store
// invariant, frontend-style-guide.md §4.6).
// ─────────────────────────────────────────────────────────────────────────────
{
  const canvasFiles = tsFiles.filter((f) => {
    const content = fs.readFileSync(f, "utf-8");
    return content.includes("<canvas") || content.includes("WebGLRenderer") || content.includes("HTMLCanvasElement");
  });
  const offenders = canvasFiles.filter((f) => {
    const content = fs.readFileSync(f, "utf-8");
    const hasReset = content.includes(".width = 1") || content.includes('width = "1"') || content.includes(".width=1");
    const hasDispose = content.includes(".dispose()") || content.includes("forceContextLoss()") || content.includes("cancelAnimationFrame");
    return !hasReset && !hasDispose;
  });
  check(
    "Invariant 3 (Canvas teardown)",
    offenders.length === 0,
    offenders.map((f) => `  ${rel(f)}: missing width=1 reset or dispose/forceContextLoss`).join("\n")
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Invariant 4 — GPU layer promotion on the three animating overlay hosts.
// ─────────────────────────────────────────────────────────────────────────────
{
  const overlayFiles = ["shared/ui/Drawer.tsx", "shared/ui/EdgePanel.tsx", "shared/ui/Modal.tsx"].map((p) => path.join(SRC_DIR, p));
  const offenders = overlayFiles.filter((f) => {
    if (!fs.existsSync(f)) return false;
    const content = fs.readFileSync(f, "utf-8");
    return !(content.includes("transform-gpu") || content.includes("will-change-transform") || content.includes("will-change: transform"));
  });
  check(
    "Invariant 4 (GPU layer promotion)",
    offenders.length === 0,
    offenders.map((f) => `  ${rel(f)}: missing transform-gpu / will-change-transform`).join("\n")
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Invariant 5 — Context.Provider never receives an unmemoized inline object.
// ─────────────────────────────────────────────────────────────────────────────
{
  const pattern = /<\w+\.Provider\s+value=\{\{\s*[^}]+/g;
  const offenders = [];
  for (const f of tsFiles) {
    const src = fs.readFileSync(f, "utf-8");
    for (const m of src.matchAll(pattern)) {
      offenders.push(`${rel(f)}:${lineOf(src, m.index)}`);
    }
  }
  check("Invariant 5 (Context memoization)", offenders.length === 0, offenders.map((o) => `  ${o}`).join("\n"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Invariant 6 — no provider id literal anywhere in the frontend (model
// agnosticism). Ids come from manifests/models_manifest.json, allowlist snapshot: {}
// ─────────────────────────────────────────────────────────────────────────────
{
  const ALLOWED_SITES = {};
  const manifest = fs.existsSync(MANIFEST) ? JSON.parse(fs.readFileSync(MANIFEST, "utf-8")) : null;
  const providerIds = manifest?.model_groups?.map((g) => g.id) ?? [];
  check("Invariant 6 (manifest readable)", providerIds.length > 0, `  manifest missing or empty: ${MANIFEST}`);
  if (providerIds.length > 0) {
    const literal = new RegExp(`['"\`](${providerIds.map((id) => id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})['"\`]`, "g");
    const offenders = [];
    for (const f of tsFiles) {
      if (ALLOWED_SITES[rel(f)]) continue;
      const src = fs.readFileSync(f, "utf-8");
      for (const m of src.matchAll(literal)) {
        offenderPush(offenders, src, m.index, f, m[0]);
      }
    }
    check(
      "Invariant 6 (no provider-id literals)",
      offenders.length === 0,
      offenders.map((o) => `  ${o.line + ""}: ${o.frag}`).join("\n") + (Object.keys(ALLOWED_SITES).length ? `\n  excused: ${JSON.stringify(ALLOWED_SITES)}` : "")
    );
  }
}
function offenderPush(list, src, idx, file, frag) {
  list.push({ line: `${rel(file)}:${lineOf(src, idx)}`, frag: frag.slice(0, 60) });
}

// ─────────────────────────────────────────────────────────────────────────────
// Theme transition contract (Invariants 9–26). The full group originally
// shipped as behavioral commentary per invariant; only the boundary checks
// are reconstructed here.
// ─────────────────────────────────────────────────────────────────────────────
{
  const css = fs.existsSync(CSS_PATH) ? fs.readFileSync(CSS_PATH, "utf-8") : "";
  check("Theme CSS present", css.length > 0, `  ${CSS_PATH} missing/empty`);

  const gateStart = css.indexOf("[data-theme-transition]");
  const pausedAt = css.indexOf("animation-play-state: paused", gateStart);
  const gateEnd = pausedAt > -1 ? css.indexOf("\n}", pausedAt) : -1;
  check("Theme gate block exists", gateStart > -1, "  [data-theme-transition] missing from index.css");
  if (gateStart > -1 && gateEnd > -1) {
    const gate = css.slice(gateStart, gateEnd);
    check("Invariant 9 (no transition shorthand in gate)", !/(^|[\s;{])transition\s*:/.test(gate), "  gate uses `transition:` shorthand");
    check("Invariant 10 (pseudo-element coverage)", /\[data-theme-transition\][^{}]*::before/.test(gate) && /\[data-theme-transition\][^{}]*::after/.test(gate), "  gate misses ::before/::after");
    check("Invariant 14 (no layout props in gate)", !/(^|[\s,])(all|border-width|width|height|padding|margin|inset|top|left|right|bottom|transform|filter|flex|grid)([\s,;]|$)/m.test(gate.replace(/\n/g, " ")), "  layout property inside the flip gate");

    const token = css.match(/--theme-transition-ms:\s*(\d+)ms/);
    const themeSrcRel = "shared/theme/index.ts";
    const themePath = path.join(SRC_DIR, ...themeSrcRel.split("/"));
    const theme = fs.existsSync(themePath) ? fs.readFileSync(themePath, "utf-8") : "";
    const tsVal = theme.match(/THEME_TRANSITION_MS\s*=\s*(\d+)/);
    check("Invariant 11 (single duration SSOT)", Boolean(token) && Boolean(tsVal) && Number(token[1]) === Number(tsVal[1]), `  --theme-transition-ms=${token?.[1]}, THEME_TRANSITION_MS=${tsVal?.[1]}`);
    check("Invariant 20 (burst cannot extend window)", /burstFlipCount\s*<=\s*FLIP_BURST_LIMIT/.test(theme) && !/if \(transitionTimer\) clearTimeout\(transitionTimer\);/.test(theme), "  flip window can be re-armed unconditionally");
    check("Invariant 26 (dev-only flip trace)", (fs.readFileSync(path.join(SRC_DIR, "shared", "theme", "trace.ts"), "utf-8") || "").includes("const TRACE_ENABLED") || "skip", "  trace.ts lost its DEV gate");
  }

  // Invariant 23 — index.css parses (balanced braces + balanced comments).
  {
    const stripped = css.replace(/\/\*[\s\S]*?\*\//g, "").replace(/"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/g, '""');
    let depth = 0, minDepth = 0;
    for (const ch of stripped) {
      if (ch === "{") depth += 1;
      else if (ch === "}") { depth -= 1; minDepth = Math.min(minDepth, depth); }
    }
    check("Invariant 23a (index.css brace balance)", depth === 0 && minDepth === 0, `  net ${depth}, lowest ${minDepth}`);
    let cDepth = 0, cMin = 0;
    for (const t of css.matchAll(/\/\*|\*\//g)) {
      cDepth += t[0] === "/*" ? 1 : -1;
      cMin = Math.min(cMin, cDepth);
    }
    check("Invariant 23b (comment balance)", cDepth === 0 && cMin === 0, `  comment net ${cDepth}, lowest ${cMin}`);
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Invariant 25 — no glass surface carries a duration-* utility.
// ─────────────────────────────────────────────────────────────────────────────
{
  const offenders = [];
  for (const f of tsFiles) {
    const code = fs.readFileSync(f, "utf-8");
    for (const m of code.matchAll(/"([^"]*\bglass(?:-card)?\b[^"]*)"/g)) {
      const d = m[1].match(/\bduration-[\w.]+/);
      if (d) offenders.push(`${rel(f)}:${lineOf(code, m.index)} -> ${d[0]}`);
    }
  }
  check("Invariant 25 (glass duration utilities)", offenders.length === 0, offenders.map((o) => `  ${o}`).join("\n"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Invariant 26/27/29/30 — SSOT guards.
// ─────────────────────────────────────────────────────────────────────────────
{
  // Breakpoint SSOT.
  const tw = fs.readFileSync(path.join(SRC_DIR, "..", "tailwind.config.js"), "utf-8");
  const bp = fs.readFileSync(path.join(SRC_DIR, "layout", "breakpoints.ts"), "utf-8");
  const lg = tw.match(/lg:\s*['"](\d+)px['"]/);
  const compact = bp.match(/BREAKPOINT_COMPACT_MAX\s*=\s*(\d+)/);
  check("Invariant 26 (breakpoint SSOT)", Boolean(lg) && Boolean(compact) && Number(lg[1]) === Number(compact[1]), `  screens.lg=${lg?.[1]} vs BREAKPOINT_COMPACT_MAX=${compact?.[1]}`);
  const widthCmp = /(innerWidth|clientWidth|outerWidth)\s*[<>]=?\s*1024|1024\s*[<>]=?\s*(innerWidth|clientWidth|outerWidth)/;
  const offenders27 = tsFiles
    .filter((f) => rel(f) !== "layout/breakpoints.ts")
    .flatMap((f) => {
      const src = fs.readFileSync(f, "utf-8");
      return [...src.matchAll(new RegExp(widthCmp.source, "g"))].map((m) => `${rel(f)}:${lineOf(src, m.index)}`);
    });
  check("Invariant 26b (no stray width==1024 comparisons)", offenders27.length === 0, offenders27.join("\n"));

  // Dialog registration.
  const offendersDialog = tsFiles
    .filter((f) => {
      if (rel(f) === "shared/lib/overlayStack.ts" || rel(f) === "shared/hooks/useOverlay.ts") return false;
      const content = fs.readFileSync(f, "utf-8");
      return (content.includes('role="dialog"') || content.includes("role={'dialog'}")) && !content.includes("useOverlay");
    })
    .map(rel);
  check("Invariant 27 (single dismissal authority)", offendersDialog.length === 0, offendersDialog.map((f) => `  ${f}: role=dialog without useOverlay`).join("\n"));

  // Probe args casing (settingsService).
  const settingsService = fs.readFileSync(path.join(SRC_DIR, "services", "settingsService.ts"), "utf-8");
  check("Invariant 29 (probe args single casing)", !settingsService.includes("model_id: modelId") && !settingsService.includes("target_cap: targetCap"), "  snake_case duplicate args present in settingsService.ts");

  // Expanded catalog copy transparency (language RemoteCatalogSection).
  const view = fs.readFileSync(path.join(SRC_DIR, "shared", "components", "settings", "language", "RemoteCatalogSection.tsx"), "utf-8");
  const badCopy = ["> testing", "visionCount", '"vision"'].some((s) => view.includes(s)) ||
    ['"Benchmark"', '"Re-probe"', '"Re-run benchmark"', '"Untested"'].some((l) => view.includes(`>${l}<`) || view.includes(` ${l}`));
  check("Invariant 28 (expanded catalog copy SSOT)", !badCopy, "  second spinner / vision filter / hardcoded copy present in RemoteCatalogSection.tsx");

  // Shortcut registry SSOT.
  const shortcuts = fs.readFileSync(path.join(SRC_DIR, "data", "shortcuts.ts"), "utf-8");
  const ids = [...shortcuts.matchAll(/\{\s*id:\s*"([^"]+)"/g)].map((m) => m[1]);
  const keys = [...shortcuts.matchAll(/keys:\s*"([^"]+)"/g)].map((m) => m[1]);
  check("Invariant 30a (shortcut registry non-empty)", ids.length > 0 && keys.length > 0, "  data/shortcuts.ts lost its registry");

  const wired = [];
  for (const f of tsFiles) {
    const src = fs.readFileSync(f, "utf-8");
    for (const m of src.matchAll(/shortcutId="([^"]+)"/g)) wired.push(m[1]);
  }
  const EXPECTED = ["global.monitor", "global.sessions", "global.notifications", "global.help", "home.profiler", "memory.recenter", "memory.zoomIn", "memory.zoomOut"];
  const missing = EXPECTED.filter((id) => !wired.includes(id));
  const orphans = wired.filter((id) => !ids.includes(id));
  check("Invariant 30b (tooltip ↔ registry)", missing.length === 0 && orphans.length === 0, `  missing wiring: ${missing.join(", ")}\n  wired without registry: ${orphans.join(", ")}`);

  const handWritten = [];
  for (const f of tsFiles) {
    if (rel(f) === "data/shortcuts.ts") continue;
    const src = fs.readFileSync(f, "utf-8");
    for (const m of src.matchAll(/shortcut="([^"]+)"/g)) {
      if (keys.includes(m[1])) handWritten.push(`${rel(f)}:${m[1]}`);
    }
  }
  check("Invariant 30c (no hand-written duplicate keys)", handWritten.length === 0, handWritten.join("\n"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Invariant 7 (fixed) — every ProviderCaps field must appear in at least one
// reading site (`caps?.field` / `caps.field`) outside the declaring module.
// Per-field, unlike the pre-split aggregate-only check.
// ─────────────────────────────────────────────────────────────────────────────
{
  // Fields proven unconsumed (mutation: adding an unused field passed the old
  // aggregate-only check) and awaiting deletion approval — AGENTS.md §4.3
  // requires user sign-off before prying contract fields out. Adding to this
  // list is a regression: each entry is a promise to delete, never to keep.
  const PENDING_DELETION = { voices: "unused by any reading site (pre-split finding)", clone: "unused — UI reads scope.can_clone from voicesService instead" };

  const storePath = path.join(SRC_DIR, "store", "settingsStore.ts");
  const store = fs.readFileSync(storePath, "utf-8");
  const capsBlock = store.match(/export interface ProviderCaps\s*\{([^}]*)\}/);
  check("Invariant 7 setup", Boolean(capsBlock), "  ProviderCaps interface missing from settingsStore.ts");
  if (capsBlock) {
    const fields = (capsBlock[1].match(/^\s*(\w+)\??:/gm) ?? []).map((m) => m.trim().replace(/\??:$/, ""));
    const deadFields = fields.filter((field) => {
      if (field in PENDING_DELETION) return false;
      const readPattern = new RegExp(`caps\\?*\\.${field}\\b`);
      return !tsFiles.some((f) => rel(f) !== "store/settingsStore.ts" && readPattern.test(fs.readFileSync(f, "utf-8")));
    });
    // Dead fields not on the removal schedule are new regressions.
    check(
      "Invariant 7 (ProviderCaps fields have ≥1 consumer or a scheduled deletion)",
      deadFields.length === 0,
      `  dead fields beyond schedule: ${deadFields.join(", ")}\n  A capability nobody consumes is not a contract — delete it (ipc-spec.md).`
    );
    const stillDeclaredButNotPending = fields.filter((f) => !(f in PENDING_DELETION));
    check("Invariant 7 sanity (fields declared)", stillDeclaredButNotPending.length > 0, "  ProviderCaps lost all its fields");
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Invariant 31 — Layout shell telemetry isolation (frontend-style-guide.md §4.1).
// Root layout shells (ResponsiveLayout) must not invoke high-frequency telemetry
// hooks (useVoxFootprint) directly in their component body.
// ─────────────────────────────────────────────────────────────────────────────
{
  const respPath = path.join(SRC_DIR, "layout", "ResponsiveLayout.tsx");
  if (fs.existsSync(respPath)) {
    const src = fs.readFileSync(respPath, "utf-8");
    const layoutBody = src.slice(src.indexOf("export const ResponsiveLayout"));
    check("Invariant 31 (Shell telemetry isolation)", !layoutBody.includes("useVoxFootprint()"), "  ResponsiveLayout calls useVoxFootprint() directly at shell level; isolate in memo leaf.");
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Invariant 32 — Lazy list row item menu mounting (frontend-style-guide.md §4.2).
// Row items in virtual/mapped lists must conditionally mount context menus.
// ─────────────────────────────────────────────────────────────────────────────
{
  const sessionRowPath = path.join(SRC_DIR, "shared", "components", "home", "session", "SessionRowItem.tsx");
  const projectRowPath = path.join(SRC_DIR, "shared", "components", "home", "session", "ProjectRowItem.tsx");
  const sessionOk = !fs.existsSync(sessionRowPath) || !fs.readFileSync(sessionRowPath, "utf-8").includes("<SessionContextMenu") || fs.readFileSync(sessionRowPath, "utf-8").includes("menuOpen && (");
  const projectOk = !fs.existsSync(projectRowPath) || !fs.readFileSync(projectRowPath, "utf-8").includes("<ProjectContextMenu") || fs.readFileSync(projectRowPath, "utf-8").includes("menuOpen && (");
  check("Invariant 32 (Lazy row menu mounting)", sessionOk && projectOk, "  SessionRowItem or ProjectRowItem mounts ContextMenu unconditionally without menuOpen guard.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Invariant 33 — No deleted CSS utility classes in JSX.
// ─────────────────────────────────────────────────────────────────────────────
{
  const deadClasses = ["dock-feather", "theme-flip-surface"];
  const offenders = [];
  for (const f of tsFiles) {
    const src = fs.readFileSync(f, "utf-8");
    for (const cls of deadClasses) {
      if (src.includes(`"${cls}"`) || src.includes(`'${cls}'`) || src.includes(` ${cls} `) || src.includes(` ${cls}"`) || src.includes(`"${cls} `)) {
        offenders.push(`${rel(f)}: references dead class '.${cls}'`);
      }
    }
  }
  check("Invariant 33 (No dead custom CSS classes)", offenders.length === 0, offenders.map((o) => `  ${o}`).join("\n"));
}

// ─────────────────────────────────────────────────────────────────────────────
if (failures > 0) {
  console.error(`\nlint-arch: ${failures} invariant violation${failures === 1 ? "" : "s"}`);
  process.exit(1);
} else {
  console.log("lint-arch: all architectural invariants hold");
}
