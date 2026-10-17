
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

export const CLOUD_PROVIDERS: CloudProvider[] = [];



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

export const TTS_MODAL_PROVIDERS: TtsProviderEntry[] = [];

/**
 * Synchronizes the module-level CLOUD_PROVIDERS array from the backend contract.
 */
export function updateCloudProvidersFromContract(
  providers: Array<{
    id: string;
    name: string;
    base_url?: string;
    url?: string;
    key_placeholder?: string;
    keyPlaceholder?: string;
    popular?: boolean;
    tagline?: string;
    logo?: string;
  }>
) {
  if (!providers || providers.length === 0) return;
  CLOUD_PROVIDERS.length = 0;
  for (const p of providers) {
    CLOUD_PROVIDERS.push({
      id: p.id,
      name: p.name,
      url: p.base_url || p.url || "",
      keyPlaceholder: p.key_placeholder || p.keyPlaceholder || "sk-...",
      popular: p.popular,
      tagline: p.tagline,
      logo: p.logo,
    });
  }
}

/**
 * Synchronizes the module-level TTS_MODAL_PROVIDERS array from the backend contract.
 */
export function updateTtsProvidersFromContract(
  providers: Array<{
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
  }>
) {
  if (!providers || providers.length === 0) return;
  const filtered = providers.filter(
    (t) => t.is_cloud || t.is_remote || t.subcategory === "cloud" || t.subcategory === "remote"
  );
  if (filtered.length === 0) return;
  TTS_MODAL_PROVIDERS.length = 0;
  for (const p of filtered) {
    const nameMap: Record<string, string> = {
      google_tts: "Google Cloud TTS",
      azure_tts: "Azure AI Speech",
      edge_tts: "Microsoft Edge TTS",
      chatterbox_remote: "Chatterbox Remote (HTTP)",
    };
    TTS_MODAL_PROVIDERS.push({
      id: p.id,
      name: nameMap[p.id] || p.name,
      category: p.is_remote ? "server" : "cloud",
      tagline: p.description || p.tagline || "",
      url: p.url,
      logo: p.logo,
      isFree: p.is_free ?? p.id.includes("edge"),
      isConfigured: Boolean(p.is_configured ?? p.isConfigured),
      statusLabel: p.id.includes("edge") ? "Free" : p.is_remote ? "Requires Server" : "Ready",
    });
  }
}

