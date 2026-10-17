
export interface CloudProvider {
  id: string;
  name: string;
  url: string;
  keyPlaceholder: string;
  popular?: boolean;
  tagline?: string;
  logo?: string;
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
    is_free?: boolean;
    isFree?: boolean;
    status_label?: string;
    statusLabel?: string;
  }>
) {
  if (!providers || providers.length === 0) return;
  const cloudOnes = providers.filter(
    (t) => t.category === "cloud" || t.is_cloud || t.subcategory === "cloud"
  );
  if (cloudOnes.length === 0) return;
  TTS_MODAL_PROVIDERS.length = 0;
  for (const p of cloudOnes) {
    TTS_MODAL_PROVIDERS.push({
      id: p.id,
      name: p.name,
      category: "cloud",
      tagline: p.tagline || p.description || "",
      url: p.url,
      logo: p.logo,
      isFree: p.is_free ?? p.isFree ?? p.id.includes("edge"),
      statusLabel:
        p.status_label ||
        p.statusLabel ||
        (p.id.includes("edge") ? "Free" : "Ready"),
    });
  }
}

