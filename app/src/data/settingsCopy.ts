import type { LucideIcon } from "lucide-react";
import { BookOpenText, CircleUserRound, NotebookTabs, Orbit, Palette, SlidersHorizontal } from "lucide-react";

export type SettingsDomainId = "persona" | "models" | "working_memory" | "personal_memory" | "appearance" | "interaction";

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
  { id: "working_memory", label: "Working", sublabel: "Context, turns & compaction", icon: NotebookTabs, angle: 30 },
  { id: "appearance", label: "Appearance", sublabel: "Visual theme & colors", icon: Palette, angle: 90 },
  { id: "personal_memory", label: "Personal", sublabel: "Knowledge, facts & consolidation", icon: BookOpenText, angle: 150 },
  { id: "interaction", label: "Interaction", sublabel: "Activation & cloud key", icon: SlidersHorizontal, angle: -150 },
];

export type SettingsScope =
  | "appearance"
  | "vad"
  | "stt"
  | "llm"
  | "tts"
  | "interaction"
  | "dictation"
  | "working_memory"
  | "personal_memory"
  | "persona"
  | "realtime"
  | "system";

export const SETTINGS_SCOPE_KEYS: Partial<Record<SettingsScope, readonly string[]>> = {
  appearance: ["theme", "accent_seed"],
  // `vad`, `stt`, `llm`, and `tts` deliberately have NO declared key lists.
  // The backend owns the key contract (`config/mutation.rs`); any frontend
  // roster goes stale the moment a key is added or removed — exactly how
  // `llm.server` survived the config refactor as a phantom write. Omitting
  // the entry routes these scopes through the generic whole-scope paths:
  //   isCategoryDirty / isDomainDirty -> deep scope comparison
  //   isDomainRequiringRestart       -> Object.keys(scope) filtered by
  //                                     isRestartKey
  //   discardCategoryChanges / discardDomainChanges -> whole-scope restore
  interaction: ["mode", "pipeline_mode"],
  dictation: ["enabled", "interaction_mode", "hotkey", "output_mode", "silence_auto_stop_ms"],
  working_memory: ["private_mode", "auto_compaction", "max_context_share", "web_search_enabled"],
  personal_memory: [
    "context_retrieval_enabled",
    "pipeline_processing_enabled",
    "top_k_facts",
    "semantic_similarity_cutoff",
    "consolidation_cadence",
    "consolidation_time",
    "suggestion_policy",
  ],
  persona: ["modular_prompt", "realtime_prompt"],
  realtime: [
    "active",
    "active_voice_id",
    "api_keys",
  ],
  system: ["setup_completed"],
};

export interface DomainDirtyKey {
  scope: SettingsScope;
  keys?: readonly string[];
  nestedKey?: string;
}

export const DOMAIN_DIRTY_KEYS: Record<SettingsDomainId, readonly DomainDirtyKey[]> = {
  models: [
    { scope: "vad", keys: SETTINGS_SCOPE_KEYS.vad },
    { scope: "stt", keys: SETTINGS_SCOPE_KEYS.stt },
    { scope: "tts", keys: SETTINGS_SCOPE_KEYS.tts },
    { scope: "llm", keys: SETTINGS_SCOPE_KEYS.llm },
    { scope: "realtime", keys: SETTINGS_SCOPE_KEYS.realtime },
  ],
  working_memory: [
    { scope: "working_memory", keys: SETTINGS_SCOPE_KEYS.working_memory },
  ],
  persona: [
    { scope: "persona", keys: SETTINGS_SCOPE_KEYS.persona },
  ],
  personal_memory: [
    { scope: "personal_memory", keys: SETTINGS_SCOPE_KEYS.personal_memory },
  ],
  appearance: [
    { scope: "appearance", keys: SETTINGS_SCOPE_KEYS.appearance },
  ],
  interaction: [
    { scope: "interaction", keys: SETTINGS_SCOPE_KEYS.interaction },
    { scope: "dictation", keys: SETTINGS_SCOPE_KEYS.dictation },
    { scope: "realtime", keys: SETTINGS_SCOPE_KEYS.realtime },
  ],
};

