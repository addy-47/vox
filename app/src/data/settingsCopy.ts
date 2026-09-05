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
  title: "Realtime Direct Voice Connection",
  bannerDescPrefix: "Full duplex speech-to-speech engine with ",
  bannerHighlight: "sub-200ms",
  bannerDescSuffix: " live audio streaming, native grounding, and dynamic turn detection.",
  duplexBadge: "Direct Voice",
  providerLabel: "Voice Provider",
  apiKeyLabel: "API Key (Required)",
  prevProvider: "Previous provider",
  nextProvider: "Next provider",
  stages: {
    capture: "Capture",
    think: "Think",
    speak: "Speak",
  },
  voice: {
    selectVoice: "Select Voice",
    prevVoice: "Previous Voice",
    nextVoice: "Next Voice",
  },
  modelLabel: "Model",
  modelPlaceholder: "Model ID",
  temperature: "Temperature",
  hubTitle: "Realtime Hub",
  liveMode: "Live Mode",
};

export const PIPELINE_MODE_COPY = {
  cardTitle: "Pipeline",
  modularTitle: "Modular",
  modularSub: "Custom Models",
  realtimeTitle: "Realtime",
  realtimeSub: "Direct Voice",
};

export const TRIGGER_MODE_COPY = {
  cardTitle: "Trigger",
  continuousTitle: "Continuous",
  continuousSub: "Always Listening",
  pttTitle: "Push-To-Talk",
  pttSub: "Hold to Speak",
};

export const INTERACTION_CARD_COPY = {
  cardTitle: "Interaction",
  viewAssistant: "Assistant",
  viewDictation: "Dictation",
  voiceTyping: "Voice Typing",
  triggerMode: "Trigger Mode",
  toggleEnabled: "Enabled",
  toggleDisabled: "Disabled",
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
  restoreAria: "Restore default settings",
  restoreConfirmAria: "Confirm restore defaults",
  restoreConfirmHint: "Click again to reset",
  confirmRestoreTitle: "Are you sure you want to restore defaults?",
  confirmRestoreDesc: "This will reset all persona, model, voice, interaction, and system configurations to default factory settings.",
  openDomain: "Open {label} settings",
  closeDomain: "Close {label} settings",
  openAllDomains: "Open all settings",
  closeAllDomains: "Clear all settings",
  helpGuideTooltip: "Open help & guide",
  loadingSettings: "Loading Settings...",
  loadingHint: "Reading hardware and model configurations",
  initializingEngine: "INITIALIZING CONFIG ENGINE",
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
  outputTitle: "Output",
  rebindHint: "Click to rebind activation shortcut",
};

