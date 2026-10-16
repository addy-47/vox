// Vox frontend ESLint flat config — ESLint 10
// Enforces .agents/rules/frontend-style-guide.md.
// Style-guide rules left as prose are re-stated here as errors so they can no
// longer be violated silently.
import js from "@eslint/js";
import tseslint from "@typescript-eslint/eslint-plugin";
import tsparser from "@typescript-eslint/parser";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";

export default [
  { ignores: ["dist/**", "node_modules/**", "src/test/**", "app/src-tauri/**", "android/**"] },
  js.configs.recommended,
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      parser: tsparser,
      parserOptions: { ecmaVersion: 2022, sourceType: "module" },
      globals: { React: "readonly", fetch: "readonly" },
    },
    plugins: {
      "@typescript-eslint": tseslint,
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "error",
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-unused-vars": ["error", { "argsIgnorePattern": "^_", "varsIgnorePattern": "^_" }],
      // §2 service boundary: raw invoke/fetch in view layers
      "no-restricted-imports": ["error", {
        "patterns": [
          {
            "group": ["@tauri-apps/api/core", "@tauri-apps/api/tauri"],
            "message": "Raw IPC in a view layer. Use a src/services/* module (frontend-style-guide.md §2)."
          }
        ]
      }],
    },
  },
  // Scripts-only layer (no React specifics for service files? Keep hooks here too)
];
