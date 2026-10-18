
export interface CloudProvider {
  id: string;
  name: string;
  url: string;
  keyPlaceholder: string;
  popular?: boolean;
  tagline?: string;
  logo?: string;
  isFree?: boolean;
  isConfigured?: boolean;
}

export interface CloudProviderContractEntry {
  id: string;
  name: string;
  base_url?: string;
  url?: string;
  key_placeholder?: string;
  keyPlaceholder?: string;
  popular?: boolean;
  tagline?: string;
  logo?: string;
}

export const REALTIME_PROVIDERS = [] as const;

export interface TtsProviderEntry {
  id: string;
  name: string;
  category: "local" | "cloud" | "server";
  tagline: string;
  url?: string;
  logo?: string;
  isFree?: boolean;
  isConfigured?: boolean;
  isComingSoon?: boolean;
  statusLabel?: string;
}


export interface TtsProviderContractEntry {
  id: string;
  name: string;
  category: string;
  subcategory?: string;
  description?: string;
  tagline?: string;
  url?: string;
  logo?: string;
  is_cloud?: boolean;
  is_remote?: boolean;
  is_configured?: boolean;
  isConfigured?: boolean;
  is_free?: boolean;
  isFree?: boolean;
  status_label?: string;
  statusLabel?: string;
}

/**
 * Pure mapper: backend contract → cloud provider list (store-owned state).
 */
export function mapCloudProvidersFromContract(
  providers: CloudProviderContractEntry[] | undefined | null
): CloudProvider[] {
  if (!providers || providers.length === 0) return [];
  return providers.map((p) => ({
    id: p.id,
    name: p.name,
    url: p.base_url || p.url || "",
    keyPlaceholder: p.key_placeholder || p.keyPlaceholder || "sk-...",
    popular: p.popular,
    tagline: p.tagline,
    logo: p.logo,
  }));
}

/**
 * Pure mapper: backend contract → TTS provider list (store-owned state).
 */
export function mapTtsProvidersFromContract(
  providers: TtsProviderContractEntry[] | undefined | null
): TtsProviderEntry[] {
  if (!providers || providers.length === 0) return [];
  const filtered = providers.filter(
    (t) => t.is_cloud || t.is_remote || t.subcategory === "cloud" || t.subcategory === "remote"
  );
  if (filtered.length === 0) return [];
  const nameMap: Record<string, string> = {
    google_tts: "Google Cloud TTS",
    azure_tts: "Azure AI Speech",
    edge_tts: "Microsoft Edge TTS",
    chatterbox_remote: "Chatterbox Remote (HTTP)",
  };
  return filtered.map((p) => ({
    id: p.id,
    name: nameMap[p.id] || p.name,
    category: p.is_remote ? "server" : "cloud",
    tagline: p.description || p.tagline || "",
    url: p.url,
    logo: p.logo,
    // backend-gap: `is_free` / `status_label` should arrive canonically for
    // edge_tts; the `includes("edge")` fallback is a frontend stand-in.
    isFree: p.is_free ?? p.id.includes("edge"),
    isConfigured: Boolean(p.is_configured ?? p.isConfigured),
    statusLabel: p.id.includes("edge") ? "Free" : p.is_remote ? "Requires Server" : "Ready",
  }));
}

// --- Deprecated sync helpers (removed per ZBC) --------------------------------
// `CLOUD_PROVIDERS` / `TTS_MODAL_PROVIDERS` mutable singletons and their
// `update*FromContract` sync functions were deleted. Provider lists live in
// `useCatalogStore` (`cloudProviders` / `ttsProviders`), populated by
// `settingsStore.loadContract` via the pure `map*FromContract` mappers below.