export const CATEGORY_SWITCH_COPY = {
  switchButton: "Switch",
  switchTooltip: "Switch between hearing (STT), thinking (LLM), and speaking (TTS)",
  prevCategory: "Previous category",
  nextCategory: "Next category",
  tabs: {
    stt: "Listening",
    sttSub: "Speech Recognition",
    llm: "Reasoning",
    llmSub: "Language Model",
    tts: "Speaking",
    ttsSub: "Voice Synthesis",
  },
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
  modularFooterPrefix: "Supports ",
  modularFooterMid: " and ",
  modularFooterSuffix: " template variables, dynamically resolved based on user speech language.",
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
  status: {
    testing: "Testing",
    offline: "Offline",
    online: "Online",
    active: "Active",
    backToProviders: "Back to providers",
    providers: "Providers",
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
  compute: {
    title: "Compute Allocation",
    description:
      "CPU worker threads for TTS synthesis. Higher counts reduce latency but share cores with STT and LLM.",
  },
  region: {
    previous: "Previous region",
    next: "Next region",
  },
};

export const VOICE_CAROUSEL_COPY = {
  namePlaceholder: "Enter voice name...",
  chooseFile: "Choose WAV File",
  fileSelected: "File Selected:",
  stopRecording: "Stop Recording",
  recordVoice: "Record Voice",
  cancel: "Cancel",
  cloning: "Cloning...",
  processing: "Processing...",
  cloneVoice: "Clone Voice",
  searchPlaceholder: "Search voice...",
  clear: "Clear",
  closeSearch: "Close search",
  searchVoices: "Search Voices",
  searchVoice: "Search Voice",
  saveVoiceName: "Save voice name",
  cancelRename: "Cancel rename",
  noVoice: "No Voice",
  renameCustomVoice: "Rename Custom Voice",
  renameVoice: "Rename voice",
  deleteCustomVoice: "Delete Custom Voice",
  deleteVoice: "Delete voice",
  cloneVoiceProfile: "Clone Voice Profile",
  prevVoice: "Previous Voice",
  nextVoice: "Next Voice",
} as const;

export const LLM_CATALOG_COPY = {
  connectedServer: "Connected Server",
  fetching: "Fetching...",
  customModelPlaceholder: "Enter custom model ID (e.g. mistralai/mistral-large)...",
  customModelTitle: "Enter custom model ID",
  customModelAria: "Custom model ID",
  searchPlaceholder: "Filter models with fuzzy matching (e.g. llama 70b, gemma q4)...",
  searchTitle: "Search models (fzf fuzzy filter)",
  searchAria: "Search models",
  emptyTitle: "No remote models loaded",
  emptyHint: "Ensure the remote server is online and configured in the Interaction Card.",
  clearSearch: "Clear Search",
  use: "Use",
  noServer: "No server configured",
  noModelsMatch: "No models matching",
  toolsSupported: "Supported",
  toolsNone: "None",
  languageStandard: "Standard",
  managed: "Managed",
  gpuBadge: "🚀 GPU",
  cpuBadge: "⚠️ CPU",
  rerunBenchmark: "Re-run capability benchmark",
  runBenchmark: "Run capability benchmark",
  benchmark: "Benchmark",
  benchmarking: "Benchmarking...",
  capabilities: "Capabilities",
  modelCapabilities: "Model Capabilities",
  notBenchmarked: "Not benchmarked",
  speed: "Speed:",
  context: "Context:",
  vram: "VRAM:",
  tools: "Tools:",
  languages: "Languages:",
} as const;

export const REMOTE_SERVER_COPY = {
  bannerTitle: "Chatterbox Remote Deployment",
  bannerBody:
    "Deploy Chatterbox on a remote CUDA-accelerated GPU host (e.g. RunPod, Vast.ai, or homelab) to offload memory-intensive flow-matching voice synthesis. Enter your SSH connection info below to automatically sync the codebase, download GGUF models, and run the server.",
  panelTitle: "Setup Remote GPU Server (SSH Setup Required)",
  online: "Online / Connected",
  offline: "Offline / Unconfigured",
  hostLabel: "SSH Host / Profile",
  hostPlaceholder: "user@hostname",
  portLabel: "SSH Port",
  portPlaceholder: "22",
  keyLabel: "Identity Key Path",
  keyPlaceholder: "~/.ssh/id_rsa",
  steps: {
    initiating: "Initializing Setup...",
    connecting: "Testing SSH Connection...",
    deploying: "Configuring Remote Server...",
    starting_service: "Starting Chatterbox Service...",
    verifying: "Verifying Health Endpoint...",
    complete: "Setup Completed Successfully",
    failed: "Setup Failed",
  } as Record<string, string>,
  footerReady: "Ready to synthesize flow-matching audio.",
  footerBusy: "Syncs scripts and installs PyTorch CUDA on remote host.",
  deployed: "Deployed & Active",
  deploying: "Deploying...",
  deploy: "Deploy Chatterbox Server",
} as const;

export const MODEL_HUB_COPY = {
  title: "Model Hub",
  missing: "Missing",
  notDownloadedDesc: "This model file is not downloaded yet.",
  row: {
    cancel: "Cancel",
    confirmDelete: "Confirm Delete",
    deleteWeights: "Delete weights",
    deleteConfirm: "Delete?",
    downloadModel: "Download model",
    mandatoryNote: "Mandatory core model (cannot be deleted)",
    required: "Required",
    specs: "Specs",
  },
};

export const DIRTY_STATE_COPY = {
  category: "Unsaved changes in this category",
  stage: "Unsaved changes in this stage",
} as const;

export const COMPUTE_PROFILE_COPY = {
  title: "Compute Allocation",
  auto: "Auto",
  eco: "Eco",
  max: "Max",
  custom: "Custom",
} as const;

export const LLM_SETTINGS_COPY = {
  tabs: {
    compute: "Compute",
    tokens: "Response",
    context: "Context",
    creativity: "Creativity",
  },
  compute: {
    title: "Compute Allocation",
    description:
      "Allocate local CPU worker threads for model reasoning. Auto balances thermal load and latency.",
    remoteTitleLocal: "Cloud Infrastructure",
    remoteTitleRemote: "Remote Compute",
    remoteActive: "Active",
    remoteDescription:
      "Inference computation is offloaded entirely to the remote provider. Zero local CPU or RAM is consumed.",
    remoteManaged: "Managed",
  },
  tokens: {
    title: "Token Limit",
    native: "Native",
    description:
      "Maximum token generation per reply. Concise caps prevent rambling; Native lets the model complete reasoning.",
  },
  context: {
    title: "Context Window",
    description:
      "RAM-allocated token budget for conversation history and retrieved memory facts.",
  },
  creativity: {
    title: "Creativity",
    description:
      "Sampling temperature. Lower values produce strict facts; higher values encourage conversational flair.",
  },
};

export const APPEARANCE_COPY = {
  cardTitle: "Appearance",
  darkMode: "Dark Mode",
  lightMode: "Light Mode",
} as const;

export const HISTORY_SETTINGS_COPY = {
  engineTitle: "Session History Engine",
  engineDesc: "Turso SQLite storage active. Conversations are recorded with zero arbitrary retention limits.",
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
    description:
      "Speech detection probability threshold. Lower values catch whispers; higher values prevent ambient room triggers.",
  },
  silence: {
    title: "Silence Cutoff",
    description:
      "Pause duration before speech turn finishes and initiates response reasoning. Snappy for quick orders; patient for contemplation.",
  },
  noiseGate: {
    title: "Noise Gate Floor",
    description:
      "Minimum microphone energy threshold to discard ambient PC fans, air conditioners, and mechanical keyboard clicks.",
  },
};

export const STT_SETTINGS_COPY = {
  tabs: {
    streamingRate: "Streaming Rate",
    transliteration: "Transliterate",
  },
  compute: {
    title: "Compute Allocation",
    description:
      "CPU worker threads allocated for speech recognition inference. Requires restart to apply.",
  },
  streamingRate: {
    title: "Subtitle Cadence",
    description:
      "Interim partial transcription update frequency. Faster updates give immediate feedback; slower updates preserve CPU.",
  },
  transliteration: {
    title: "Script Transliteration",
    description:
      "Automatically normalizes and maps recognized multilingual phonemes into target orthography during live transcription.",
  },
};

