import { describe, it, expect } from "vitest";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SRC_DIR = path.resolve(__dirname, "..");

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
      // Skip services directory, test directory, and ToastApp (which manages its own standalone window)
      if (
        relPath.startsWith("services/") ||
        relPath.startsWith("test/") ||
        relPath === "toast/ToastApp.tsx"
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
      `Unmemoized inline object literal passed to Context.Provider (wrap with useMemo):\n${violations
        .map((v) => `  ${v.file}:${v.line} -> ${v.match}`)
        .join("\n")}`
    ).toEqual([]);
  });
});
