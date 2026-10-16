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
 * embedded models and an inventory id otherwise: a `providers.*.servers` id
 * for `server`, a cloud provider id (`CLOUD_PROVIDERS[].id`) for `cloud`.
 * Display names must never be passed — the backend resolves by id and falls
 * back to a different endpoint when the id is unknown.
 */
export function activeSelection(
  category: ActiveCategory,
  model: string,
  provider: string | null = null
): ActiveSelection {
  return { category, provider, model };
}