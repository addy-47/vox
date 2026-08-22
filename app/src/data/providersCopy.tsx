import React from "react";
import type { LucideIcon } from "lucide-react";
import { Brain, Cloud, Server } from "lucide-react";

export interface CloudProvider {
  id: string;
  name: string;
  url: string;
  keyPlaceholder: string;
}

export const CLOUD_PROVIDERS: CloudProvider[] = [
  { id: "nvidia", name: "NVIDIA NIM", url: "https://integrate.api.nvidia.com/v1", keyPlaceholder: "nvapi-..." },
  { id: "openai", name: "OpenAI", url: "https://api.openai.com/v1", keyPlaceholder: "sk-proj-..." },
  { id: "gemini", name: "Gemini", url: "https://generativelanguage.googleapis.com/v1beta", keyPlaceholder: "AIzaSy..." },
  { id: "anthropic", name: "Anthropic", url: "https://api.anthropic.com/v1", keyPlaceholder: "sk-ant-..." },
  { id: "groq", name: "Groq", url: "https://api.groq.com/openai/v1", keyPlaceholder: "gsk_..." },
];

export type CloudProviderId = (typeof CLOUD_PROVIDERS)[number]["id"];

export const CLOUD_PROVIDER_HOSTS = ["openai.com", "googleapis.com", "anthropic.com", "groq.com", "nvidia.com"] as const;

