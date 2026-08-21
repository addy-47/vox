import type { LucideIcon } from "lucide-react";
import { Brain, Database, History, Palette, Sliders, UserCircle } from "lucide-react";

export type SettingsDomainId = "persona" | "models" | "history" | "memory" | "appearance" | "interaction";

export interface SettingsDomain {
  id: SettingsDomainId;
  label: string;
  sublabel: string;
  icon: LucideIcon;
  angle: number;
}

export const SETTINGS_DOMAINS: SettingsDomain[] = [
  { id: "persona", label: "Persona", sublabel: "Prompts & identity", icon: UserCircle, angle: -90 },
  { id: "models", label: "Models", sublabel: "Voice & thinking models", icon: Brain, angle: -30 },
  { id: "history", label: "History", sublabel: "Session history & limits", icon: History, angle: 30 },
  { id: "appearance", label: "Appearance", sublabel: "Visual theme & colors", icon: Palette, angle: 90 },
  { id: "memory", label: "Memory", sublabel: "What Vox remembers", icon: Database, angle: 150 },
  { id: "interaction", label: "Interaction", sublabel: "Activation & cloud key", icon: Sliders, angle: -150 },
];

export const MOBILE_SETTINGS_ORDER = ["interaction", "history", "models", "appearance", "memory", "persona"] as const;

export type SettingsScope =
  | "appearance"
  | "audio"
  | "vad"
  | "stt"
  | "llm"
  | "tts"
  | "interaction"
  | "dictation"
  | "history"
  | "memory"
  | "persona"
  | "realtime"
  | "system";

export const SETTINGS_SCOPE_KEYS: Record<SettingsScope, readonly string[]> = {
  appearance: ["theme", "accent_seed"],
  audio: ["output_mode", "input_device"],
  vad: ["threshold", "ptt_noise_gate", "backend"],
  stt: ["active", "transliterate_enabled", "embedded", "cloud"],
  llm: [
    "active",
    "temperature",
    "compaction_temperature",
    "max_output_tokens",
    "context_window",
    "threads",
    "embedded",
    "server",
    "cloud",
  ],
  tts: [
    "active",
    "voice_index",
    "quality_steps",
    "speed",
    "edge_tts",
    "supertonic",
    "chatterbox",
    "chatterbox_remote",
  ],
  interaction: ["mode", "auto_sleep_timeout", "pipeline_mode"],
  dictation: ["enabled", "interaction_mode", "hotkey", "output_mode"],
  history: ["private_mode", "tray_history_limit"],
  memory: [
    "context_retrieval_enabled",
    "pipeline_processing_enabled",
    "max_context_share",
    "context_chaining_window_hours",
    "top_k_facts",
    "max_hops",
    "semantic_similarity_cutoff",
  ],
  persona: ["modular_prompt", "realtime_prompt"],
  realtime: [
    "active",
    "gemini_live",
    "openai_realtime",
    "deepgram_voice_agent",
    "elevenlabs_convai",
  ],
  system: ["log_level", "telemetry_enabled", "setup_completed"],
};

export interface DomainDirtyKey {
  scope: SettingsScope;
  keys?: readonly string[];
  nestedKey?: string;
}

export const DOMAIN_DIRTY_KEYS: Record<SettingsDomainId, readonly DomainDirtyKey[]> = {
  models: [
    { scope: "audio" },
    { scope: "vad" },
    { scope: "stt" },
    { scope: "tts" },
    { scope: "llm" },
    { scope: "realtime" },
  ],
  history: [
    { scope: "history" },
  ],
  persona: [
    { scope: "persona" },
  ],
  memory: [
    { scope: "memory" },
  ],
  appearance: [
    { scope: "appearance" },
  ],
  interaction: [
    { scope: "interaction" },
    { scope: "dictation" },
  ],
};

export const SETTINGS_COPY = {
  settingsTitle: "Settings",
  unsavedChanges: "Unsaved Changes",
  saveChanges: "Save",
  discardChanges: "Discard",
  restartRequired: "Restart Required to Apply Changes",
  restoreDefaults: "Restore All Defaults",
  confirmRestoreTitle: "Are you sure you want to restore defaults?",
  confirmRestoreDesc: "This will reset all persona, model, voice, interaction, and system configurations to default factory settings.",
  openDomain: "Open {label} settings",
  closeDomain: "Close {label} settings",
  openAllDomains: "Open all settings",
  closeAllDomains: "Clear all settings",
};

export const DICTATION_COPY = {
  destinationTitle: "Output Destination",
  destinationPasteDesc: "Transcribes and automatically pastes into active cursor position",
  destinationClipboardDesc: "Silently copies final transcript to system clipboard without pasting",
  destinationTrayDesc: "Renders live transcription in floating desktop HUD window",
  hotkeyTitle: "Activation Hotkey",
  hotkeySubtitle: "Global shortcut to trigger voice typing",
  editLabel: "Edit",
  saveLabel: "Save",
  recordingPrompt: "Press shortcut keys...",
  cancelLabel: "Esc to cancel",
  modePaste: "Paste",
  modeClipboard: "Clipboard",
  modeTray: "Tray",
  modePasteLong: "Simulated Keystroke Paste",
  modeClipboardLong: "Clipboard Only Buffer",
  modeTrayLong: "Floating Desktop HUD",
  triggerContinuous: "Continuous",
  triggerPtt: "Push-To-Talk",
  triggerContinuousSub: "Always Transcribing",
  triggerPttSub: "Hold Hotkey",
  voiceTypingActive: "Ready to Transcribe",
  voiceTypingInactive: "System Muted",
};

export const CATEGORY_SWITCH_COPY = {
  switchButton: "Switch",
  switchTooltip: "Switch between hearing (STT), thinking (LLM), and speaking (TTS)",
  prevCategory: "Previous category",
  nextCategory: "Next category",
};

export const PERSONA_COPY = {
  cardTitle: "Persona",
  instructionMode: "Instruction Mode",
  tabModular: "Modular",
  tabRealtime: "Realtime",
  viewEdit: "Edit",
  viewPreview: "Preview",
  modularPlaceholder: "Modular instruction prompt...",
  realtimePlaceholder: "Realtime instruction prompt...",
  modularFooterHint: "Supports <lang> and <script> template variables, dynamically resolved based on user speech language.",
  realtimeFooterHint: "Instructions supplied to duplex cloud speech-to-speech models (e.g. Gemini Live).",
  emptyPrompt: "No instructions defined. Switch to Edit to write prompt directives.",
};

