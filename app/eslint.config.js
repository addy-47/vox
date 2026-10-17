// Vox frontend ESLint flat config — ESLint 10 / typescript-eslint 8
// Enforces .agents/rules/frontend-style-guide.md.
//
// Rule layers (later entries override earlier ones in flat config):
//   1. js.configs.recommended        — baseline JS correctness
//   2. Parser + globals              — TS parser, browser globals
//   3. Disable superseded JS rules   — no-undef, no-unused-vars
//   4. Type-aware TS rules           — floating promises, unsafe any, etc.
//   5. Project-specific rules        — explicit any, unused vars, hooks, IPC
//   6. Per-file exceptions           — transport.ts IPC exemption
//
// Type-aware rules use `parserOptions.projectService: true` (typescript-eslint 8).
// This auto-discovers tsconfig.json without needing a separate lint tsconfig.
import js from "@eslint/js";
import tseslint from "@typescript-eslint/eslint-plugin";
import tsparser from "@typescript-eslint/parser";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";

// Browser globals for the renderer sources. Flat config has no implicit
// `browser` env: without this list every DOM/global access is a `no-undef`
// error (this config shipped without it and lint has been failing repo-wide).
const BROWSER_GLOBALS = Object.fromEntries(
  [
    "window", "document", "navigator", "console", "location", "history",
    "localStorage", "sessionStorage", "requestAnimationFrame",
    "cancelAnimationFrame", "requestIdleCallback", "cancelIdleCallback",
    "setTimeout", "clearTimeout", "setInterval", "clearInterval",
    "MutationObserver", "ResizeObserver", "IntersectionObserver",
    "CustomEvent", "Event", "Audio", "URL", "Blob", "Worker",
    "IntersectionObserverEntry", "getComputedStyle", "matchMedia",
    "structuredClone", "queueMicrotask", "crypto", "performance",
  ].map((name) => [name, "readonly"]),
);

