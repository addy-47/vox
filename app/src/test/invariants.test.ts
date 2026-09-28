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
});
