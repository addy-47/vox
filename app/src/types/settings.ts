export type PipelineMode = "modular" | "realtime";

/** Primary runtime tier shared by every model domain. */
export type ActiveCategory = "embedded" | "server" | "cloud";

/**
 * Mirrors the backend `ActiveSelection` (`app/src-tauri/src/config/types.rs`).
 * `provider` is null for plain embedded models, otherwise a provider name or a
 * server id from `providers.*.servers`.
 */
export interface ActiveSelection {
  category: ActiveCategory;
  provider: string | null;
  model: string;
}

/** Mirrors the backend `LastSelectedModels` (`config/types.rs`). */
export interface LastSelectedModels {
  embedded?: string;
  server?: string;
  cloud?: string;
}

/** Realtime configuration subkey (the `realtime.<subkey>` settings object). */
export type RealtimeSubkey = string;

export type LlmProviderKind = ActiveCategory;

export interface LlmProviderConfig {
  kind: LlmProviderKind;
  /** Inventory id (`providers.llm.servers[].id`) or cloud provider id. Never a display name. */
  provider_id?: string;
  base_url?: string;
  model?: string;
  api_key?: string;
  provider_name?: string;
}

export type SttProviderKind = "embedded";

export interface SttProviderConfig {
  kind: SttProviderKind;
  model_type?: string;
}

export type CapabilitySupport = "supported" | "unsupported" | "unknown";

export type CapabilityProvenance =
  | "catalog_baseline"
  | "family_baseline"
  | "probed_server"
  | "declared_static"
  | "user_configured"
  | "unknown";

export type ProbeOutcome = "measured" | "unsupported" | "failed" | "skipped";

export interface ProbeCheck {
  id: string;
  label: string;
  outcome: ProbeOutcome;
  detail?: string | null;
  duration_ms?: number | null;
}

export interface ModelCapabilities {
  model_id: string;
  provider_kind: string;
  supports_tools: CapabilitySupport;
  supports_latin: CapabilitySupport;
  supports_devanagari: CapabilitySupport;
  context_window?: number | null;
  max_output_tokens?: number | null;
  provenance: CapabilityProvenance;
  tps?: number | null;
  ttft_ms?: number | null;
  server_has_gpu: boolean;
  is_gpu_accelerated: boolean;
  gpu_status: string;
  vram_bytes?: number | null;
  parameter_size?: string | null;
  quantization?: string | null;
  family?: string | null;
  tested_at_epoch: number;
  checks: ProbeCheck[];
}

export interface LlmModelInfo {
  id: string;
  name: string;
  size_bytes: number | null;
  quantization: string | null;
  family: string | null;
  provider_kind: string;
  capabilities?: ModelCapabilities | null;
}

export interface ModelEntry {
  id: string;
  path: string;
  size: number;
  sha256: string;
  archive?: string | null;
  required?: boolean;
}

export interface ModelGroupInfo {
  id: string;
  name: string;
  category: string;
  subcategory?: string;
  description?: string;
  parameters?: string;
  ram_usage?: string;
  tradeoffs?: string;
  version: string;
  is_built_in?: boolean;
  is_cloud?: boolean;
  is_remote?: boolean;
  is_configured?: boolean;
  is_free?: boolean;
  required?: boolean;
  files?: ModelEntry[];
}

export type { TtsVoiceSource, ParamRange, ProviderCaps } from "@/store/settingsStore";

// Alias for backwards compatibility during component migration
export type ModelMetadata = ModelGroupInfo;

export interface VoiceProfile {
  id: number;
  name: string;
}

export interface ModelCatalog {
  llm: ModelGroupInfo[];
  stt: ModelGroupInfo[];
  tts: ModelGroupInfo[];
  vad: ModelGroupInfo[];
  auxiliary: ModelGroupInfo[];
  model_groups: ModelGroupInfo[];
  voices: VoiceProfile[];
  preset_colors: string[];
  active_voice_name?: string | null;
  /** Per-model parameter overrides from the `models` table (`ipc/catalog.rs`). */
  model_parameters: Record<string, Record<string, unknown>>;
}

export type AudioOutputMode = "Speaker" | "Headset";

export interface VadSettings {
  threshold: number;
  ptt_noise_gate: number;
  vad_backend: string;
  silence_duration_ms: number;
  speech_onset_ms: number;
  max_speech_duration_s: number;
}

export interface SttSettings {
  active: ActiveSelection;
  last_selected: LastSelectedModels;
  transliterate_enabled: boolean;
}

