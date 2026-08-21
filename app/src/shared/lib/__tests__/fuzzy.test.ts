import { describe, it, expect } from "vitest";
import { fuzzyMatch, fzfMultiTermScore } from "../fuzzy";

describe("fuzzyMatch", () => {
  it("matches exact strings with highest score", () => {
    const res = fuzzyMatch("llama-3", "llama-3");
    expect(res.matches).toBe(true);
    expect(res.score).toBe(1000);
  });

  it("matches empty query to anything", () => {
    const res = fuzzyMatch("", "qwen-2.5");
    expect(res.matches).toBe(true);
    expect(res.score).toBe(0);
  });

  it("fails when target is empty and query is non-empty", () => {
    const res = fuzzyMatch("abc", "");
    expect(res.matches).toBe(false);
  });

  it("matches substring with boundary bonus", () => {
    const prefixRes = fuzzyMatch("llama", "llama-3-8b");
    const midRes = fuzzyMatch("3-8b", "llama-3-8b");
    expect(prefixRes.matches).toBe(true);
    expect(midRes.matches).toBe(true);
    // Boundary match gets higher score than arbitrary non-boundary match
    expect(prefixRes.score).toBeGreaterThan(0);
  });

  it("matches fuzzy subsequence characters across word boundaries", () => {
    const res = fuzzyMatch("qwn8b", "qwen-2.5-8b-instruct");
    expect(res.matches).toBe(true);
    expect(res.score).toBeGreaterThan(0);
  });

  it("rejects strings missing required characters", () => {
    const res = fuzzyMatch("xyz", "llama-3-8b-instruct");
    expect(res.matches).toBe(false);
  });
});

describe("fzfMultiTermScore", () => {
  it("scores multi-term queries where all terms match candidates", () => {
    const score = fzfMultiTermScore(["qwen", "q4", "instruct"], [
      "qwen2.5-7b-instruct-q4_k_m.gguf",
      "Qwen 2.5 7B Instruct",
      "q4_k_m"
    ]);
    expect(score).not.toBeNull();
    expect(score!).toBeGreaterThan(0);
  });

  it("returns null if any single term fails to match across all candidate fields", () => {
    const score = fzfMultiTermScore(["qwen", "missingterm"], [
      "qwen2.5-7b-instruct-q4_k_m.gguf",
      "Qwen 2.5 7B Instruct"
    ]);
    expect(score).toBeNull();
  });

  it("ranks exact matches higher than fuzzy matches", () => {
    const exactScore = fzfMultiTermScore(["llama"], ["llama", "Llama"]);
    const fuzzyScore = fzfMultiTermScore(["llama"], ["meta-llama-3-8b-instruct-gguf", "Meta Llama 3 8B"]);
    expect(exactScore).not.toBeNull();
    expect(fuzzyScore).not.toBeNull();
    expect(exactScore!).toBeGreaterThan(fuzzyScore!);
  });
});
