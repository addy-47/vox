#!/usr/bin/env node
/**
 * Fast AST / Static Architecture Invariant Checker
 * Runs in <300ms without needing browser or test runner.
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SRC_DIR = path.resolve(__dirname, "../../../../app/src");

function getAllFiles(dir, extensions) {
  let files = [];
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

console.log("=== VOX FRONTEND INVARIANT CHECKER ===");
const tsFiles = getAllFiles(SRC_DIR, [".ts", ".tsx"]);
let hasErrors = false;

// 1. Strict Service Boundary
const serviceViolations = [];
for (const file of tsFiles) {
  const relPath = path.relative(SRC_DIR, file);
  if (relPath.startsWith("services/") || relPath.startsWith("test/") || relPath === "toast/ToastApp.tsx") continue;
  const content = fs.readFileSync(file, "utf-8");
  const lines = content.split("\n");
  lines.forEach((line, idx) => {
    const trimmed = line.trim();
    if (trimmed.startsWith("//") || trimmed.startsWith("*")) return;
    if (line.includes('from "@tauri-apps/api/core"') || line.includes('from "@tauri-apps/api/event"')) {
      serviceViolations.push({ file: relPath, line: idx + 1, match: line.trim() });
    }
    if (/\binvoke\s*\(\s*["']/.test(line) && !line.includes("services")) {
      serviceViolations.push({ file: relPath, line: idx + 1, match: line.trim() });
    }
  });
}

if (serviceViolations.length > 0) {
  console.error("❌ INVARIANT 1 FAILED (Strict Service Boundary):");
  serviceViolations.forEach(v => console.error(`   ${v.file}:${v.line} -> ${v.match}`));
  hasErrors = true;
} else {
  console.log("✅ INVARIANT 1 PASSED: Strict Service Boundary (0 raw IPC calls outside src/services/)");
}

// 2. Zustand Store Destructuring
const storeViolations = [];
const storePattern = /const\s+\{[^}]+\}\s*=\s*use\w*Store\s*\(\s*\)/;
for (const file of tsFiles) {
  const relPath = path.relative(SRC_DIR, file);
  if (relPath.startsWith("store/") || relPath.startsWith("test/")) continue;
  const content = fs.readFileSync(file, "utf-8");
  const lines = content.split("\n");
  lines.forEach((line, idx) => {
    if (storePattern.test(line)) {
      storeViolations.push({ file: relPath, line: idx + 1, match: line.trim() });
    }
  });
}

if (storeViolations.length > 0) {
  console.error("❌ INVARIANT 2 FAILED (Zustand Selector Discipline):");
  storeViolations.forEach(v => console.error(`   ${v.file}:${v.line} -> ${v.match}`));
  hasErrors = true;
} else {
  console.log("✅ INVARIANT 2 PASSED: Zustand Discipline (All store hooks use atomic selectors)");
}

// 3. Canvas Teardown
const canvasViolations = [];
for (const file of tsFiles) {
  const relPath = path.relative(SRC_DIR, file);
  if (relPath.startsWith("test/")) continue;
  const content = fs.readFileSync(file, "utf-8");
  if (content.includes("<canvas") || content.includes("WebGLRenderer") || content.includes("HTMLCanvasElement")) {
    const hasReset = content.includes(".width = 1") || content.includes('width = "1"') || content.includes(".width=1");
    const hasDispose = content.includes(".dispose()") || content.includes("forceContextLoss()") || content.includes("cancelAnimationFrame");
    if (!hasReset && !hasDispose) {
      canvasViolations.push(relPath);
    }
  }
}

if (canvasViolations.length > 0) {
  console.error("❌ INVARIANT 3 FAILED (Canvas Teardown):");
  canvasViolations.forEach(f => console.error(`   ${f}: missing canvas width=1/height=1 or dispose on unmount`));
  hasErrors = true;
} else {
  console.log("✅ INVARIANT 3 PASSED: Canvas Teardown (All canvas/WebGL elements tear down on unmount)");
}

// 4. GPU Promotion on Overlays
const overlayViolations = [];
const overlayFiles = [
  path.join(SRC_DIR, "shared/ui/Drawer.tsx"),
  path.join(SRC_DIR, "shared/ui/EdgePanel.tsx"),
];
for (const file of overlayFiles) {
  if (!fs.existsSync(file)) continue;
  const relPath = path.relative(SRC_DIR, file);
  const content = fs.readFileSync(file, "utf-8");
  const hasGpu = content.includes("transform-gpu") || content.includes("will-change-transform") || content.includes("will-change: transform");
  if (!hasGpu) {
    overlayViolations.push(relPath);
  }
}

if (overlayViolations.length > 0) {
  console.error("❌ INVARIANT 4 FAILED (GPU Layer Promotion):");
  overlayViolations.forEach(f => console.error(`   ${f}: missing transform-gpu or will-change-transform`));
  hasErrors = true;
} else {
  console.log("✅ INVARIANT 4 PASSED: GPU Layer Promotion (All animating drawer overlays are GPU-promoted)");
}

// 5. Context Value Memoization
const contextViolations = [];
const inlineObjectPattern = /<\w+\.Provider\s+value=\{\{\s*[^}]+/;
for (const file of tsFiles) {
  const relPath = path.relative(SRC_DIR, file);
  if (relPath.startsWith("test/")) continue;
  const content = fs.readFileSync(file, "utf-8");
  const lines = content.split("\n");
  lines.forEach((line, idx) => {
    if (inlineObjectPattern.test(line)) {
      contextViolations.push({ file: relPath, line: idx + 1, match: line.trim() });
    }
  });
}

if (contextViolations.length > 0) {
  console.error("❌ INVARIANT 5 FAILED (Context Value Memoization):");
  contextViolations.forEach(v => console.error(`   ${v.file}:${v.line} -> ${v.match}`));
  hasErrors = true;
} else {
  console.log("✅ INVARIANT 5 PASSED: Context Memoization (No unmemoized inline object literals in Provider values)");
}

if (hasErrors) {
  process.exit(1);
} else {
  console.log("\nALL 5 FRONTEND INVARIANTS SATISFIED.");
  process.exit(0);
}