export default [
  // ── 0. Global ignores ────────────────────────────────────────────────────
  { ignores: ["dist/**", "node_modules/**", "src/test/**", "app/src-tauri/**", "android/**"] },

  // ── 1. JS baseline ───────────────────────────────────────────────────────
  js.configs.recommended,

  // ── 2. Parser + globals for all TS/TSX ───────────────────────────────────
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      parser: tsparser,
      parserOptions: {
        ecmaVersion: 2022,
        sourceType: "module",
        // projectService: true enables type-aware rules without a separate
        // lint tsconfig. typescript-eslint 8 auto-resolves tsconfig.json.
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
      globals: { ...BROWSER_GLOBALS, React: "readonly" },
    },
    plugins: {
      "@typescript-eslint": tseslint,
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
  },

  // ── 3. Disable JS rules superseded by TypeScript ─────────────────────────
  // `no-undef` — TS already type-checks undefined globals; DOM type positions
  //   (HTMLDivElement, CSSNamespaceRule…) would be false positives.
  //   https://typescript-eslint.io/getting-started/#step-2-2
  // `no-unused-vars` — ts-eslint's version below handles types correctly.
  {
    files: ["src/**/*.{ts,tsx}"],
    rules: { "no-undef": "off", "no-unused-vars": "off" },
  },

  // ── 4. Type-aware TS rules (require projectService above) ─────────────────
  {
    files: ["src/**/*.{ts,tsx}"],
    rules: {
      // --- Errors: actionable defects ---

      // Floating promises: async functions called without await/catch silently
      // swallow errors (style-guide §4.3 async lifecycle).
      "@typescript-eslint/no-floating-promises": "error",

      // Misused promises: Promise passed where void is expected, e.g. in
      // useEffect callbacks or boolean conditions. checksVoidReturn.attributes
      // is false so JSX event handlers (onClick={asyncFn}) are allowed.
      "@typescript-eslint/no-misused-promises": [
        "error",
        { "checksVoidReturn": { "attributes": false } },
      ],

      // Unsafe any: any propagating through operations erodes type coverage.
      "@typescript-eslint/no-unsafe-argument": "error",
      "@typescript-eslint/no-unsafe-assignment": "error",
      "@typescript-eslint/no-unsafe-call": "error",
      "@typescript-eslint/no-unsafe-member-access": "error",
      "@typescript-eslint/no-unsafe-return": "error",

      // Awaiting non-thenables is always a bug.
      "@typescript-eslint/await-thenable": "error",

      // --- Warnings: valuable but occasionally noisy ---

      // Unnecessary conditions catch always-true/false guards that indicate
      // a logic bug (e.g. guarding against null TS proves impossible).
      "@typescript-eslint/no-unnecessary-condition": "warn",

      // Prefer ?. coalescing over || for nullable types.
      "@typescript-eslint/prefer-nullish-coalescing": "warn",

      // Async functions that never await are likely annotation mistakes.
      "@typescript-eslint/require-await": "warn",
    },
  },

  // ── 5. Project-specific rules ─────────────────────────────────────────────
  {
    files: ["src/**/*.{ts,tsx}"],
    rules: {
      // --- TypeScript (non-type-aware) ---

      // `any` is strictly prohibited (style-guide §2 Type Safety).
      // The type-aware no-unsafe-* rules above catch propagation too.
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { "argsIgnorePattern": "^_", "varsIgnorePattern": "^_" },
      ],

      // --- React Hooks (react-hooks v7 recommended-latest rule set) ---------

      // Core ordering invariant — must be error.
      "react-hooks/rules-of-hooks": "error",
      // Stale-closure bug detector. Pre-existing violations are real defects;
      // do NOT downgrade to warn to hide them.
      "react-hooks/exhaustive-deps": "error",

      // v7 extended rules:
      "react-hooks/static-components": "error",      // Non-hook fn wrapped in memo
      "react-hooks/use-memo": "error",               // useMemo must return a value
      "react-hooks/void-use-memo": "error",          // useMemo must NOT return void
      "react-hooks/preserve-manual-memoization": "error", // Don't remove author useMemo
      "react-hooks/immutability": "error",           // Don't mutate props/state
      "react-hooks/refs": "error",                   // Safe ref patterns
      "react-hooks/set-state-in-effect": "error",    // setState lifecycle safety
      "react-hooks/error-boundaries": "error",       // Correct error boundary setup
      "react-hooks/purity": "error",                 // Side-effects in render
      "react-hooks/set-state-in-render": "error",    // setState during render
      "react-hooks/globals": "error",                // Module-scope React globals
      "react-hooks/config": "error",                 // Hook configuration validity
      "react-hooks/gating": "error",                 // Gated hook usage

      // Warnings — noisy or environment-specific:
      "react-hooks/incompatible-library": "warn",    // Library version mismatch
      "react-hooks/unsupported-syntax": "warn",      // Non-standard syntax

      // --- Service boundary (frontend-style-guide.md §2, Invariant 1/1b) ---
      // Raw @tauri-apps invoke in view layers banned; use src/services/*.
      // fetch() is a global so cannot be blocked by no-restricted-imports;
      // it is enforced at architecture review + lint-arch level.
      "no-restricted-imports": ["error", {
        "patterns": [
          {
            "group": ["@tauri-apps/api/core", "@tauri-apps/api/tauri"],
            "message": "Raw IPC in a view layer. Use a src/services/* module (frontend-style-guide.md §2).",
          },
        ],
      }],

      // Vite HMR correctness.
      "react-refresh/only-export-components": ["warn", { "allowConstantExport": true }],
    },
  },

  // ── 6. Transport module: exempt from IPC import restriction ───────────────
  // 'src/services/transport.ts' is the single designated raw-IPC module
  // (frontend-style-guide.md §2; invariants Invariant 1b). Placed AFTER the
  // blanket ban above: flat-config later blocks override earlier ones.
  {
    files: ["src/services/transport.ts"],
    rules: {
      "no-restricted-imports": "off",
    },
  },
];
