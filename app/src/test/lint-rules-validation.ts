/**
 * lint-rules-validation.ts
 *
 * Representative violations for the new ESLint rules added to eslint.config.js.
 * This file is INTENTIONALLY INVALID TypeScript/ESLint — each export
 * demonstrates one rule that must be caught.
 *
 * How to verify:
 *   cd app
 *   pnpm eslint src/test/lint-rules-validation.ts --no-ignore 2>&1
 *
 * Expected: each violation listed below produces at least one ESLint error/warning.
 * The file is excluded from `src/test/**` in the ignore list, so it must be
 * linted with --no-ignore to bypass that exclusion.
 *
 * DO NOT fix these violations. They prove the rules are active.
 */

// ── Rule: @typescript-eslint/no-floating-promises ─────────────────────────
// EXPECTED ERROR: Promise returned from async fn not handled.
async function fetchData(): Promise<string> {
  return "data";
}

export function floatingPromiseViolation(): void {
  fetchData(); // ← no-floating-promises: missing void / await / .catch
}

// ── Rule: @typescript-eslint/no-misused-promises ──────────────────────────
// EXPECTED ERROR: async callback in array.forEach (void return context).
export function misusedPromisesViolation(): void {
  const items = [1, 2, 3];
  items.forEach(async (_item) => {
    await fetchData(); // ← no-misused-promises: forEach expects void fn
  });
}

// ── Rule: @typescript-eslint/no-unsafe-assignment ─────────────────────────
// EXPECTED ERROR: variable typed as `any` assigned to typed variable.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function returnsAny(): any { return 42; }

export function unsafeAssignmentViolation(): void {
  const val: string = returnsAny(); // ← no-unsafe-assignment
  console.log(val);
}

// ── Rule: @typescript-eslint/no-unnecessary-condition ─────────────────────
// EXPECTED WARNING: TS knows string is always truthy.
export function unnecessaryConditionViolation(x: string): void {
  if (x) { // ← no-unnecessary-condition: string is always truthy
    console.log(x);
  }
}

// ── Rule: @typescript-eslint/require-await ────────────────────────────────
// EXPECTED WARNING: async function without await.
export async function requireAwaitViolation(): Promise<void> {
  console.log("no await here"); // ← require-await
}

// ── Rule: no-restricted-imports (IPC boundary) ────────────────────────────
// EXPECTED ERROR: raw IPC import outside transport.ts.
// import { invoke } from "@tauri-apps/api/core"; // ← un-comment to test
// The above line is commented because importing from a module that may not
// exist in this environment would cause a parse failure before lint runs.
// To verify this rule manually:
//   echo "import { invoke } from '@tauri-apps/api/core';" | pnpm eslint --stdin --stdin-filename src/components/Test.tsx

// ── Rule: react-hooks (validated in .tsx context only) ────────────────────
// These rules only apply to .tsx files processed by the React parser context.
// The hooks violations (conditional hook call, exhaustive-deps) are already
// proven by the 88-error baseline in VoiceCard.tsx, TitleBar.tsx etc.

export const _validationMarker = "lint-rules-validation";
