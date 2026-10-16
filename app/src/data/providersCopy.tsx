import React from "react";

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

const GeminiLogo = ({
  active,
  ...props
}: { active?: boolean } & React.SVGProps<SVGSVGElement>) => (
  <svg viewBox="0 0 24 24" fill="none" {...props}>
    <path
      d="M12 3c0 4.5 3.5 8 8 8-4.5 0-8 3.5-8 8 0-4.5-3.5-8-8-8 4.5 0 8-3.5 8-8z"
      fill="currentColor"
      opacity={active ? 0.9 : 0.45}
    />
  </svg>
);

const DeepgramLogo = ({
  active,
  ...props
}: { active?: boolean } & React.SVGProps<SVGSVGElement>) => (
  <svg
    viewBox="0 0 24 24"
    fill="#13EF93"
    {...props}
  >
    <path d="M11.203 24H1.517a.364.364 0 0 1-.258-.62l6.239-6.275a.37.37 0 0 1 .259-.108h3.52c2.723 0 5.025-2.127 5.107-4.845a5.004 5.004 0 0 0-4.999-5.148H7.613v4.646c0 .2-.164.364-.365.364H.968a.365.365 0 0 1-.363-.364V.364C.605.164.768 0 .969 0h10.416c6.684 0 12.111 5.485 12.01 12.187C23.293 18.77 17.794 24 11.202 24z" />
  </svg>
);



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

