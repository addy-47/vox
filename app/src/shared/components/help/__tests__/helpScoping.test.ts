import { describe, it, expect } from "vitest";
import { HELP_ARTICLES } from "@/data/helpCopy";
import { scopeArticles } from "../HelpContent";

describe("helpScoping", () => {
  it("shows only the exact article for a page deepLink", () => {
    const { scoped, rest } = scopeArticles(HELP_ARTICLES, "page:home");
    expect(scoped.map((a) => a.id)).toEqual(["page:home"]);
    expect(rest.length).toBe(HELP_ARTICLES.length - 1);
    expect(rest.some((a) => a.id === "page:home")).toBe(false);
  });

  it("falls back to the settings group for domain links without articles", () => {
    const { scoped, rest } = scopeArticles(HELP_ARTICLES, "settings:realtime");
    expect(scoped.length).toBeGreaterThan(0);
    expect(scoped.every((a) => a.group === "settings")).toBe(true);
    expect(rest.every((a) => a.group !== "settings")).toBe(true);
  });

  it("shows everything when opened without context", () => {
    const { scoped, rest } = scopeArticles(HELP_ARTICLES, null);
    expect(scoped.length).toBe(HELP_ARTICLES.length);
    expect(rest).toEqual([]);
  });

  it("every page deepLink used by the top-right cluster resolves", () => {
    for (const id of ["page:home", "page:history", "page:memory", "page:monitoring"]) {
      const { scoped } = scopeArticles(HELP_ARTICLES, id);
      expect(scoped.map((a) => a.id)).toEqual([id]);
    }
    const { scoped } = scopeArticles(HELP_ARTICLES, "settings:overview");
    expect(scoped.map((a) => a.id)).toEqual(["settings:overview"]);
  });
});
