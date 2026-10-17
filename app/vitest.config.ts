import { defineConfig } from "vitest/config";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@shared": path.resolve(__dirname, "./src/shared"),
      "@layout": path.resolve(__dirname, "./src/layout"),
      "@pages": path.resolve(__dirname, "./src/pages"),
    },
  },
  test: {
    include: ["src/test/**/*.test.ts", "src/test/**/*.test.tsx"],
    // jsdom is required by the render/dom-touching suites; node-environment
    // suites (store/service logic) run fine under it too, so one environment
    // keeps the run matrix simple.
    environment: "jsdom",
    globals: false,
    // No setup file: tests reset shared state explicitly (transport stub,
    // zustand setState, module-global maps) so no hidden global reset exists.
    coverage: {
      provider: "v8",
      include: ["src/**/*.ts", "src/**/*.tsx"],
      exclude: [
        "src/test/**",
        "src/main.tsx",
        "src/vite-env.d.ts",
        // Visual scaffolding (WebView shell) — not unit-testable in jsdom.
        "src/shared/components/**/canvas/**",
      ],
      reporter: ["text-summary", "lcov"],
      // Report-only: no thresholds. Per AGENTS.md §5 decisions, quality is
      // gated by the critical-seam test list, not a percentage.
    },
  },
});
