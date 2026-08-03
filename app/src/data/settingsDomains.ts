import type { LucideIcon } from "lucide-react";
import { Brain, Database, Eye, Palette, Sliders, UserCircle } from "lucide-react";

export type SettingsDomainId = "persona" | "models" | "tray" | "memory" | "appearance" | "interaction";

export interface SettingsDomain {
  id: SettingsDomainId;
  label: string;
  sublabel: string;
  icon: LucideIcon;
  angle: number;
}

export const SETTINGS_DOMAINS: SettingsDomain[] = [
  { id: "persona", label: "Persona", sublabel: "Prompts & identity", icon: UserCircle, angle: -90 },
  { id: "models", label: "Models", sublabel: "Intelligence engines", icon: Brain, angle: -30 },
  { id: "tray", label: "Tray", sublabel: "HUD & overlay settings", icon: Eye, angle: 30 },
  { id: "appearance", label: "Appearance", sublabel: "Visual theme & colors", icon: Palette, angle: 90 },
  { id: "memory", label: "Memory", sublabel: "Database & retention", icon: Database, angle: 150 },
  { id: "interaction", label: "Interaction", sublabel: "Activation & cloud key", icon: Sliders, angle: -150 },
];

export const MOBILE_SETTINGS_ORDER = ["interaction", "tray", "models", "appearance", "memory", "persona"] as const;

export const REALTIME_SUBKEYS = ["gemini", "openai", "deepgram", "elevenlabs"] as const;

export type SettingsScope =
  | "ui"
  | "audio"
  | "vad"
  | "asr"
  | "llm"
  | "tts"
  | "interaction"
  | "telemetry"
  | "persistence"
  | "memory"
  | "assistant"
  | "setup"
  | "realtime";

export const SETTINGS_SCOPE_KEYS: Record<SettingsScope, readonly string[]> = {
  ui: ["theme", "accent_seed", "tray_enabled", "tray_blur_density", "tray_glass_tint", "tray_history_limit"],
  audio: ["output_mode", "input_device"],
  vad: ["threshold", "ptt_noise_gate", "vad_backend"],
  asr: ["model", "transliterate_enabled", "provider"],
  llm: ["model", "ctx_size", "threads", "provider"],
  tts: ["provider", "voice", "quality_steps", "speed"],
  interaction: ["main_app_mode", "tray_mode", "auto_sleep_timeout", "pipeline_mode"],
  telemetry: ["enabled", "log_level"],
  persistence: ["private_mode", "max_sessions", "retention_days"],
  memory: ["episodic_enabled", "bg_worker_enabled", "top_k", "similarity_threshold", "max_context_share"],
  assistant: ["modular_prompt", "realtime_prompt"],
  setup: ["completed"],
  realtime: ["provider", "gemini", "openai", "deepgram", "elevenlabs"],
};

export interface DomainDirtyKey {
  scope: SettingsScope;
  keys?: readonly string[];
  nestedKey?: string;
}

export const DOMAIN_DIRTY_KEYS: Record<SettingsDomainId, readonly DomainDirtyKey[]> = {
  models: [
    { scope: "vad" },
    { scope: "asr" },
    { scope: "tts" },
    { scope: "llm", keys: ["model", "ctx_size", "threads"] },
    { scope: "llm", keys: ["provider"], nestedKey: "model" },
  ],
  tray: [
    { scope: "ui", keys: ["tray_enabled", "tray_blur_density", "tray_glass_tint", "tray_history_limit"] },
    { scope: "interaction", keys: ["tray_mode"] },
  ],
  persona: [{ scope: "assistant" }],
  memory: [{ scope: "persistence" }, { scope: "memory" }],
  appearance: [{ scope: "ui", keys: ["theme", "accent_seed"] }],
  interaction: [
    { scope: "interaction", keys: ["main_app_mode", "auto_sleep_timeout", "pipeline_mode"] },
    { scope: "llm", keys: ["provider"] },
    { scope: "realtime", keys: ["provider", "gemini", "openai", "deepgram", "elevenlabs"] },
  ],
};
