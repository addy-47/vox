/**
 * Word-level difference generator for inline suggestion review (Google Docs style).
 * Splits text into words and whitespace/punctuation tokens, computes the LCS,
 * and emits a stream of tokens categorized as 'same', 'added', or 'removed'.
 */

export interface DiffToken {
  type: "same" | "added" | "removed";
  value: string;
}

/**
 * Tokenizes text into words, punctuation, and whitespace preserving all characters.
 */
function tokenize(text: string): string[] {
  if (!text) return [];
  // Matches words, whitespace sequences, or individual punctuation marks
  const tokens = text.match(/\s+|[^\s\w]+|\w+/g);
  return tokens ?? [text];
}

/**
 * Computes word-level diff between oldText and newText using LCS.
 */
export function diffWords(oldText: string, newText: string): DiffToken[] {
  if (oldText === newText) {
    return oldText ? [{ type: "same", value: oldText }] : [];
  }
  if (!oldText) {
    return newText ? [{ type: "added", value: newText }] : [];
  }
  if (!newText) {
    return oldText ? [{ type: "removed", value: oldText }] : [];
  }

  const a = tokenize(oldText);
  const b = tokenize(newText);
  const n = a.length;
  const m = b.length;

  // LCS DP table (lengths)
  // For memory safety with typical block sizes (1-500 words), this is fast and bounded.
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));

  for (let i = 0; i < n; i++) {
    for (let j = 0; j < m; j++) {
      if (a[i] === b[j]) {
        dp[i + 1][j + 1] = dp[i][j] + 1;
      } else {
        dp[i + 1][j + 1] = Math.max(dp[i + 1][j], dp[i][j + 1]);
      }
    }
  }

  // Backtrack to build diff tokens
  const rawDiff: DiffToken[] = [];
  let i = n;
  let j = m;

  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && a[i - 1] === b[j - 1]) {
      rawDiff.push({ type: "same", value: a[i - 1] });
      i--;
      j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      rawDiff.push({ type: "added", value: b[j - 1] });
      j--;
    } else if (i > 0 && (j === 0 || dp[i][j - 1] < dp[i - 1][j])) {
      rawDiff.push({ type: "removed", value: a[i - 1] });
      i--;
    }
  }

  rawDiff.reverse();

  // Merge contiguous tokens of the same type for cleaner rendering and performance
  const merged: DiffToken[] = [];
  for (const token of rawDiff) {
    if (merged.length > 0 && merged[merged.length - 1].type === token.type) {
      merged[merged.length - 1].value += token.value;
    } else {
      merged.push({ ...token });
    }
  }

  return merged;
}