export const REALTIME_CONFIG_DESK_COPY = {
  title: "Realtime Direct Voice Connection",
  bannerDescPrefix: "Full duplex speech-to-speech engine with ",
  bannerHighlight: "sub-200ms",
  bannerDescSuffix: " live audio streaming, native grounding, and dynamic turn detection.",
  providerLabel: "Voice Provider",
  apiKeyLabel: "API Key (Required)",
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
  saveChanges: "Save",
  discardChanges: "Discard",
  changesSaved: "Changes Saved",
  autoSynced: "Auto-synced",
  saveFailedTitle: "Could Not Save",
  saveFailedHint: "These settings were rejected by the backend and were not applied:",
  saveFailedNone: "The setting was rejected by the backend and was not applied.",
  restartingEngine: "Restarting Engine",
  apiKeyRequired: "API Key Required for Cloud Provider",
  applyAndRestart: "Apply & Restart",
  restoreDefaults: "Restore All Defaults",
  restoreAria: "Restore default settings",
  openDomain: "Open {label} settings",
  closeDomain: "Close {label} settings",
  openAllDomains: "Open all settings",
  closeAllDomains: "Clear all settings",
};

export const DICTATION_COPY = {
  destinationTitle: "Output Destination",
  destinationPasteDesc: "Transcribes and routes to active window ",
  destinationClipboardDesc: "Silently copies final transcript to system clipboard without pasting",
  destinationTrayDesc: "Renders live transcription in floating desktop HUD window",
  hotkeyTitle: "Activation Hotkey",
  hotkeySubtitle: "Global shortcut to trigger voice typing",
  recordingPrompt: "Press shortcut keys...",
  modePaste: "Paste",
  modeClipboard: "Clipboard",
  modeTray: "Tray",
  triggerContinuous: "Continuous",
  triggerPtt: "Push-To-Talk",
  triggerContinuousSub: "Always Transcribing",
  triggerPttSub: "Hold Hotkey",
  voiceTypingActive: "Ready to Transcribe",
  voiceTypingInactive: "System Muted",
  rebindHint: "Click to rebind activation shortcut",
  savedFeedback: "Saved!",
  cancelBtn: "Cancel",
  saveBtn: "Save",
  resetDefault: "Reset (Alt+V)",
  defaultHotkey: "Alt+V",
};

export const CATEGORY_SWITCH_COPY = {
  tabs: {
    stt: "Listening",
    llm: "Reasoning",
    tts: "Speaking",
  },
};

export const PERSONA_COPY = {
  cardTitle: "Persona",
  tabModular: "Modular",
  tabRealtime: "Realtime",
  modularPlaceholder: "Modular instruction prompt...",
  realtimePlaceholder: "Realtime instruction prompt...",
  emptyPrompt: "No instructions defined. Switch to Edit to write prompt directives.",
  tagsProtectedHint: "That edit touches protected system tags, so it wasn't saved.",
};

