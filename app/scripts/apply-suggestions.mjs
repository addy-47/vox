/**
 * Applies ESLint *suggestions* (not fixes) to the settings surface.
 *
 * `no-unnecessary-condition` reports removable optional chains as suggestions,
 * which `eslint --fix` will not apply on its own. This script applies the
 * mechanical "remove unnecessary optional chain" suggestions only — anything
 * else is left for a human.
 *
 * Usage: node scripts/apply-suggestions.mjs [file ...]
 */
import { ESLint } from "eslint";
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const SRC = path.join(ROOT, "src");

const TARGETS = process.argv.slice(2);
if (TARGETS.length === 0) {
  console.error("usage: node scripts/apply-suggestions.mjs <file> [...]");
  process.exit(1);
}

const REMOVABLE_CHAIN = "Remove unnecessary optional chain";

/**
 * Files where removing optional chains breaks the React compiler's
 * memoization analysis: a `useMemo` dependency array that reads
 * `a?.b.c` and `a.b.c` are not equivalent to the compiler, and rewriting the
 * dependency invalidates the memo. These are fixed by hand instead.
 */
const SKIP = new Set(["src/shared/hooks/useProviderSelection.ts"]);

const eslint = new ESLint({
  cwd: ROOT,
  fix: true,
  overrideConfig: {
    rules: {
      "@typescript-eslint/no-unnecessary-condition": "warn",
    },
  },
});

let changed = 0;
for (const target of TARGETS) {
  const abs = path.isAbsolute(target) ? target : path.join(ROOT, target);
  if (!abs.startsWith(SRC)) {
    console.error(`skip (outside src): ${target}`);
    continue;
  }
  const rel = path.relative(ROOT, abs);
  if (SKIP.has(rel)) {
    console.log(`${rel}: skipped (memoization-sensitive)`);
    continue;
  }
  const source = fs.readFileSync(abs, "utf-8");
  const results = await eslint.lintFiles([abs]);
  const result = results[0];
  if (!result) continue;

  const messages = result.messages.filter(
    (m) => m.ruleId === "@typescript-eslint/no-unnecessary-condition"
  );
  const suggestions = messages.flatMap((m) => m.suggestions ?? []);
  const applicable = suggestions.filter((s) => s.desc === REMOVABLE_CHAIN);
  if (applicable.length === 0) {
    console.log(`${path.relative(ROOT, abs)}: nothing to apply`);
    continue;
  }

  // Suggestions carry a fix range; apply them back-to-front so earlier
  // offsets stay valid.
  const fixes = applicable
    .map((s) => s.fix)
    .filter((f) => f && typeof f.range?.[0] === "number")
    .sort((a, b) => b.range[0] - a.range[0]);

  let out = source;
  for (const fix of fixes) {
    out = out.slice(0, fix.range[0]) + fix.text + out.slice(fix.range[1]);
  }
  fs.writeFileSync(abs, out);
  changed += 1;
  console.log(`${path.relative(ROOT, abs)}: applied ${fixes.length} suggestion(s)`);
}

console.log(`\n${changed} file(s) changed.`);