export const GeminiLogo = ({
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

export const OpenAiLogo = ({
  active,
  ...props
}: { active?: boolean } & React.SVGProps<SVGSVGElement>) => (
  <svg viewBox="0 0 24 24" fill="currentColor" {...props}>
    <path
      d="M21.3 11.1c0-.7-.2-1.4-.6-2-.4-.6-1-.9-1.7-1.1-.1-.7-.4-1.3-.9-1.8s-1.1-.9-1.8-1c-.5-.5-1.1-.9-1.8-1-.7-.2-1.4-.2-2.1 0-.6.2-1.2.5-1.7 1-.5-.5-1.1-.8-1.7-1-.7-.2-1.4-.2-2.1 0-.7.2-1.3.5-1.8 1-.5.5-.8 1.1-.9 1.8-.7.1-1.3.4-1.8.9C3 8.4 2.7 9 2.6 9.7c-.5.5-.9 1.1-1 1.8-.2.7-.2 1.4 0 2.1.2.6.5 1.2 1 1.7-.5.5-.8 1.1-1 1.7-.2.7-.2 1.4 0 2.1.2.7.5 1.3 1 1.8.5.5 1.1.8 1.8.9.1.7.4 1.3.9 1.8.5.5 1.1.9 1.8 1 .5.5 1.1.9 1.8 1 .7.2 1.4.2 2.1 0 .6-.2 1.2-.5 1.7-1 .5.5 1.1.8 1.7 1 .7.2 1.4.2 2.1 0 .7-.2 1.3-.5 1.8-1 .5-.5.8-1.1.9-1.8.7-.1 1.3-.4 1.8-.9.5-.5.8-1.1.9-1.8.5-.5.9-1.1 1-1.8.2-.7.2-1.4 0-2.1-.2-.6-.5-1.2-1-1.7.5-.5.8-1.1 1-1.7.2-.6.2-1.3 0-2zm-8.8 7.3l-2.9-1.7c-.2-.1-.3-.3-.3-.6V12.7l1.4.8c.2.1.3.3.3.6v2.1l1.5.9v-4.2l-1.4-.8c-.2-.1-.3-.3-.3-.6V9.3l2.9 1.7c.2.1.3.3.3.6v3.4l-1.4-.8c-.2-.1-.3-.3-.3-.6v-2.1l-1.5-.9v4.2l1.4.8c.2.1.3.3.3.6v1.9z"
      fill="currentColor"
      opacity={active ? 0.9 : 0.45}
    />
  </svg>
);

export const DeepgramLogo = ({
  active,
  ...props
}: { active?: boolean } & React.SVGProps<SVGSVGElement>) => (
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2.5"
    strokeLinecap="round"
    strokeLinejoin="round"
    {...props}
  >
    <polygon
      points="12 2 22 8.5 22 15.5 12 22 2 15.5 2 8.5"
      opacity={active ? 0.95 : 0.5}
    />
    <path d="M12 22V12" opacity={active ? 0.95 : 0.5} />
    <path d="M12 12L22 8.5" opacity={active ? 0.95 : 0.5} />
    <path d="M12 12L2 8.5" opacity={active ? 0.95 : 0.5} />
  </svg>
);

export const ElevenLabsLogo = ({
  active,
  ...props
}: { active?: boolean } & React.SVGProps<SVGSVGElement>) => (
  <svg viewBox="0 0 24 24" fill="currentColor" {...props}>
    <rect
      x="5"
      y="4"
      width="5.5"
      height="16"
      rx="2.5"
      fill="currentColor"
      opacity={active ? 0.95 : 0.45}
    />
    <rect
      x="13.5"
      y="4"
      width="5.5"
      height="16"
      rx="2.5"
      fill="currentColor"
      opacity={active ? 0.75 : 0.3}
    />
  </svg>
);

export const checkIfCloudUrl = (url: string) => {
  if (!url) return false;
  return (
    url.includes("openai.com") ||
    url.includes("googleapis.com") ||
    url.includes("anthropic.com") ||
    url.includes("groq.com") ||
    url.includes("nvidia.com")
  );
};

export const REALTIME_PROVIDERS = [
  {
    id: "gemini_live",
    name: "Gemini Live",
    subkey: "gemini_live",
    icon: GeminiLogo,
    desc: "Live Speech",
    url: "https://aistudio.google.com/apikey",
    tagline: "Google's direct live speech model with search grounding",
    keyPlaceholder: "AIzaSy...",
  },
  {
    id: "deepgram_voice_agent",
    name: "Deepgram Agent",
    subkey: "deepgram_voice_agent",
    icon: DeepgramLogo,
    desc: "Voice Assistant",
    url: "https://console.deepgram.com/",
    tagline: "Deepgram's real-time conversational voice agent",
    keyPlaceholder: "Token...",
  },
] as const;

export type RealtimeProvider = (typeof REALTIME_PROVIDERS)[number];

export const REALTIME_PROVIDER_SUBKEY: Record<string, string> = {
  gemini_live: "gemini_live",
  gemini: "gemini_live",
  openai_realtime: "openai_realtime",
  openai: "openai_realtime",
  deepgram_voice_agent: "deepgram_voice_agent",
  deepgram: "deepgram_voice_agent",
  elevenlabs_convai: "elevenlabs_convai",
  elevenlabs: "elevenlabs_convai",
};

export const REALTIME_PROVIDER_DISPLAY_NAMES = {
  gemini_live: "Gemini Multimodal Live",
  openai_realtime: "OpenAI Realtime API",
  deepgram_voice_agent: "Deepgram Voice Agent",
  elevenlabs_convai: "ElevenLabs Conversational AI",
} as const;

export const REALTIME_PROVIDER_SHORT_NAMES = {
  gemini_live: "Gemini",
  openai_realtime: "OpenAI",
  deepgram_voice_agent: "Deepgram",
  elevenlabs_convai: "ElevenLabs",
} as const;

export const REALTIME_DEFAULT_MODEL_IDS = {
  gemini: "gemini-2.5-flash",
  openai: "gpt-4o-realtime-preview",
  deepgram: "gpt-4o-mini",
  elevenlabs: "",
} as const;

export const REALTIME_SUBKEY_TOGGLES = {
  gemini: { field: "enable_web_search", voiceField: "voice_name", label: "Google Search", sub: "Live web grounding" },
  openai: { field: "voice_activity_detection", voiceField: "voice", label: "VAD", sub: "Activity detection" },
  deepgram: { field: "agent_mode", voiceField: "voice", label: "Agent Mode", sub: "AI agent routing" },
  elevenlabs: { field: "dynamic_vars", voiceField: "voice", label: "Dynamic Vars", sub: "Context variables" },
} as const;

export interface ProviderCategoryPill {
  id: "local" | "remote" | "cloud";
  label: string;
  icon: LucideIcon;
}

export const PROVIDER_CATEGORY_PILLS: ProviderCategoryPill[] = [
  { id: "local", label: "Embedded", icon: Brain },
  { id: "remote", label: "Server", icon: Server },
  { id: "cloud", label: "Cloud", icon: Cloud },
];
