import type { LucideIcon } from "lucide-react";
import { Archive, CircleUserRound, History, Orbit, Palette, SlidersHorizontal } from "lucide-react";

export type SettingsDomainId = "persona" | "models" | "history" | "memory" | "appearance" | "interaction";

export interface SettingsDomain {
  id: SettingsDomainId;
  label: string;
  sublabel: string;
  icon: LucideIcon;
  angle: number;
}

export const SETTINGS_DOMAINS: SettingsDomain[] = [
  { id: "persona", label: "Persona", sublabel: "Prompts & identity", icon: CircleUserRound, angle: -90 },
  { id: "models", label: "Models", sublabel: "Voice & thinking models", icon: Orbit, angle: -30 },
  { id: "history", label: "History", sublabel: "Session history & limits", icon: History, angle: 30 },
  { id: "appearance", label: "Appearance", sublabel: "Visual theme & colors", icon: Palette, angle: 90 },
  { id: "memory", label: "Memory", sublabel: "What Vox remembers", icon: Archive, angle: 150 },
  { id: "interaction", label: "Interaction", sublabel: "Activation & cloud key", icon: SlidersHorizontal, angle: -150 },
];

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
  vad: ["threshold", "ptt_noise_gate", "vad_backend"],
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
    "kokoro",
    "chatterbox",
    "chatterbox_remote",
  ],
  interaction: ["mode", "auto_sleep_timeout", "pipeline_mode"],
  dictation: ["enabled", "interaction_mode", "hotkey", "output_mode"],
  history: ["private_mode", "tray_history_limit", "auto_compaction"],
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
    { scope: "audio", keys: SETTINGS_SCOPE_KEYS.audio },
    { scope: "vad", keys: SETTINGS_SCOPE_KEYS.vad },
    { scope: "stt", keys: SETTINGS_SCOPE_KEYS.stt },
    { scope: "tts", keys: SETTINGS_SCOPE_KEYS.tts },
    { scope: "llm", keys: SETTINGS_SCOPE_KEYS.llm },
    { scope: "realtime", keys: SETTINGS_SCOPE_KEYS.realtime },
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
    { scope: "realtime" },
  ],
};

export const REALTIME_CONFIG_DESK_COPY = {
  title: "Live Voice Provider",
  duplexBadge: "Direct Voice",
  providerLabel: "Voice Provider",
  apiKeyLabel: "API Key (Required)",
  prevProvider: "Previous provider",
  nextProvider: "Next provider",
};

export const PIPELINE_MODE_COPY = {
  modularTitle: "Modular",
  modularSub: "Custom Models",
  realtimeTitle: "Realtime",
  realtimeSub: "Direct Voice",
};

export const TRIGGER_MODE_COPY = {
  continuousTitle: "Continuous",
  continuousSub: "Always Listening",
  pttTitle: "Push-To-Talk",
  pttSub: "Hold to Speak",
};

