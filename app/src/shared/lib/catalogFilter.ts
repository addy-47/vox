import type { LlmModelInfo } from "@/store/settingsStore";
import type { UnifiedVoiceDto } from "@/services/voicesService";

export interface ProviderFilterItem {
  id: string;
  name: string;
  tagline?: string | null;
  url?: string | null;
  isFree?: boolean;
  category?: "local" | "cloud" | "server" | string;
}

export interface FilterProvidersOptions {
  getProviderKey?: (id: string) => string | undefined;
  isTts?: boolean;
}

/**
 * Pure predicate filtering for catalog providers.
 */
export function filterProviders<T extends ProviderFilterItem>(
  providers: T[],
  filterId: string = "all",
  searchQuery: string = "",
  options?: FilterProvidersOptions
): T[] {
  let list = providers;
  const q = searchQuery.trim().toLowerCase();

  if (q) {
    list = list.filter((p) => {
      const nameMatch = p.name ? p.name.toLowerCase().includes(q) : false;
      const idMatch = p.id ? p.id.toLowerCase().includes(q) : false;
      const taglineMatch = p.tagline ? p.tagline.toLowerCase().includes(q) : false;
      const urlMatch = p.url ? p.url.toLowerCase().includes(q) : false;
      return nameMatch || idMatch || taglineMatch || urlMatch;
    });
  }

  if (filterId === "local") {
    return list.filter((p) => p.category === "local");
  }

  if (filterId === "cloud") {
    return list.filter((p) => p.category === "cloud" || (!p.category && p.category !== "local" && p.category !== "server"));
  }

  if (filterId === "configured") {
    return list.filter((p) => {
      if (p.isFree || p.category === "local" || p.category === "server") return true;
      const key = options?.getProviderKey ? options.getProviderKey(p.id) : undefined;
      return Boolean(key?.trim());
    });
  }

  if (filterId === "unconfigured") {
    return list.filter((p) => {
      if (p.isFree || p.category === "local" || p.category === "server") return false;
      const key = options?.getProviderKey ? options.getProviderKey(p.id) : undefined;
      return !key?.trim();
    });
  }

  return list;
}

export interface ModelFilterState {
  category?: "all" | "favorites" | "benchmarked" | "tools" | "fast" | string;
  favoriteIds?: Set<string> | string[];
  resolveProbed?: (m: LlmModelInfo) => { tps?: number | null; supports_tools?: string | null } | undefined | null;
}

/**
 * Pure predicate filtering for LLM models.
 */
export function filterModels<T extends LlmModelInfo = LlmModelInfo>(
  models: T[],
  filterState: ModelFilterState | string = "all",
  searchQuery: string = ""
): T[] {
  let list = models;
  const q = searchQuery.trim().toLowerCase();

  if (q) {
    list = list.filter((m) => {
      const idMatch = m.id.toLowerCase().includes(q);
      const nameMatch = m.name ? m.name.toLowerCase().includes(q) : false;
      const familyMatch = m.family ? m.family.toLowerCase().includes(q) : false;
      const quantMatch = m.quantization ? m.quantization.toLowerCase().includes(q) : false;
      return idMatch || nameMatch || familyMatch || quantMatch;
    });
  }

  const state: ModelFilterState = typeof filterState === "string" ? { category: filterState } : filterState;
  const category = state.category ?? "all";

  if (category === "favorites") {
    const favSet = state.favoriteIds instanceof Set ? state.favoriteIds : new Set(state.favoriteIds || []);
    return list.filter((m) => favSet.has(m.id));
  }

  if (category === "benchmarked") {
    return list.filter((m) => {
      const caps = state.resolveProbed ? state.resolveProbed(m) : m.capabilities;
      return (caps?.tps ?? 0) > 0;
    });
  }

  if (category === "tools") {
    return list.filter((m) => {
      const caps = state.resolveProbed ? state.resolveProbed(m) : m.capabilities;
      return caps?.supports_tools === "supported";
    });
  }

  if (category === "fast") {
    return list.filter((m) => {
      const caps = state.resolveProbed ? state.resolveProbed(m) : m.capabilities;
      return (caps?.tps ?? 0) >= 30;
    });
  }

  return list;
}

export interface VoiceFilterState {
  scope?: "all" | "recent" | "favorites" | "custom" | string;
  gender?: string | null;
  accent?: "english" | "multilingual" | null;
  providerId?: string | null;
}

export function isVoiceEnglish(v: { locale: string }): boolean {
  return v.locale.toLowerCase().startsWith("en");
}

/**
 * Pure predicate filtering for unified voices.
 */
export function filterVoices<T extends UnifiedVoiceDto = UnifiedVoiceDto>(
  voices: T[],
  filterState: VoiceFilterState | string = "all",
  searchQuery: string = ""
): T[] {
  let list = voices;
  const state: VoiceFilterState = typeof filterState === "string" ? { scope: filterState } : filterState;

  if (state.providerId && state.providerId.trim()) {
    const norm = state.providerId.trim().toLowerCase();
    list = list.filter((v) => v.provider_id.toLowerCase() === norm);
  }

  const scope = state.scope ?? "all";
  if (scope === "recent") {
    list = list.filter((v) => v.last_used_at != null);
  } else if (scope === "favorites") {
    list = list.filter((v) => Boolean(v.is_favorite));
  } else if (scope === "custom") {
    list = list.filter((v) => v.source_kind === "custom");
  }

  if (state.gender) {
    list = list.filter((v) => v.gender === state.gender);
  }

  if (state.accent === "english") {
    list = list.filter(isVoiceEnglish);
  } else if (state.accent === "multilingual") {
    list = list.filter((v) => !isVoiceEnglish(v));
  }

  const q = searchQuery.trim().toLowerCase();
  if (q) {
    list = list.filter((v) => {
      const nameMatch = v.name ? v.name.toLowerCase().includes(q) : false;
      const descMatch = v.description ? v.description.toLowerCase().includes(q) : false;
      const localeMatch = v.locale ? v.locale.toLowerCase().includes(q) : false;
      const accentMatch = v.accent ? v.accent.toLowerCase().includes(q) : false;
      const tagsMatch = v.tags ? v.tags.some((t) => t.toLowerCase().includes(q)) : false;
      return nameMatch || descMatch || localeMatch || accentMatch || tagsMatch;
    });
  }

  return list;
}
