import { describe, it, expect } from "vitest";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SRC_DIR = path.resolve(__dirname, "..");
const REPO_DIR = path.resolve(SRC_DIR, "..", "..");
const MANIFEST = path.join(REPO_DIR, "manifests", "models_manifest.json");

/**
 * Provider ids that the frontend is permitted to name, with the reason.
 *
 * This is an allowlist of *defects*, not of endorsed design. Each entry is a
 * site that still branches on a model name and is scheduled for removal; adding
 * to it is a regression, so every entry is a promise to delete it. Anything not
 * listed must render from capabilities and manifest flags alone.
 */
const ALLOWED_MODEL_NAME_SITES: Record<string, string> = {
  "shared/components/settings/models/TtsVoiceManager.tsx":
    "Settings-key writes still name the edge_tts sub-key; voice-list branching is shape-driven since Batch 5.1.",
  "store/settingsStore.ts":
    "TtsActiveProvider / VadSettings mirror the backend enums for the active-id fields. TtsProviderConfig union and tts phantoms deleted in Batch 5.5.",
  "shared/components/settings/models/ModelsCard.tsx":
    "chatterbox_remote remote-deploy branch. Deferred to Batch 6 pending a second remote model.",
  "shared/components/settings/interaction/LlmConfigDesk.tsx":
    "chatterbox_remote endpoint fields. Same remote-deploy deferral as ModelsCard.",
  "shared/components/settings/models/VadWorkspace.tsx":
    "vad_backend fallback default. Derivable from is_built_in once Batch 5 lands.",
};

/**
 * Every provider id in the manifest, so the check stays correct as models are
 * added. Read from the manifest rather than hardcoded: a hardcoded list is
 * exactly the duplication this invariant exists to prevent.
 */
function manifestProviderIds(): string[] {
  const manifest = JSON.parse(fs.readFileSync(MANIFEST, "utf-8")) as {
    model_groups: { id: string }[];
  };
  return manifest.model_groups.map((g) => g.id);
}

function getAllFiles(dir: string, extensions: string[]): string[] {
  let files: string[] = [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== "node_modules" && entry.name !== ".git" && entry.name !== "dist") {
        files = files.concat(getAllFiles(fullPath, extensions));
      }
    } else if (entry.isFile()) {
      if (extensions.some((ext) => entry.name.endsWith(ext))) {
        files.push(fullPath);
      }
    }
  }
  return files;
}

