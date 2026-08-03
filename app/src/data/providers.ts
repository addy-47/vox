import type { LucideIcon } from "lucide-react";
import { Brain, Cloud, Server } from "lucide-react";

export interface CloudProvider {
  id: string;
  name: string;
  url: string;
  keyPlaceholder: string;
}

export const CLOUD_PROVIDERS: CloudProvider[] = [
  { id: "openai", name: "OpenAI", url: "https://api.openai.com/v1", keyPlaceholder: "sk-proj-..." },
  { id: "gemini", name: "Gemini", url: "https://generativelanguage.googleapis.com/v1beta", keyPlaceholder: "AIzaSy..." },
  { id: "nvidia", name: "NVIDIA NIM", url: "https://integrate.api.nvidia.com/v1", keyPlaceholder: "nvapi-..." },
  { id: "anthropic", name: "Anthropic", url: "https://api.anthropic.com/v1", keyPlaceholder: "sk-ant-..." },
  { id: "groq", name: "Groq", url: "https://api.groq.com/openai/v1", keyPlaceholder: "gsk_..." },
];

export type CloudProviderId = (typeof CLOUD_PROVIDERS)[number]["id"];

export const CLOUD_PROVIDER_HOSTS = ["openai.com", "googleapis.com", "anthropic.com", "groq.com", "nvidia.com"] as const;

export const REALTIME_PROVIDERS = [
  { id: "gemini_live", name: "Gemini Live", subkey: "gemini", desc: "Sub-300ms Duplex", url: "https://aistudio.google.com/apikey", tagline: "Google's multimodal live streaming with sub-300ms duplex voice interaction" },
  { id: "openai_realtime", name: "OpenAI Realtime", subkey: "openai", desc: "S2S WebSocket", url: "https://platform.openai.com/api-keys", tagline: "OpenAI's speech-to-speech API via persistent WebSocket connections" },
  { id: "deepgram_voice_agent", name: "Deepgram Agent", subkey: "deepgram", desc: "Voice Agent SDK", url: "https://console.deepgram.com/", tagline: "Deepgram's voice agent platform for building custom AI assistants" },
  { id: "elevenlabs_convai", name: "ElevenLabs ConvAI", subkey: "elevenlabs", desc: "Conversational AI", url: "https://elevenlabs.io/app/settings/api-keys", tagline: "ElevenLabs' conversational AI with ultra-realistic voice synthesis" },
] as const;

export type RealtimeProvider = (typeof REALTIME_PROVIDERS)[number];

export const REALTIME_PROVIDER_SUBKEY: Record<RealtimeProvider["id"], string> = {
  gemini_live: "gemini",
  openai_realtime: "openai",
  deepgram_voice_agent: "deepgram",
  elevenlabs_convai: "elevenlabs",
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
