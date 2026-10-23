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
 * The model id a provider switch must write.
 *
 * A model id belongs to the provider that serves it, so it must never be
 * carried across a switch: the old code copied `active.model` into the new
 * selection, which left a foreign id behind and made the domain permanently
 * dirty once the user returned to the saved provider (`Apply & Restart` that
 * could not clear).
 *
 * Reverting to the committed category+provider restores the committed model, so
 * a revert is always byte-identical to the saved selection and the dirty flag
 * clears. Any other target uses the caller's fallback (the target's own known
 * model, or `""` when the catalog must supply it).
 */
export function resolveProviderModel(
  target: { category: ActiveCategory; provider: string | null },
  committed: ActiveSelection | null | undefined,
  fallback = ""
): string {
  if (
    committed &&
    committed.category === target.category &&
    (committed.provider ?? null) === (target.provider ?? null)
  ) {
    return committed.model;
  }
  return fallback;
}

/**
 * Resolves the effective API key for a cloud provider across draft and committed settings.
 * Checks draft.llm.cloud_keys, draft.providers.llm.cloud_keys, committed.providers.llm.cloud_keys,
 * and committed.llm.cloud_keys.
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

  return undefined;
}

/**
 * Resolves an effective realtime API key across draft and committed settings.
 * Checks realtime.api_keys and falls back to llm.cloud_keys for gemini_live.
 */
export function getEffectiveRealtimeApiKey(
  draft?: {
    realtime?: { api_keys?: Record<string, string> };
    providers?: {
      realtime?: { api_keys?: Record<string, string> };
      llm?: { cloud_keys?: Record<string, string> };
    };
    llm?: { cloud_keys?: Record<string, string> };
  } | null,
  committed?: {
    realtime?: { api_keys?: Record<string, string> };
    providers?: {
      realtime?: { api_keys?: Record<string, string> };
      llm?: { cloud_keys?: Record<string, string> };
    };
    llm?: { cloud_keys?: Record<string, string> };
  } | null,
  providerId?: string | null
): string | undefined {
  if (!providerId) return undefined;

  const lookup = (id: string) =>
    draft?.providers?.realtime?.api_keys?.[id] ??
    draft?.realtime?.api_keys?.[id] ??
    committed?.providers?.realtime?.api_keys?.[id] ??
    committed?.realtime?.api_keys?.[id] ??
    (id === "gemini_live"
      ? (draft?.providers?.llm?.cloud_keys?.["gemini"] ??
         draft?.llm?.cloud_keys?.["gemini"] ??
         committed?.providers?.llm?.cloud_keys?.["gemini"] ??
         committed?.llm?.cloud_keys?.["gemini"])
      : undefined);

  const key = lookup(providerId)?.trim();
  return key || undefined;
}

/**
 * Resolves the full dictionary of configured cloud API keys across draft and committed settings.
 * Precedence: committed.llm -> committed.providers.llm -> draft.providers.llm -> draft.llm.
 */
export function getEffectiveCloudKeyMap(
  draft?: {
    llm?: { cloud_keys?: Record<string, string> };
    providers?: { llm?: { cloud_keys?: Record<string, string> } };
  } | null,
  committed?: {
    llm?: { cloud_keys?: Record<string, string> };
    providers?: { llm?: { cloud_keys?: Record<string, string> } };
  } | null
): Record<string, string> {
  const map: Record<string, string> = {};
  const sources = [
    committed?.llm?.cloud_keys,
    committed?.providers?.llm?.cloud_keys,
    draft?.providers?.llm?.cloud_keys,
    draft?.llm?.cloud_keys,
  ];
  for (const src of sources) {
    if (!src) continue;
    for (const [k, v] of Object.entries(src)) {
      if (typeof v === "string" && v.trim().length > 0) {
        map[k] = v.trim();
      }
    }
  }
  return map;
}

/**
 * Resolves configured LLM custom servers across draft.llm.servers, draft.providers.llm.servers,
 * committed.providers.llm.servers, and committed.llm.servers.
 */
export function getEffectiveLlmServers(
  draft?: {
    llm?: { servers?: Array<{ id: string; name?: string; url: string; model?: string; protocol?: string; api_key?: string | null }> };
    providers?: { llm?: { servers?: Array<{ id: string; name?: string; url: string; model?: string; protocol?: string; api_key?: string | null }> } };
  } | null,
  committed?: {
    llm?: { servers?: Array<{ id: string; name?: string; url: string; model?: string; protocol?: string; api_key?: string | null }> };
    providers?: { llm?: { servers?: Array<{ id: string; name?: string; url: string; model?: string; protocol?: string; api_key?: string | null }> } };
  } | null
): Array<{ id: string; name?: string; url: string; model?: string; protocol?: string; api_key?: string | null }> {
  return (
    draft?.llm?.servers ??
    draft?.providers?.llm?.servers ??
    committed?.providers?.llm?.servers ??
    committed?.llm?.servers ??
    []
  );
}

/**
 * Active-card primacy: the committed/active entry is always rendered first.
 *
 * Semantics (shared by the model catalog, provider cloud/TTS grids, and the
 * custom-server list — `ModelModal.tsx`, `ProviderModal.tsx`): an already
 * *sorted* list, plus the id of the entry the user actively uses, moves that
 * entry to index 0 — only when present at index > 0. Absent id or an id
 * already first is a no-op. Only the active selection promotes; a *drafted*
 * (uncommitted) selection must never reorder until its caller passes the
 * committed id.
 *
 * Returns a new array; the input list is never mutated.
 */
export function promoteActive<T extends { id: string }>(
  list: readonly T[],
  activeId?: string | null
): T[] {
  const sorted = [...list];
  if (!activeId) return sorted;
  const idx = sorted.findIndex((entry) => entry.id === activeId);
  if (idx > 0) {
    const [active] = sorted.splice(idx, 1);
    sorted.unshift(active);
  }
  return sorted;
}