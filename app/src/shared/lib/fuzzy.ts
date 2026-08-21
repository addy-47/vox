/**
 * fzf-style fuzzy subsequence matching and scoring algorithm.
 * Evaluates pattern character subsequence with bonuses for:
 * - Word boundary matches (following `/`, `-`, `_`, `.`, space)
 * - Exact consecutive character matches
 * - Exact substring matches
 * - Match compactness (penalizing long gaps)
 */

export interface FuzzyMatchResult {
  matches: boolean;
  score: number;
}

export function fuzzyMatch(pattern: string, target: string): FuzzyMatchResult {
  if (!pattern) return { matches: true, score: 0 };
  if (!target) return { matches: false, score: 0 };

  const pLower = pattern.toLowerCase();
  const tLower = target.toLowerCase();

  // Fast-path exact match
  if (tLower === pLower) {
    return { matches: true, score: 1000 };
  }

  // Fast-path substring match
  const subIdx = tLower.indexOf(pLower);
  if (subIdx !== -1) {
    const isBoundary = subIdx === 0 || /[\s\-_/.]/.test(tLower[subIdx - 1]);
    const score = 500 + (isBoundary ? 150 : 0) - subIdx;
    return { matches: true, score };
  }

  let pIdx = 0;
  let tIdx = 0;
  let score = 0;
  let consecutiveCount = 0;
  let prevMatchIdx = -1;

  while (pIdx < pLower.length && tIdx < tLower.length) {
    const pChar = pLower[pIdx];
    const tChar = tLower[tIdx];

    if (pChar === tChar) {
      let charScore = 10;

      // Word boundary bonus: match right after delimiter or at string start
      const isStartOfWord =
        tIdx === 0 ||
        /[\s\-_/.]/.test(target[tIdx - 1]) ||
        (target[tIdx] >= "A" && target[tIdx] <= "Z" && target[tIdx - 1] >= "a" && target[tIdx - 1] <= "z");

      if (isStartOfWord) {
        charScore += 30;
      }

      // Consecutive character bonus
      if (prevMatchIdx === tIdx - 1) {
        consecutiveCount++;
        charScore += consecutiveCount * 15;
      } else {
        consecutiveCount = 0;
        if (prevMatchIdx !== -1) {
          const gap = tIdx - prevMatchIdx - 1;
          charScore -= Math.min(gap * 2, 20);
        }
      }

      prevMatchIdx = tIdx;
      score += charScore;
      pIdx++;
    }

    tIdx++;
  }

  if (pIdx === pLower.length) {
    // Length penalty: favor tighter, more specific matches
    score -= target.length - pattern.length;
    return { matches: true, score };
  }

  return { matches: false, score: 0 };
}

/**
 * Multi-term fzf search across candidate string fields.
 * Every term in `terms` must match at least one candidate field.
 * Returns total composite score, or null if any term failed to match.
 */
export function fzfMultiTermScore(terms: string[], candidateStrings: string[]): number | null {
  const validCandidates = candidateStrings.filter(Boolean);
  let totalScore = 0;

  for (const term of terms) {
    let bestTermScore = -Infinity;
    let termMatched = false;

    for (const str of validCandidates) {
      const { matches, score } = fuzzyMatch(term, str);
      if (matches) {
        termMatched = true;
        if (score > bestTermScore) {
          bestTermScore = score;
        }
      }
    }

    if (!termMatched) {
      return null;
    }

    totalScore += bestTermScore;
  }

  return totalScore;
}