export interface LlmSettings {
  active: ActiveSelection;
  last_selected: LastSelectedModels;
  cloud_keys: Record<string, string>;
  /**
   * Draft-only vehicle for the custom server inventory. The backend persists
   * it to `providers.jsonc` (`update_setting("llm","servers",…)`); it is never
   * part of `settings.jsonc`, so it stays undefined until the user edits it.
   */
  servers?: LlmCustomServer[];
}

export interface TtsSettings {
  active: ActiveSelection;
  last_selected: LastSelectedModels;
  active_voice_id: string;
  /**
   * Draft-only vehicle for the TTS server inventory, persisted to
   * `providers.jsonc` like `LlmSettings.servers`.
   */
  servers?: TtsCustomServer[];
}

export interface RealtimeSettings {
  active: ActiveSelection;
  active_voice_id: string;
  /**
   * Draft-only vehicle for realtime provider API keys, persisted to
   * `providers.jsonc` (`update_setting("realtime","api_keys",…)`).
   */
  api_keys?: Record<string, string>;
}

export interface InteractionSettings {
  mode: "passive" | "ptt" | "Passive" | "PTT";
  pipeline_mode: PipelineMode;
  web_search_enabled: boolean;
}

export interface DictationSettings {
  enabled: boolean;
  interaction_mode: "passive" | "ptt";
  hotkey: string;
  output_mode: "paste" | "clipboard" | "tray";
  silence_auto_stop_ms?: number;
}

export interface MemorySettings {
  context_retrieval_enabled: boolean;
  pipeline_processing_enabled: boolean;
  private_mode: boolean;
  consolidation_cadence: string;
  consolidation_time: string;
  suggestion_policy?: "manual_review" | "auto_apply" | string;
  auto_compaction: boolean;
  max_context_share: number;
  top_k_facts: number;
  semantic_similarity_cutoff: number;
}

export interface AppearanceSettings {
  theme: string;
  accent_seed: string;
}

export interface PersonaSettings {
  modular_prompt: string;
  realtime_prompt: string;
}

export interface SystemSettings {
  setup_completed: boolean;
}

export interface LlmCustomServer {
  id: string;
  name: string;
  url: string;
  model: string;
  protocol: string;
  api_key?: string | null;
}

export interface TtsCustomServer {
  id: string;
  name: string;
  url: string;
  remote_path: string;
  api_key?: string | null;
}

export interface ProvidersConfigFile {
  llm: {
    servers: LlmCustomServer[];
    cloud_keys: Record<string, string>;
  };
  tts: {
    servers: TtsCustomServer[];
  };
  realtime: {
    api_keys: Record<string, string>;
  };
}

export interface VoxSettings {
  vad: VadSettings;
  stt: SttSettings;
  llm: LlmSettings;
  tts: TtsSettings;
  realtime: RealtimeSettings;
  interaction: InteractionSettings;
  dictation: DictationSettings;
  memory: MemorySettings;
  appearance: AppearanceSettings;
  persona: PersonaSettings;
  system: SystemSettings;
  providers?: ProvidersConfigFile;
}

/** Per-domain commit state, derived once per store mutation. */
export interface DomainFlags {
  dirty: boolean;
  requiresRestart: boolean;
}

/**
 * Every `(domain, key)` pair the backend `apply_setting_mutation` accepts
 * (`app/src-tauri/src/config/mutation.rs`). `updateDraft` only compiles for
 * these keys, so a phantom write like `("llm","server")` is a build error,
 * not a runtime rejection. `providers` maps to `never`: inventory writes go
 * through their domain vehicle (`llm.servers`, `llm.cloud_keys`, …).
 */
export interface DraftKeyMap {
  appearance: "theme" | "accent_seed";
  vad: "threshold" | "ptt_noise_gate" | "silence_duration_ms" | "speech_onset_ms" | "vad_backend";
  stt: "active" | "last_selected" | "transliterate_enabled";
  llm: "active" | "last_selected" | "servers" | "cloud_keys";
  tts: "active" | "last_selected" | "active_voice_id" | "voice" | "voice_id" | "servers";
  realtime: "active" | "active_voice_id" | "api_keys";
  interaction: "mode" | "pipeline_mode" | "web_search_enabled";
  dictation: "enabled" | "interaction_mode" | "hotkey" | "output_mode" | "silence_auto_stop_ms";
  memory:
    | "context_retrieval_enabled"
    | "pipeline_processing_enabled"
    | "private_mode"
    | "consolidation_cadence"
    | "consolidation_time"
    | "suggestion_policy"
    | "auto_compaction"
    | "max_context_share"
    | "top_k_facts"
    | "semantic_similarity_cutoff";
  persona: "modular_prompt" | "realtime_prompt";
  system: "setup_completed";
  providers: never;
}