export const SETTINGS_COPY = {
  settingsTitle: "Settings",
  settingsSubtitle: "System Configuration",
  unsavedChanges: "Unsaved Changes",
  saveChanges: "Save",
  discardChanges: "Discard",
  applyAndReload: "Apply & Reload",
  changesSaved: "Changes Saved",
  autoSynced: "Auto-synced",
  apiKeyRequired: "API Key Required for Cloud Provider",
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

export const INTERACTION_CONFIG_DESK_COPY = {
  integrated: {
    title: "Integrated Voice Engine",
    badge: "Sub-200ms",
    description: "Streaming STT, LLM inference, and TTS audio synthesis run tightly coupled in unified process memory for minimal perceived latency.",
  },
  stt: {
    local: {
      title: "Embedded Speech Recognition",
      description: "Speech recognition models run 100% locally inside Vox using on-device neural engines. Voice audio is processed entirely on your hardware with zero external network transmission.",
    },
    remote: {
      title: "Remote Speech Server",
      badge: "Coming Soon",
      description: "Stream microphone audio frames directly to a self-hosted or dedicated remote ASR inference node over secure WebSocket connection.",
    },
    cloud: {
      title: "Cloud Speech API",
      badge: "Coming Soon",
      description: "Ultra-low latency streaming cloud transcription powered by hosted speech-to-text API endpoints.",
    },
  },
  llm: {
    local: {
      title: "Embedded Neural LLM",
      description: "Language models run directly on your local hardware using Vox's embedded inference engine. All conversational context and prompt reasoning remain completely offline and private.",
    },
    remote: {
      title: "Self-Hosted / Ollama Server",
      urlLabel: "Server URL",
      urlPlaceholder: "http://127.0.0.1:11434",
      apiKeyLabel: "API Key (Optional)",
      apiKeyPlaceholder: "Bearer token...",
    },
    cloud: {
      title: "Cloud Provider API",
      providerLabel: "Cloud Provider",
      apiKeyLabel: "API Key (Required)",
    },
  },
  tts: {
    local: {
      title: "Embedded Voice Synthesizer",
      description: "Voice generation models execute entirely on-device for crisp, realtime offline audio synthesis with zero cloud latency or external API costs.",
    },
    remote: {
      title: "Chatterbox GPU Server",
      urlLabel: "Server HTTP URL",
      urlPlaceholder: "http://127.0.0.1:7860",
      pathLabel: "Remote Path",
      pathPlaceholder: "~/.vox",
    },
    cloud: {
      title: "Cloud Voice Engine",
      badge: "Zero Config",
      description: "Natural multi-voice speech synthesis streamed over the web with high-fidelity pronunciation, global accent selection, and zero API key setup.",
    },
  },
};

export const MEMORY_CONFIG_DESK_COPY = {
  cardTitle: "Memory Stack",
  recallToggle: {
    title: "Retrieval",
    activeLabel: "Recall Active",
    inactiveLabel: "Recall Paused",
    activeSublabel: "Context Injected",
    inactiveSublabel: "Turn Bypassed",
  },
  pipelineToggle: {
    title: "Processing",
    activeLabel: "Pipeline Active",
    inactiveLabel: "Pipeline Paused",
    activeSublabel: "Background Ingestion",
    inactiveSublabel: "Queue Staged Only",
  },
  tabs: {
    depth: "Depth",
    cutoff: "Cutoff",
    graph: "Graph",
    budget: "Budget",
    window: "Window",
  },
  depth: {
    title: "Recall Fact Limit",
    description: "Maximum number of long-term facts and memories injected into context for each conversation turn.",
    unit: "facts",
  },
  cutoff: {
    title: "Relevance Cutoff",
    description: "Minimum semantic similarity score required for a past fact to be recalled and sent to the model.",
    knobLabel: "Cutoff Floor",
  },
  graph: {
    title: "Knowledge Graph Hops",
    description: "Maximum relationship connections explored across entity nodes to discover linked memories.",
  },
  budget: {
    title: "Context Budget",
    description: "Maximum percentage of LLM prompt window allocated to memory facts and user profile context.",
  },
  window: {
    title: "Conversation Window",
    description: "Duration over which past dialogue turns are chained together as continuous active context.",
  },
};

export const TTS_VOICE_MANAGER_COPY = {
  tabs: {
    selectVoice: "Select Voice",
    speechSpeed: "Speech Rate",
  },
  voice: {
    title: "Voice Profile",
    prefix: "Choose an AI voice persona from the",
    suffix: "region to synthesize natural audio responses.",
    localDescription: "Select an on-device neural voice profile for offline speech synthesis.",
  },
  speed: {
    title: "Speech Rate",
    description: "Fine-tune speech synthesis playback tempo and velocity across assistant responses without pitch distortion.",
  },
};

export const MODEL_HUB_COPY = {
  backToModels: "Models",
  present: "Ready",
  notPresent: "Not downloaded",
  unsavedChanges: "Unsaved changes for this provider",
};

export const LLM_SETTINGS_COPY = {
  tabs: {
    compute: "Compute",
    tokens: "Response",
    context: "Context",
    creativity: "Creativity",
  },
};

export const HISTORY_SETTINGS_COPY = {
  privateModeTitle: "Session Storage",
  privateModeActive: "Incognito Active",
  privateModeInactive: "Logging Active",
  privateModeActiveSub: "No traces saved",
  privateModeInactiveSub: "Standard SQLite",
  autoCompactionTitle: "Auto Compaction",
  autoCompactionActive: "Auto Active",
  autoCompactionInactive: "Manual Review",
  autoCompactionActiveSub: "Summarizes on idle",
  autoCompactionInactiveSub: "Prompt on uncompacted",
};

export const VAD_SETTINGS_COPY = {
  tabs: {
    sensitivity: "Sensitivity",
    silence: "Silence Cutoff",
    noiseGate: "Noise Gate",
  },
  sensitivity: {
    title: "Voice Sensitivity",
    description: "Neural probability threshold required to detect user speech. Lower values detect soft whispers; higher values prevent ambient pickup.",
  },
  silence: {
    title: "Silence Duration",
    description: "Pause duration before Vox considers speech finished and initiates response reasoning.",
  },
  noiseGate: {
    title: "Acoustic Noise Gate",
    description: "Raw microphone energy floor to suppress background PC fans, air conditioners, and mechanical keystrokes.",
  },
};

export const STT_SETTINGS_COPY = {
  tabs: {
    streamingRate: "Streaming Rate",
    transliteration: "Transliterate",
  },
  streamingRate: {
    title: "Live Subtitle Cadence",
    description: "Throttle interval for interim partial speech transcription. Faster cadences yield immediate visual feedback at higher CPU load.",
  },
  transliteration: {
    title: "Script Transliteration",
    description: "Automatically transliterates non-Latin or phonetic scripts to standard script formats during transcription.",
  },
};