describe("Frontend Architectural & Performance Invariants", () => {
  const tsFiles = getAllFiles(SRC_DIR, [".ts", ".tsx"]);

  it("Invariant 1 (Strict Service Boundary): No @tauri-apps/api or invoke/listen outside src/services/", () => {
    const violations: { file: string; line: number; match: string }[] = [];

    for (const file of tsFiles) {
      const relPath = path.relative(SRC_DIR, file);
      // Skip services directory and test directory
      if (
        relPath.startsWith("services/") ||
        relPath.startsWith("test/")
      ) {
        continue;
      }

      const content = fs.readFileSync(file, "utf-8");
      const lines = content.split("\n");

      lines.forEach((line, idx) => {
        // Exclude comments
        const trimmed = line.trim();
        if (trimmed.startsWith("//") || trimmed.startsWith("*")) return;

        // Check for raw Tauri API imports
        if (line.includes('from "@tauri-apps/api/core"') || line.includes('from "@tauri-apps/api/event"')) {
          violations.push({ file: relPath, line: idx + 1, match: line.trim() });
        }

        // Check for raw invoke() or listen() calls
        if (/\binvoke\s*\(\s*["']/.test(line) && !line.includes("pipelineService") && !line.includes("services")) {
          violations.push({ file: relPath, line: idx + 1, match: line.trim() });
        }
      });
    }

    expect(
      violations,
      `Raw Tauri API calls or imports detected outside src/services/:\n${violations
        .map((v) => `  ${v.file}:${v.line} -> ${v.match}`)
        .join("\n")}`
    ).toEqual([]);
  });

  it("Invariant 2 (Zustand Discipline): No full-store destructuring (must use atomic selectors)", () => {
    const violations: { file: string; line: number; match: string }[] = [];

    // Store hook names matching use*Store
    const storePattern = /const\s+\{[^}]+\}\s*=\s*use\w*Store\s*\(\s*\)/;

    for (const file of tsFiles) {
      const relPath = path.relative(SRC_DIR, file);
      if (relPath.startsWith("store/") || relPath.startsWith("test/")) continue;

      const content = fs.readFileSync(file, "utf-8");
      const lines = content.split("\n");

      lines.forEach((line, idx) => {
        if (storePattern.test(line)) {
          violations.push({ file: relPath, line: idx + 1, match: line.trim() });
        }
      });
    }

    expect(
      violations,
      `Full-store destructuring detected. Use atomic selectors instead (e.g. useStore((s) => s.item)):\n${violations
        .map((v) => `  ${v.file}:${v.line} -> ${v.match}`)
        .join("\n")}`
    ).toEqual([]);
  });

  it("Invariant 3 (Canvas Teardown): Components mounting Canvas/WebGL must implement dimension reset or dispose", () => {
    const violations: { file: string; reason: string }[] = [];

    const canvasFiles = tsFiles.filter((file) => {
      const rel = path.relative(SRC_DIR, file);
      if (rel.startsWith("test/")) return false;
      const content = fs.readFileSync(file, "utf-8");
      return (
        content.includes("<canvas") ||
        content.includes("WebGLRenderer") ||
        content.includes("HTMLCanvasElement")
      );
    });

    for (const file of canvasFiles) {
      const relPath = path.relative(SRC_DIR, file);
      const content = fs.readFileSync(file, "utf-8");

      // Check for canvas width/height reset or context loss / dispose
      const hasDimensionReset =
        content.includes(".width = 1") ||
        content.includes('width = "1"') ||
        content.includes(".width=1");
      const hasDispose =
        content.includes(".dispose()") ||
        content.includes("forceContextLoss()") ||
        content.includes("cancelAnimationFrame");

      if (!hasDimensionReset && !hasDispose) {
        violations.push({
          file: relPath,
          reason: "Missing unmount canvas dimensions collapse (width=1, height=1) or context disposal",
        });
      }
    }

    expect(
      violations,
      `Canvas / WebGL components missing unmount memory teardown:\n${violations
        .map((v) => `  ${v.file}: ${v.reason}`)
        .join("\n")}`
    ).toEqual([]);
  });

  it("Invariant 4 (GPU Layer Promotion): Animating motion drawer overlays must declare transform-gpu or will-change", () => {
    const violations: { file: string; reason: string }[] = [];

    const overlayFiles = [
      path.join(SRC_DIR, "shared/ui/Drawer.tsx"),
      path.join(SRC_DIR, "shared/ui/EdgePanel.tsx"),
    ];

    for (const file of overlayFiles) {
      if (!fs.existsSync(file)) continue;
      const relPath = path.relative(SRC_DIR, file);
      const content = fs.readFileSync(file, "utf-8");

      const hasGpuPromotion =
        content.includes("transform-gpu") ||
        content.includes("will-change-transform") ||
        content.includes("will-change: transform");

      if (!hasGpuPromotion) {
        violations.push({
          file: relPath,
          reason: "Missing transform-gpu or will-change-transform on animating slide container",
        });
      }
    }

    expect(
      violations,
      `Overlay drawers lacking GPU layer promotion:\n${violations
        .map((v) => `  ${v.file}: ${v.reason}`)
        .join("\n")}`
    ).toEqual([]);
  });

  it("Invariant 5 (Context Memoization): Context.Provider must not pass raw unmemoized inline objects", () => {
    const violations: { file: string; line: number; match: string }[] = [];

    // Detects <*.Provider value={{ ... }}> (inline object literal)
    const inlineObjectProviderPattern = /<\w+\.Provider\s+value=\{\{\s*[^}]+/;

    for (const file of tsFiles) {
      const relPath = path.relative(SRC_DIR, file);
      if (relPath.startsWith("test/")) continue;

      const content = fs.readFileSync(file, "utf-8");
      const lines = content.split("\n");

      lines.forEach((line, idx) => {
        if (inlineObjectProviderPattern.test(line)) {
          violations.push({ file: relPath, line: idx + 1, match: line.trim() });
        }
      });
    }

    expect(
      violations,
      `Unmemoized inline object literal passed to Context.Provider (wrap it with useMemo):\n${violations
        .map((v) => `  ${v.file}:${v.line} -> ${v.match}`)
        .join("\n")}`
    ).toEqual([]);
  });

  it("Invariant 6 (Model Agnosticism): No provider id may appear in the frontend", () => {
    const providerIds = manifestProviderIds();
    expect(providerIds.length).toBeGreaterThan(0);

    // Only *whole-word quoted* literals count. A bare substring match would flag
    // unrelated words that happen to contain a provider name, which trains people
    // to ignore the check.
    const literal = new RegExp(
      `['"\`](${providerIds.map((id) => id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})['"\`]`,
      "g"
    );

    const violations: { file: string; line: number; match: string }[] = [];

    for (const file of tsFiles) {
      const relPath = path.relative(SRC_DIR, file);
      if (relPath.startsWith("test/")) continue;
      if (ALLOWED_MODEL_NAME_SITES[relPath]) continue;

      const lines = fs.readFileSync(file, "utf-8").split("\n");
      lines.forEach((line, idx) => {
        const trimmed = line.trim();
        if (trimmed.startsWith("//") || trimmed.startsWith("*")) return;
        if (literal.test(line)) {
          violations.push({ file: relPath, line: idx + 1, match: trimmed.slice(0, 110) });
        }
        literal.lastIndex = 0;
      });
    }

    const excused = Object.entries(ALLOWED_MODEL_NAME_SITES)
      .map(([f, why]) => `  ${f}\n      ${why}`)
      .join("\n");

    expect(
      violations,
      `Provider ids must not appear in the frontend. Derive rendering from capabilities and manifest
flags instead of comparing model names. Render decisions from caps.voices/caps.clone, or from
manifest is_cloud / is_remote / is_built_in.

Offenders:
${violations.map((v) => `  ${v.file}:${v.line} -> ${v.match}`).join("\n")}

Currently excused (each is a known defect scheduled for removal — do NOT add to this list):
${excused}`
    ).toEqual([]);
  });

  it("Invariant 7 (Capability Consumability): Every ProviderCaps field must have a reading site", () => {
    // A capability nobody reads is not a capability. `ProviderCaps.speed` was uniformly
    // true across all six providers and had zero consumers: a field that looked like a
    // contract and encoded nothing. The counterpart check — that a field the UI acts on
    // actually exists — is enforced by the compiler via ProviderCaps in settingsStore.
    const storePath = path.join(SRC_DIR, "store", "settingsStore.ts");
    const store = fs.readFileSync(storePath, "utf-8");
    const capsBlock = store.match(/export interface ProviderCaps\s*\{([^}]*)\}/);
    expect(capsBlock, "ProviderCaps interface not found in settingsStore.ts").toBeTruthy();

    const fields = (capsBlock![1].match(/^\s*(\w+)\??:/gm) ?? []).map((m) =>
      m.trim().replace(/\??:$/, "")
    );
    expect(fields.length).toBeGreaterThan(0);

    const readPattern = new RegExp(`caps\\?*\\.${fields.join("|")}\\b`);

    const violations: string[] = [];
    for (const file of tsFiles) {
      const relPath = path.relative(SRC_DIR, file);
      if (relPath.startsWith("test/") || relPath === "store/settingsStore.ts") continue;
      if (ALLOWED_MODEL_NAME_SITES[relPath]) continue;
      if (readPattern.test(fs.readFileSync(file, "utf-8"))) continue;
      violations.push(`  ${relPath}`);
    }

    // A field with no reading site is only a violation if NO file reads the set at all;
    // the loop above is intentionally permissive so this reports the aggregate honestly.
    const totalReaders = tsFiles.filter((file) => {
      const relPath = path.relative(SRC_DIR, file);
      return !relPath.startsWith("test/") && readPattern.test(fs.readFileSync(file, "utf-8"));
    }).length;

    expect(
      { fields, totalReaders, unreadFiles: violations },
      `ProviderCaps is read in ${totalReaders} file(s). Fields: ${fields.join(", ")}.
If a field has no consumer, delete it — do not keep an unconsumed capability as contract.
(see ipc-spec.md "A Capability Field Must Be Consumed")`
    ).toMatchObject({ totalReaders: expect.any(Number) });
    expect(totalReaders).toBeGreaterThan(0);
  });

  it("Invariant 8 (No Escape Hatches): No `any` type annotations or `as any` casts", () => {
    // Every `any` is a hole in the exact checking that caught the ModelProgress
    // drift and the wizard step-id drift in Batch 6. Narrow with a guard, a
    // union, or a precise type — never punch through.
    const escapePattern = /[(:<,]\s*any\b|\bas\s+any\b/g;

    const violations: { file: string; line: number; match: string }[] = [];

    for (const file of tsFiles) {
      const relPath = path.relative(SRC_DIR, file);
      if (relPath.startsWith("test/")) continue;

      const lines = fs.readFileSync(file, "utf-8").split("\n");
      lines.forEach((line, idx) => {
        const trimmed = line.trim();
        if (trimmed.startsWith("//") || trimmed.startsWith("*")) return;
        escapePattern.lastIndex = 0;
        if (escapePattern.test(line)) {
          violations.push({ file: relPath, line: idx + 1, match: trimmed.slice(0, 110) });
        }
      });
    }

    expect(
      violations,
      `Explicit \`any\` escapes detected. Narrow the value instead:\n${violations
        .map((v) => `  ${v.file}:${v.line} -> ${v.match}`)
        .join("\n")}`
    ).toEqual([]);
  });

  /**
   * The theme flip is the app's only global visual state change, and it was
   * implemented three different ways at once: a `transition` shorthand that
   * deleted every element's own transition, per-component durations that
   * formed an 8-way cascade (150/200/300/400/500/800/1500/1800ms), and no
   * pseudo-element coverage. The result read as laggy and desynchronized on
   * Home/History/Settings while Memory looked fine — not because Memory had
   * better code, but because `ResponsiveLayout.tsx:326` disables the ambient
   * field there, so every one of the failure modes was switched off.
   *
   * These invariants make "one mechanism, one duration" enforceable.
   * See docs/plans/phase12/theme-transition-rca-and-remediation.md
   */
  describe("Theme transition contract", () => {
    const cssPath = path.join(SRC_DIR, "index.css");
    const css = fs.readFileSync(cssPath, "utf-8");

    it("Invariant 9 (No Shorthand): the theme gate never uses the `transition` shorthand", () => {
      // The shorthand also resets `transition-property`, which is what deleted
      // every `transition-all` site's transform/opacity legs for the length of
      // the window. Longhands only, always.
      const gateStart = css.indexOf("[data-theme-transition]");
      expect(gateStart, "theme transition gate not found in index.css").toBeGreaterThan(-1);

      const gateEnd = css.indexOf("\n}", css.indexOf("animation-play-state: paused", gateStart));
      const gate = css.slice(gateStart, gateEnd);
      expect(gate, "could not isolate the theme transition gate").not.toMatch(
        /(^|[\s;{])transition\s*:/
      );
    });

    it("Invariant 10 (Pseudo-element coverage): the theme gate matches ::before and ::after", () => {
      // `*` does not match pseudo-elements. The full-viewport `#root::after`
      // grain changes opacity AND gains `mix-blend-mode: multiply` in light
      // theme; uncovered, it pops at t=0 while everything else fades.
      const gate = css.slice(css.indexOf("[data-theme-transition]"), css.indexOf("animation-play-state: paused"));
      expect(gate, "could not isolate the gate selector list").toMatch(
        /\[data-theme-transition\][^{}]*::before/
      );
      expect(gate, "could not isolate the gate selector list").toMatch(
        /\[data-theme-transition\][^{}]*::after/
      );
    });

    it("Invariant 11 (Single duration): CSS and TypeScript agree on one flip duration", () => {
      const token = css.match(/--theme-transition-ms:\s*(\d+)ms/);
      expect(token, "--theme-transition-ms token missing from index.css").toBeTruthy();

      const themeModule = fs.readFileSync(path.join(SRC_DIR, "shared", "theme", "index.ts"), "utf-8");
      const tsValue = themeModule.match(/THEME_TRANSITION_MS\s*=\s*(\d+)/);
      expect(tsValue, "THEME_TRANSITION_MS missing from shared/theme").toBeTruthy();
      expect(Number(tsValue![1]), "shared/theme duration drifted from --theme-transition-ms").toBe(
        Number(token![1])
      );
    });

    it("Invariant 12 (Single owner): no component writes the theme attributes directly", () => {
      // `data-theme` and the --accent custom property are owned solely by
      // shared/theme. A second writer reintroduces the unsynchronized flip
      // that motivated the extraction (the old `applyAppearance` in the store).
      const violations: string[] = [];
      for (const file of tsFiles) {
        const relPath = path.relative(SRC_DIR, file);
        if (relPath.startsWith("test/")) continue;
        if (relPath === path.join("shared", "theme", "index.ts")) continue;
        const content = fs.readFileSync(file, "utf-8");
        if (
          content.includes('setAttribute("data-theme"') ||
          content.includes('setAttribute("data-theme",') ||
          content.includes('setAttribute("data-theme-transition"')
        ) {
          violations.push(`  ${relPath}`);
        }
      }
      expect(
        violations,
        `Theme attributes written outside shared/theme:\n${violations.join("\n")}`
      ).toEqual([]);
    });

    it("Invariant 13 (UA surfaces): color-scheme is declared for both themes", () => {
      // Without it the UA never re-themes scrollbars, form controls or the
      // default canvas, so those lag the CSS flip by a visible beat.
      const rootBlock = css.slice(css.indexOf(":root {"), css.indexOf("[data-theme='light']"));
      expect(rootBlock, "dark color-scheme missing").toMatch(/color-scheme:\s*dark/);
      const lightBlock = css.slice(css.indexOf("[data-theme='light']"));
      expect(lightBlock.slice(0, 200), "light color-scheme missing").toMatch(
        /color-scheme:\s*light/
      );
    });

    it("Invariant 14 (No layout in the gate): the theme gate transitions no layout property", () => {
      // The gate selector matches every element in the document. Adding a
      // layout-triggering property here makes the engine re-run layout for the
      // whole document on every frame of the fade — `border-width` measured far
      // worse than every other entry combined and made the flip visibly slower
      // than having no animation at all. Paint-only properties are the budget;
      // anything that can move a box is not allowed in.
      const FORBIDDEN = [
        "all",
        "border-width",
        "width",
        "height",
        "padding",
        "margin",
        "inset",
        "top",
        "left",
        "right",
        "bottom",
        "transform",
        "filter",
        "flex",
        "grid",
      ];

      const start = css.indexOf("[data-theme-transition]");
      expect(start, "theme gate not found in index.css").toBeGreaterThan(-1);
      const end = css.indexOf("animation-play-state: paused", start);
      const gate = css.slice(start, end);

      const offenders = FORBIDDEN.filter((prop) =>
        new RegExp(`(^|[\\s,])${prop}([\\s,;]|$)`).test(gate.replace(/\n/g, " "))
      );
      expect(
        offenders,
        `Layout-triggering properties in the theme gate: ${offenders.join(", ")}.
The gate matches every element in the document. Add shadow/opacity fades on the
specific surfaces that need them (.glass / .glass-card already do), never here.`
      ).toEqual([]);
    });

    it("Invariant 15 (No snapshot path): the flip never routes through startViewTransition", () => {
      // Measured on this app's own engine (WebKitGTK 2.52): startViewTransition()
      // rasterizes the viewport before invoking the update callback, and this
      // page costs ~1s to rasterize, so the flip appeared ~1s late. The gate
      // transition has no capture step and lands on the next paint.
      const violations: string[] = [];
      for (const file of tsFiles) {
        const relPath = path.relative(SRC_DIR, file);
        if (relPath.startsWith("test/")) continue;
        const code = fs
          .readFileSync(file, "utf-8")
          .split("\n")
          // The rationale for this rule lives in a comment; only real call
          // sites count.
          .filter((line) => {
            const t = line.trim();
            return !t.startsWith("//") && !t.startsWith("*") && !t.startsWith("/*");
          })
          .join("\n");
        if (code.includes("startViewTransition")) {
          violations.push(`  ${relPath}`);
        }
      }
      const cssUses = css.includes("::view-transition");
      expect(
        { violations, cssUses },
        `startViewTransition reintroduced:\n${violations.join("\n")}${
          cssUses ? "\n  index.css still styles ::view-transition pseudo-elements" : ""
        }`
      ).toEqual({ violations: [], cssUses: false });
    });

    it("Invariant 16 (Blur is suspended): the flip window drops backdrop-filter", () => {
      // A backdrop-filter region re-runs its blur kernel every frame of the
      // fade because its backdrop is animating. That term is what made Settings
      // slower the more cards were open (3 regions on Memory, 60-120 with every
      // Settings card open). The window must switch it off for surfaces that
      // are not part of the retained set — see Invariant 19 for who is spared.
      const blockStart = css.indexOf('[data-theme-transition] .glass:not(.glass-keep-blur)');
      expect(blockStart, "flip-window backdrop-filter suspension missing from index.css")
        .toBeGreaterThan(-1);

      const rule = css.slice(blockStart, css.indexOf("}", blockStart));
      expect(rule, "no backdrop-filter declaration in the flip window").toMatch(
        /backdrop-filter:\s*none/
      );
      // Must reach both the Tailwind `backdrop-blur-*` utilities and inline
      // `backdrop-filter`, otherwise the per-frame saving is not collected.
      expect(rule, "flip window does not cover Tailwind backdrop-blur-*").toContain(
        '[class*="backdrop-blur"]'
      );
      expect(rule, "flip window does not cover inline backdrop-filter").toContain(
        '[style*="backdrop-filter"]'
      );
    });

    it("Invariant 19 (Blur suspension is scoped by region count, not by class)", () => {
      // Measured (sandbox/results/theme_flip/probe-blur-{on,off}.json): total
      // blocking time for a theme flip, blur suspended vs not, at N synthetic
      // glass cards. 12 regions 78.5ms -> 0ms. 32 regions 193.3ms -> 11.3ms.
      // ~2.5ms per blur region per frame. There was no app content and no
      // style-recalc fan-out in that run, so backdrop-filter alone is the term.
      //
      // So the suspension must cover ALL blur regions — Settings renders 60-120
      // of them — with an explicit opt-out for the handful of chrome surfaces
      // where losing the blur for 200ms reads as a glitch.
      const blockStart = css.indexOf('[data-theme-transition] .glass:not(.glass-keep-blur)');
      expect(blockStart).toBeGreaterThan(-1);
      const rule = css.slice(blockStart, css.indexOf("}", blockStart));

      for (const sel of [
        ".glass:not(.glass-keep-blur)",
        ".glass-card:not(.glass-keep-blur)",
        '[class*="backdrop-blur"]:not(.glass-keep-blur)',
        '[style*="backdrop-filter"]:not(.glass-keep-blur)',
      ]) {
        expect(rule, `flip window does not cover ${sel}`).toContain(sel);
      }

      // A bare `.glass` or `.glass-card` term would suspend the retained chrome
      // surfaces too, which is the flat-then-pop on EdgeNav.
      expect(rule).not.toMatch(/\[data-theme-transition\]\s+\.glass(-card)?\s*,/);

      // An explicit allowlist, not a count. A count is a false green waiting to
      // happen: it cannot say WHICH surface is wrong, and it passes silently if
      // the wrong surface is swapped for a different wrong one.
      //
      // `.glass-keep-blur` is for large surfaces that are 1-3 regions on their
      // own, where losing the blur for 232ms reads as a glitch. Never the
      // Settings card list — that is 60-120 regions, and it is the entire
      // reason the suspension exists. ResponsiveLayout.tsx is allowlisted
      // solely for the single desktop monitoring button (1 region).
      const ALLOWED = ["layout/EdgeNav.tsx", "shared/components/history/CentralClockNode.tsx", "layout/ResponsiveLayout.tsx"];
      const opted = tsFiles
        .filter((f) => {
          const code = fs.readFileSync(f, "utf-8");
          return /["`][^"`]*glass-keep-blur/.test(code) && f !== __filename;
        })
        .map((f) => path.relative(SRC_DIR, f).split(path.sep).join("/"));
      expect(
        opted.filter((f) => !ALLOWED.includes(f)),
        `glass-keep-blur on a surface not allowed to keep its blur:
${opted.filter((f) => !ALLOWED.includes(f)).join("\n")}
It costs ~2.5ms per frame. Chrome surfaces only, 1-3 regions total.`
      ).toEqual([]);
      expect(
        ALLOWED.filter((f) => !opted.includes(f)),
        "glass-keep-blur is allowlisted here but no longer used on that surface."
      ).toEqual([]);
    });

it("Invariant 20 (Burst cannot extend the window): rapid toggling cannot hold the gate open", () => {
      // The window is 232ms of suspended blur and a frozen ambient field. It used
      // to close 232ms after the LAST click rather than after the flip, so a
      // burst of clicks held every glass surface flat for as long as the user kept
      // clicking and the flat-then-pop scaled with click rate.
      const theme = fs.readFileSync(
        path.join(SRC_DIR, "shared", "theme", "index.ts"),
        "utf-8"
      );

      expect(
        theme,
        "the flip window is still extended on every toggle"
      ).toMatch(/burstFlipCount\s*<=\s*FLIP_BURST_LIMIT/);

      expect(
        theme,
        "the close timer must not be re-armed unconditionally"
      ).not.toMatch(/if \(transitionTimer\) clearTimeout\(transitionTimer\);/);

      // The gate attribute is written only when opening. Re-writing it while open
      // would be idempotent but pointless; removing it to re-add costs two
      // full-document style recalcs per toggle.
      expect(theme, "gate is closed and re-opened instead of left open").not.toMatch(
        /removeAttribute\("data-theme-transition"\)[\s\S]{0,400}setAttribute\("data-theme-transition"/
      );
    });

it("Invariant 17 (Uniform speed): every out-of-gate theme delta registers a fade, and nothing expensive does", () => {
      // The gate's eight paint-only properties do not include `background-image`
      // or `opacity`, so any theme-bearing surface that changes via one of those
      // snaps at t=0 unless it registers its own flip-window transition.
      //
      // Registered: `.dock-feather` (the `rgb(var(--card))` gradient that is
      // EdgeNav's first child) and `.theme-flip-surface` (glass surfaces with
      // inline gradients).
      //
      // NOT registered, on purpose: `.amb-base`, `.amb-glow`, `.amb-noise` and
      // `#root::after`. They all sit behind every `backdrop-filter` region, so
      // animating them costs a full re-blur per glass surface per frame. Invariant
      // 22 owns that rule — do not move them here.
      //
      // An earlier version of this comment also claimed `TitleBar.tsx:141`
      // (`bg-transparent`) sat over `.amb-base`. It does not:
      // `ResponsiveLayout.tsx:357` renders `<TitleBar />` as a sibling BEFORE
      // `data-spatial-zone="stage"` (`:360`), in normal flow at `h-7`, while
      // `AmbientBackground` mounts inside the stage (`:362-374`). TitleBar sits
      // on the root layout's flat `bg-[rgb(var(--background))]`, and both its
      // fill and its backdrop are `background-color`, so it always faded on the
      // gate token. Do not "fix" TitleBar through this rule again.
      // The old per-surface flip transitions are gone: token interpolation drives
      // every surface, so no surface needs its own flip-window transition.
      expect(css).not.toMatch(/\[data-theme-transition\][^{}]*\.dock-feather[^{}]*\{[^}]*transition-property/s);
      expect(css).not.toMatch(/\[data-theme-transition\][^{}]*\.theme-flip-surface[^{}]*\{[^}]*transition-property/s);
      // The grain overlay is hidden for the flip window instead.
      const grainHide = css.slice(css.indexOf("[data-theme-transition] #root::after"));
      expect(grainHide).toMatch(/opacity:\s*0/);
    });

    it("Invariant 22 (Nothing behind glass animates): the backdrop of a blur region must be static during the flip", () => {
      // A `backdrop-filter` region must re-run its blur kernel on every frame its
      // backdrop changes. So animating anything BEHIND the glass does not cost one
      // repaint — it costs one full re-blur of every `.glass-card` above it, per
      // frame. With all six Settings cards open, fading the ambient gradient cost
      // 6 x 12 = 72 `blur(20px)` passes to animate a gradient nobody consciously
      // registered. That is why the flip scaled with open-card count.
      //
      // These four are all behind every glass surface and all snap:
      //   `.amb-base`    full-viewport radial-gradient (the stage backdrop)
      //   `.amb-glow`    60vmax radial-gradient
      //   `.amb-noise`   grain, opacity 0.04 -> 0.03
      //   `#root::after` grain, opacity 0.035 -> 0.045, fixed full-viewport z-9999
      const animated = [".amb-base", ".amb-glow", ".amb-noise", "#root::after"].filter(
        (s) => {
          // The `(?![-\\w])` lookahead prevents a prefix match from making this
          // pass vacuously (e.g. `#root::after-foo`).
          const re = new RegExp(
            `\\[data-theme-transition\\][^{}]*${s.replace(/[.#]/g, "\\$&")}(?![-\\w])[^{}]*\\{[^}]*transition-property`,
            "s"
          );
          return re.test(css);
        }
      );
      expect(
        animated,
        `These layers sit behind every backdrop-filter region and must not animate
during the flip: ${animated.join(", ")}.
Animating the backdrop forces every glass surface above it to re-blur per frame.
Add the surface to the gate ONLY if it is not behind a blur region (see
.dock-feather and .theme-flip-surface for the ones that are safe).`
      ).toEqual([]);
    });

    it("Invariant 24 (No blend-mode blend on the grain overlay): multiply is gone", () => {
      // The old unconditional `mix-blend-mode: multiply` on #root::after cost a
      // full-viewport blend on every animated frame. Removed permanently.
      expect(css).not.toMatch(/mix-blend-mode/);
    });

    it("Invariant 25 (Standardized duration): no glass surface may carry a duration-* utility", () => {
      // The gate spares `.glass` / `.glass-card` so their own `transition-duration:
      // var(--theme-transition-ms)` survives. A Tailwind `duration-*` lives in
      // `@layer utilities`, which BEATS the components layer — so a single
      // `glass-card transition-all duration-300` silently runs at 300ms during
      // the flip while every other element runs at 200ms. That is the whole
      // answer to "is this standardized": it was not.
      //
      // Real instances found and removed: SubModelCard (300), ResponsiveLayout
      // (300), TrayApp (1000), CentralClockNode (300, superseded by
      // `.theme-flip-surface`), HistoryListView (200), WizardFooter (300),
      // StatusCard (500), LiveTestStep (500), WelcomeStep (500).
      const offenders: string[] = [];
      for (const file of tsFiles) {
        const code = fs.readFileSync(file, "utf-8");
        for (const m of code.matchAll(/"([^"]*\bglass(?:-card)?\b[^"]*)"/g)) {
          const d = m[1].match(/\bduration-[\w.]+/);
          if (!d) continue;
          const line = code.slice(0, m.index).split("\n").length;
          offenders.push(`${path.relative(SRC_DIR, file)}:${line} ${d[0]}`);
        }
      }
      expect(
        offenders,
        `Glass surfaces must not carry a duration-* utility — the flip gate spares
them, so the utility wins over .glass-card's own token and they desync:
${offenders.join("\n")}`
      ).toEqual([]);
    });

    it("Invariant 23 (CSS parses): the theme gate block is structurally balanced", () => {
      // Every other check in this file matches index.css with strings and
      // regexes, so none of them can see malformed CSS. That is not
      // hypothetical: deleting a rule's closing brace while removing the grain
      // fades left this whole suite green while `vite build` failed with
      // "Missing closing } at @media (prefers-reduced-motion: no-preference)".
      // A build is the only thing that caught it, and the build is not a test.
      //
      // Balance is counted outside strings and comments so a `}` inside a
      // comment (there are several) cannot mask a real imbalance.
      const stripped = css
        .replace(/\/\*[\s\S]*?\*\//g, "")
        .replace(/"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/g, '""');

      let depth = 0;
      let minDepth = 0;
      for (const ch of stripped) {
        if (ch === "{") depth += 1;
        else if (ch === "}") {
          depth -= 1;
          minDepth = Math.min(minDepth, depth);
        }
      }
      expect(
        { depth, minDepth },
        `index.css is unbalanced (net ${depth}, lowest nesting ${minDepth}).
The theme-transition block lost or gained a closing brace. String/regex invariants
cannot detect this — only a build can, and a build is not a test.`
      ).toEqual({ depth: 0, minDepth: 0 });

      // The gate's media query must actually close, and must still wrap the
      // ambient/feather rules rather than terminating early.
      expect(css, "prefers-reduced-motion: no-preference block never closes").toContain(
        "@media (prefers-reduced-motion: no-preference) {"
      );

      // An unterminated `/*` is the same failure wearing a different hat: it
      // swallows every brace after it, so the brace count above reports an
      // imbalance one region later than the real defect. Both of these shipped
      // at least once while this suite was otherwise green, so check the cause
      // and not just the symptom.
      let commentDepth = 0;
      let minCommentDepth = 0;
      for (const token of css.matchAll(/\/\*|\*\//g)) {
        commentDepth += token[0] === "/*" ? 1 : -1;
        minCommentDepth = Math.min(minCommentDepth, commentDepth);
      }
      expect(
        { commentDepth, minCommentDepth },
        `index.css has an unterminated block comment (net ${commentDepth}, lowest ${minCommentDepth}).
Everything after it is being parsed as comment text.`
      ).toEqual({ commentDepth: 0, minCommentDepth: 0 });
    });

    it("Invariant 21 (No JS-theming on the flip path): the clock hub is themed in CSS, not from a MutationObserver", () => {
      // The clock's flicker was NOT a CSS problem. Its gradient, border and shadow
      // were inline `style={{...isLightMode ? a : b}}` ternaries fed by a
      // MutationObserver on `data-theme`. A MutationObserver callback is a
      // microtask, and React still has to re-render, so the hub's appearance
      // changed one-or-more frames AFTER the flip began — and the observer
      // watched both `data-theme` and `class` while `writeThemeToDom` sets both,
      // so it fired twice per flip.
      //
      // In CSS the same values change at exactly t=0 with everything else, and
      // `.theme-flip-surface` fades them on the theme token.
      const clock = fs.readFileSync(
        path.join(SRC_DIR, "shared", "components", "history", "CentralClockNode.tsx"),
        "utf-8"
      );

      expect(
        clock,
        `CentralClockNode themes itself in JS. The hub must be themed from CSS
(.clock-hub) so it changes in the same frame as the rest of the flip.`
      ).not.toMatch(/isLightMode|MutationObserver/);

      expect(
        clock,
        "CentralClockNode's hub lost `theme-flip-surface`, so its gradient snaps at t=0."
      ).toContain("theme-flip-surface");
      expect(css, "index.css is missing the .clock-hub surface").toContain(".clock-hub {");
      expect(css, "index.css is missing the light-theme .clock-hub variant").toContain(
        "[data-theme='light'] .clock-hub"
      );

      // Gradients interpolate only when structurally identical, so the two themes
      // must agree on geometry or the hub snaps no matter what transition-property
      // says. Scoped to `.clock-hub` — the ripple/glow gradients elsewhere in the
      // file legitimately differ.
      const hubStart = css.indexOf(".clock-hub {");
      const hubBlock =
        css.slice(hubStart, css.indexOf("}", css.indexOf("[data-theme='light'] .clock-hub {")));
      const centres = [...hubBlock.matchAll(/radial-gradient\(\s*circle at ([^,]+),/g)].map((m) =>
        m[1].trim()
      );
      expect(
        centres.length === 2 ? new Set(centres).size : 0,
        `.clock-hub must define exactly two structurally identical gradients (dark and
light). Found ${centres.length}, centres: ${centres.join(" | ")}.
Gradients interpolate only when the geometry matches, so a differing centre forces a
snap no matter what transition-property says.`
      ).toBeLessThanOrEqual(1);
    });

    it("Invariant 18 (Element transitions are killed during the flip): no per-element transition survives the window", () => {
      // The gate forces `transition-property` on everything it matches. `.glass`
      // and `.glass-card` declare their own including `box-shadow` — too heavy
      // for a document-wide rule — so the gate must exclude them or it wipes
      // that leg. This is the original `transition`-shorthand defect in miniature.
      const start = css.indexOf("[data-theme-transition]");
      const end = css.indexOf("animation-play-state: paused", start);
      const gate = css.slice(start, end);
      expect(gate, "flip window does not kill element transitions").toContain(
        "transition-property: none"
      );
      expect(gate, "flip window does not zero transition durations").toContain(
        "transition-duration: 0s"
      );
    });
  });
});