export const INTERACTION_CONFIG_DESK_COPY = {
  integrated: {
    title: "Integrated Voice Engine",
    description: "Streaming STT, LLM inference, and TTS audio synthesis run tightly coupled in unified process memory for minimal perceived latency.",

  },
  stt: {
    embedded: {
      title: "Embedded Speech Recognition",
      description: "Speech recognition runs 100% locally on-device with zero external network transmission.",
    },
    server: {
      title: "Remote Speech Server",
      description: "Stream microphone audio frames directly to a self-hosted or dedicated remote ASR inference node over secure WebSocket connection.",
    },
    cloud: {
      title: "Cloud Speech API",
      description: "Ultra-low latency streaming cloud transcription powered by hosted speech-to-text API endpoints.",
    },
  },
  llm: {
    embedded: {
      title: "Embedded Neural LLM",
      description: "Neural language models run directly on-device with completely offline and private reasoning.",
    },
    server: {
      title: "Remote Server",
      urlLabel: "Server URL",
      urlPlaceholder: "http://127.0.0.1:11434",
      apiKeyLabel: "API Key (Optional)",
      apiKeyPlaceholder: "Bearer token...",
    },
    cloud: {
      title: "Cloud Provider API",
      providerLabel: "Cloud Provider",
      apiKeyLabel: "API Key (Required)",
      searchPlaceholder: "Search providers...",
      noProvidersMatch: "No providers match your search",
      clearSearch: "Clear search",
      connect: "+ Connect",
      editKey: "Edit API key",
      saveKey: "Save API key",
      cancelKey: "Cancel",
      expandLabel: "Expand provider list",
      expandAriaLabel: "Browse all cloud providers",
      filtersTitle: "Filters",
      filterAria: "Toggle provider filters",
      filterAll: "All Providers",
      filterConfigured: "Available",
      filterPopular: "Popular",
      filterUnconfigured: "Needs Key",
      resetFilters: "Reset Filters",
    },
  },
  status: {
    testing: "Testing",
    offline: "Offline",
    online: "Online",
    active: "Active",
    backToProviders: "Back to providers",
    providers: "Providers",
    connectionFailed: "Connection failed",
  },
  tts: {
    embedded: {
      title: "Embedded Voice Synthesizer",
      description: "Voice generation models execute entirely on-device for crisp, realtime offline audio synthesis with zero cloud latency or external API costs.",
    },
    server: {
      title: "Chatterbox GPU Server",
      description: "Connect to a remote speech synthesis node.",
      hostLabel: "IP / Domain",
      portLabel: "Port",
      urlLabel: "Server HTTP URL",
      urlPlaceholder: "http://127.0.0.1:7860",
      pathLabel: "Remote Path",
      pathPlaceholder: "~/.vox",
    },
    cloud: {
      title: "Cloud Voice Engine",
      description: "Natural multi-voice speech synthesis streamed over the web with high-fidelity pronunciation, global accent selection, and zero API key setup.",
    },

  },
};

export const PERSONAL_MEMORY_CONFIG_DESK_COPY = {
  cardTitle: "Personal Memory",
  recallToggle: {
    title: "Memory Recall",
    activeLabel: "Recall On",
    inactiveLabel: "Recall Off",
    activeSublabel: "Uses past memories in chat",
    inactiveSublabel: "Does not use past memories",
  },
  pipelineToggle: {
    title: "Memory Learning",
    activeLabel: "Learning On",
    inactiveLabel: "Learning Off",
    activeSublabel: "Saves new facts from chat",
    inactiveSublabel: "Pauses saving new facts",
  },
  tabs: {
    consolidation: "Schedule",
    depth: "Limit",
    cutoff: "Relevance",
    suggestions: "Suggestions",
  },
  consolidation: {
    title: "Update Schedule",
    sublabel: "[ 24h format ]",
    description: "How often Vox updates your personal memory notes with new facts learned from conversations.",
    manualLabel: "Manual",
    dailyLabel: "Daily",
    timeFormatHint: "24-hour format (HH:MM)",
    noSchedule: "No automatic schedule",
    nextRunLabel: "Next run",
    todayAt: "Today at",
    tomorrowAt: "Tomorrow at",
    hoursShort: "h",
    minutesShort: "m",
    in: "in",
  },
  depth: {
    title: "Memory Limit",
    description: "Maximum number of relevant memories Vox brings into each conversation.",
    unit: "memories",
  },
  cutoff: {
    title: "Relevance Threshold",
    description: "How closely a memory must match your current conversation to be remembered.",
  },
  suggestions: {
    title: "Suggestion Policy",
    description: "Determine whether extracted memory suggestions require manual review or are automatically applied.",
    manualLabel: "Manual Review",
    autoLabel: "Auto-Apply",
  },
};

