import type { ActiveCategory, ActiveSelection } from "@/store/settingsStore";

/** Reads the runtime tier of an active selection, defaulting to `embedded`. */
export function activeCategory(selection?: ActiveSelection | null): ActiveCategory {
  return selection?.category ?? "embedded";
}

/** Reads the concrete model id of an active selection. */
export function activeModelId(selection?: ActiveSelection | null): string {
  return selection?.model ?? "";
}

/**
 * Builds an `ActiveSelection` for a write. `provider` must be null for plain
 * embedded models and a provider name / server id otherwise.
 */
export function activeSelection(
  category: ActiveCategory,
  model: string,
  provider: string | null = null
): ActiveSelection {
  return { category, provider, model };
}