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

/**
 * Returns the alias provider id if one exists (e.g. nvidia <-> nvidia_nim).
 */
export function getCloudProviderAlias(providerId?: string | null): string | null {
  if (!providerId) return null;
  if (providerId === "nvidia") return "nvidia_nim";
  if (providerId === "nvidia_nim") return "nvidia";
  return null;
}

/**
 * Resolves the effective API key for a cloud provider across draft and committed settings.
 * Checks draft.llm.cloud_keys, draft.providers.llm.cloud_keys, committed.providers.llm.cloud_keys,
 * and committed.llm.cloud_keys, with bidirectional alias fallback (e.g. nvidia <-> nvidia_nim).
 */
export function getEffectiveCloudApiKey(
  draft?: {
    llm?: { cloud_keys?: Record<string, string> };
    providers?: { llm?: { cloud_keys?: Record<string, string> } };
  } | null,
  committed?: {
    llm?: { cloud_keys?: Record<string, string> };
    providers?: { llm?: { cloud_keys?: Record<string, string> } };
  } | null,
  providerId?: string | null
): string | undefined {
  if (!providerId) return undefined;

  const lookup = (id: string) =>
    draft?.llm?.cloud_keys?.[id] ??
    draft?.providers?.llm?.cloud_keys?.[id] ??
    committed?.providers?.llm?.cloud_keys?.[id] ??
    committed?.llm?.cloud_keys?.[id];

  const primary = lookup(providerId)?.trim();
  if (primary) return primary;

  const alias = getCloudProviderAlias(providerId);
  if (alias) {
    const aliasKey = lookup(alias)?.trim();
    if (aliasKey) return aliasKey;
  }

  return undefined;
}