export const TTS_VOICE_MANAGER_COPY = {
  tabs: {
    selectVoice: "Select Voice",
  },
  voice: {
    title: "Voice Profile",
    prefix: "Choose an AI voice persona from the",
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

export const CATALOG_HEADER_COPY = {
  back: "Back",
  addProviderSoon: "Add provider (coming soon)",
  sortAsc: "Sort A–Z",
  sortDesc: "Sort Z–A",
  search: "Search",
  closeSearch: "Close search",
  clearSearch: "Clear search",
} as const;

export const VOICE_CATALOG_COPY = {
  title: "Select a Voice",
  subtitle: "Browse and audition natural voices for speech synthesis.",
  catalogAriaLabel: "Browse all available voices",
  expandLabel: "Expand voice catalog",
  searchPlaceholder: "Search voices...",
  searchAria: "Search voices",
  allVoices: "All Voices",
  recent: "Recent",
  favorites: "Favorites",
  genderFacet: "Gender",
  female: "Female",
  male: "Male",
  neutral: "Neutral",
  accentFacet: "Accent / Language",
  english: "English",
  multilingual: "Multilingual",
  fallbackDescription: "On-device neural voice for natural speech synthesis.",
  emptyTitle: "No voices found",
  emptyHint: "No available voices match your current search and filter criteria.",
  resetFilters: "Reset Filters",
  cancel: "Cancel",
  selectVoice: "Select Voice",
  showingOf: "voices",
  previewUnavailableTitle: "Preview is unavailable while a session is active",
  cloned: "Cloned",
  cloneVoice: "Clone Voice",
  cloneVoiceChatterboxNote: "Voice cloning builds a Chatterbox reference voice",
  edgeSyncHint: "Cloud voices sync in the background when online.",
  providersFacet: "Providers",
  languageFacet: "Language",
  filterAria: "Toggle voice filters",
  filtersTitle: "Filters",
} as const;

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
  catalogTitle: "Available Models",
  catalogAriaLabel: "Browse all available models",
  expandLabel: "Expand model list",
  modelCountLabel: "models",
  serverErrorTooltip: "Connection failed or invalid API key. Ensure the server is online and credentials are correct.",
  fetching: "Fetching...",
  searchPlaceholder: "Filter models with fuzzy matching (e.g. llama 70b, gemma q4)...",
  emptyTitle: "No remote models loaded",
  emptyHint: "Ensure the remote server is online and configured in the Interaction Card.",
  clearSearch: "Clear Search",
  noModelsMatch: "No models matching",
  toolsSupported: "Supported",
  toolsNone: "None",
  toolsUnknown: "Unknown",
  languagesUnknown: "Unknown",
  managed: "Managed",
  gpuBadge: "🚀 GPU",
  cpuBadge: "⚠️ CPU",
  rerunBenchmark: "Re-run capability benchmark",
  runBenchmark: "Run capability benchmark",
  benchmark: "Benchmark",
  reprobe: "Re-probe",
  benchmarking: "Benchmarking...",
  untested: "Untested",
  capabilities: "Capabilities",
  modelCapabilities: "Model Capabilities",
  speed: "Speed:",
  context: "Context:",
  vram: "VRAM:",
  tools: "Tools:",
  languages: "Languages:",
  sourceRow: "Source:",
  provenanceMeasured: "Measured live",
  provenanceCatalog: "Catalog baseline",
  provenanceDeclared: "Declared static",
  provenanceFamily: "Family estimate",
  provenanceUser: "Your setting",
  provenanceUnknown: "Unknown source",
  probeFailedNote: "Probe failed",
  filtersTitle: "Filters",
  filterAria: "Toggle model filters",
  filterAll: "All Models",
  filterFavorites: "Favorites",
  filterBenchmarked: "Benchmarked",
  filterTools: "Tools Support",
  filterFast: "Fast (>30 TPS)",
  resetFilters: "Reset Filters",
} as const;

export const REMOTE_SERVER_COPY = {
  bannerTitle: "Chatterbox Remote Deployment",
  bannerBody:
    "Deploy Chatterbox on a remote CUDA-accelerated GPU host (e.g. RunPod, Vast.ai, or homelab) to offload memory-intensive flow-matching voice synthesis. Enter your SSH connection info below to automatically sync the codebase, download GGUF models, and run the server.",
  panelTitle: "Remote GPU Server",
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
  tabs: {
    vad: "Voice Detection",
    vadShort: "Detection",
    stt: "Listening",
    sttShort: "Listening",
    llm: "Reasoning",
    llmShort: "Reasoning",
    tts: "Speaking",
    ttsShort: "Speaking",
    auxiliary: "Support",
    auxiliaryShort: "Support",
  },
  missing: "Missing",
  notDownloadedDesc: "This model file is not downloaded yet.",
  /** Shown under a server/cloud LLM chip, whose weights are not local. */
  remoteLlmDesc:
    "Running against a remote provider. The saved model identifier is shown below.",
  row: {
    cancel: "Cancel",
    confirmDelete: "Confirm Delete",
    deleteWeights: "Delete weights",
    deleteConfirm: "Delete?",
    downloadModel: "Download model",
    mandatoryNote: "Mandatory core model (cannot be deleted)",
    required: "Required",
    builtIn: "Built in",
    builtInNote: "Built into the app binary (cannot be deleted)",
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
    description: "Allocate local CPU worker threads for in-process model reasoning.",
    remoteTitleLocal: "Cloud Infrastructure",
    remoteTitleRemote: "Remote Compute",
    remoteDescription:
      "Inference computation and KV-cache allocation are offloaded entirely to the remote endpoint.",
  },
  tokens: {
    title: "Token Limit",
    native: "Native",
    description: "Maximum output token budget per conversational turn.",
  },
  context: {
    title: "Context Window",
    description:
      "Token budget allocated for conversation history, system persona instructions, and memory facts.",
  },
  creativity: {
    title: "Creativity",
    description: "Controls randomness and creativity of generated text responses.",
  },
};

export const APPEARANCE_COPY = {
  cardTitle: "Appearance",
  copyHex: "Copy hex code",
  copiedHex: "Copied",
} as const;

export const WORKING_MEMORY_SETTINGS_COPY = {
  cardTitle: "Working Memory",
  tabs: {
    webSearch: "Web Search",
    contentShare: "Content Share",
  },
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
  webSearch: {
    title: "Web Search",
    description:
      "Let Vox search the live web for current facts mid-conversation. Keyless built-in engine, Modular sessions only.",
    enabled: "Enabled",
    disabled: "Disabled",
    toggleAria: "Toggle web search",
  },
  contentShare: {
    title: "Context Share Budget",
    description:
      "Percentage of LLM context window reserved for personal profile and working dialogue history.",
  },
};

export const VAD_SETTINGS_COPY = {
  tabs: {
    sensitivity: "Sensitivity",
    silence: "Silence Cutoff",
    speechOnset: "Speech Onset",
    noiseGate: "Noise Gate",
  },
  sensitivity: {
    title: "Voice Sensitivity",
    description: "Speech detection probability threshold for microphone capture.",
  },
  silence: {
    title: "Silence Cutoff",
    description: "Pause duration before a speech turn is finalized and response reasoning begins.",
  },
  speechOnset: {
    title: "Speech Onset Duration",
    description: "Minimum continuous voice duration required before initiating an active turn.",
  },
  noiseGate: {
    title: "Noise Gate Floor",
    description: "Energy floor threshold to filter steady background noise before detection.",
  },
};

export const STT_SETTINGS_COPY = {
  tabs: {
    streamingRate: "Streaming Rate",
    transliteration: "Transliterate",
  },
  compute: {
    title: "Compute Allocation",
    description: "Dedicated CPU worker threads allocated for speech recognition inference.",
  },
  streamingRate: {
    title: "Subtitle Cadence",
    description: "Update frequency for interim real-time transcription streaming.",
  },
  transliteration: {
    title: "Script Transliteration",
    description: "Converts recognized regional scripts into phonetic Latin text in real-time.",
  },
};

export const PROVIDERS_HEADER_COPY = {
  customButton: "+ Custom",
  customAria: "Configure custom server endpoint",
  comingSoon: "Coming soon",
  customConfigTitle: "Custom Server Endpoint",
  customConfigSubtitle: "Connect to a self-hosted or dedicated inference node",
  filterAria: "Toggle provider filters",
  sortAsc: "Sort A to Z",
  sortDesc: "Sort Z to A",
  searchPlaceholder: "Search providers...",
  back: "Back",
  closeSearch: "Close search",
  clearSearch: "Clear search",
  search: "Search",
} as const;

export const PIPELINE_PROVIDERS_DESK_COPY = {
  sectionTitle: "Inference Pipeline",
  sectionSubtitle: "Configure stages for listening, reasoning, and speaking.",
  configureAriaPrefix: "Configure",
  cards: {
    stt: {
      title: "Speech Recognition",
      shortLabel: "Listening",
      role: "Acoustic Hearing",
      defaultOnDevice: "On-Device",
      cloud: "Cloud Speech",
    },
    llm: {
      title: "Intelligence",
      shortLabel: "Reasoning",
      role: "Knowledge & Logic",
      defaultOnDevice: "On-Device GGUF",
      server: "Custom Server",
      cloud: "Cloud Provider",
    },
    tts: {
      title: "Voice Synthesis",
      shortLabel: "Speaking",
      role: "Speech Generation",
      defaultOnDevice: "Built-in Voices",
      remote: "Remote GPU",
      cloud: "Cloud Voices",
    },
  },
} as const;

export const PROVIDER_GUIDE_COPY = {
  guideTitle: "Provider & Connection Guide",
  guideSubtitle: "Understand how Vox connects to local AI models, private servers, and cloud gateways.",
  backToProviders: "Back to Providers",
  sections: [
    {
      id: "provider",
      title: "What is a Provider?",
      description:
        "A provider is an execution backend that powers one stage of Vox. It can run 100% locally on your machine with zero external network calls, or connect over the internet to high-throughput cloud endpoints.",
    },
    {
      id: "url",
      title: "What is an Endpoint URL?",
      description:
        "An endpoint URL is the network address where the AI model accepts requests. Cloud providers use secure web addresses like https://api.openai.com/v1. Local servers running on your machine use local loopback URLs like http://127.0.0.1:11434.",
    },
    {
      id: "port",
      title: "What is a Port Number?",
      description:
        "A port number identifies the specific communication channel on your host. Standard ports include 11434 for Ollama, 8000 for vLLM or LocalAI, and 7860 for Chatterbox or Gradio speech servers.",
    },
    {
      id: "apikey",
      title: "What is an API Key?",
      description:
        "An API key is your personal secret token issued by the provider. It authorizes Vox to generate tokens or synthesis on your account. Keys are saved locally on your computer with restricted 0600 file permissions and never shared.",
    },
  ],
  steps: [
    {
      number: "1",
      title: "Select Your Pipeline Stage",
      body: "Choose whether you are configuring Speech-to-Text (STT), Intelligence (LLM), or Voice Output (TTS).",
    },
    {
      number: "2",
      title: "Pick On-Device or Remote",
      body: "Keep the first On-Device option for total privacy with zero setup, or tap '+ Custom' to point Vox at your local Ollama or GPU server.",
    },
    {
      number: "3",
      title: "Paste Credentials & Connect",
      body: "Enter your endpoint URL and API key if required. The status indicator will verify when your endpoint is online.",
    },
  ],
} as const;


