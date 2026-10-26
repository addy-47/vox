import type { LucideIcon } from "lucide-react";
import { AudioLines, BookOpenText, BrainCircuit, CircleUserRound, Mic, Palette, Radio, SlidersHorizontal } from "lucide-react";

export type SettingsDomainId =
  | "persona"
  | "speech"
  | "memory"
  | "appearance"
  | "language"
  | "interaction";

export interface SettingsDomain {
  id: SettingsDomainId;
  label: string;
  sublabel: string;
  icon: LucideIcon;
  angle: number;
}

/**
 * Returns dynamic domain descriptor list reflecting modular vs realtime pipeline mode.
 */
export const getSettingsDomains = (isRealtime = false): SettingsDomain[] => [
  { id: "persona", label: "Persona", sublabel: "Prompts & identity", icon: CircleUserRound, angle: -90 },
  { id: "memory", label: "Memory", sublabel: "Recall, facts & compaction", icon: BookOpenText, angle: -30 },
  {
    id: "language",
    label: isRealtime ? "Realtime Model" : "Language Model",
    sublabel: isRealtime ? "Gemini Live & Deepgram Agent" : "Reasoning engine & providers",
    icon: isRealtime ? Radio : BrainCircuit,
    angle: 30,
  },
  { id: "appearance", label: "Appearance", sublabel: "Visual theme & colors", icon: Palette, angle: 90 },
  {
    id: "speech",
    label: isRealtime ? "Voices" : "Speech Models",
    sublabel: isRealtime ? "Real-time voice catalog" : "Voice, transcription & synthesis",
    icon: isRealtime ? Mic : AudioLines,
    angle: 150,
  },
  { id: "interaction", label: "Interaction", sublabel: "Activation & web search", icon: SlidersHorizontal, angle: -150 },
];

/**
 * Clock order. The array index is the slot address used by the radial hub:
 * [0] 12:00 Persona, [1] 02:00 Memory, [2] 04:00 Language, [3] 06:00 Appearance,
 * [4] 08:00 Speech (`speech`), [5] 10:00 Interaction.
 */
export const SETTINGS_DOMAINS: SettingsDomain[] = getSettingsDomains(false);

export type SettingsScope =
  | "appearance"
  | "vad"
  | "stt"
  | "llm"
  | "tts"
  | "interaction"
  | "dictation"
  | "memory"
  | "persona"
  | "realtime"
  | "system";

/**
 * @deprecated Use `settingsContractService.getSupportedKeys(scope)` or backend contract `supported_keys`.
 * Maintained as static fallback during bootstrap.
 */
export const SETTINGS_SCOPE_KEYS: Partial<Record<SettingsScope, readonly string[]>> = {
  appearance: ["theme", "accent_seed"],
  // vad / stt / llm / tts mirror the backend contract
  // (`config::mutation::supported_keys_for_domain`), the SSOT for the keys a
  // scope accepts. Without them the dirty check fell back to a whole-scope
  // compare, so any backend-side field change flagged the owning card.
  vad: ["threshold", "ptt_noise_gate", "silence_duration_ms", "speech_onset_ms", "vad_backend"],
  stt: ["active", "last_selected", "transliterate_enabled"],
  llm: ["active", "last_selected", "servers", "cloud_keys"],
  // The TTS scope carries three real keys (active, last_selected, active_voice_id)
  // plus the custom-server inventory. Provider ids are never listed here: the
  // frontend must not hardcode a provider identity (design invariant 6).
  tts: ["active", "last_selected", "active_voice_id", "servers"],
  interaction: ["mode", "pipeline_mode", "web_search_enabled"],
  dictation: ["enabled", "interaction_mode", "hotkey", "output_mode", "silence_auto_stop_ms"],
  memory: [
    "context_retrieval_enabled",
    "pipeline_processing_enabled",
    "private_mode",
    "consolidation_cadence",
    "consolidation_time",
    "suggestion_policy",
    "auto_compaction",
    "max_context_share",
    "top_k_facts",
    "semantic_similarity_cutoff",
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

/**
 * The VAD keys a user can edit. `ptt_noise_gate` (removed from the UI) and
 * `vad_backend` (engine-owned) are excluded on purpose: a card that owns the
 * vad scope must never report dirty for them.
 */
const VAD_USER_FACING_KEYS = [
  "threshold",
  "silence_duration_ms",
  "speech_onset_ms",
] as const;

/** The STT keys the models card owns: model selection only. */
const STT_MODEL_KEYS = ["active", "last_selected"] as const;

/** The STT key the interaction card owns: script transliteration. */
const TRANSLITERATION_KEYS = ["transliterate_enabled"] as const;

export const DOMAIN_DIRTY_KEYS: Record<SettingsDomainId, readonly DomainDirtyKey[]> = {
  // The speech card owns model selection for every stage, plus the detection
  // knobs it renders behind the voice-detection gear.
  speech: [
    { scope: "stt", keys: STT_MODEL_KEYS },
    { scope: "tts", keys: SETTINGS_SCOPE_KEYS.tts },
    { scope: "realtime", keys: SETTINGS_SCOPE_KEYS.realtime },
    { scope: "vad", keys: VAD_USER_FACING_KEYS },
  ],
  // The language card owns the reasoning engine: model selection, the custom
  // server inventory, and the cloud provider keys.
  language: [
    { scope: "llm", keys: SETTINGS_SCOPE_KEYS.llm },
  ],
  memory: [
    { scope: "memory", keys: SETTINGS_SCOPE_KEYS.memory },
  ],
  persona: [
    { scope: "persona", keys: SETTINGS_SCOPE_KEYS.persona },
  ],
  appearance: [
    { scope: "appearance", keys: SETTINGS_SCOPE_KEYS.appearance },
  ],
  interaction: [
    { scope: "interaction", keys: SETTINGS_SCOPE_KEYS.interaction },
    { scope: "dictation", keys: SETTINGS_SCOPE_KEYS.dictation },
    { scope: "realtime", keys: SETTINGS_SCOPE_KEYS.realtime },
    // Script transliteration is an interaction-card control: the speech card
    // owns the stt model selection only.
    { scope: "stt", keys: TRANSLITERATION_KEYS },
  ],
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
  restartingEngine: "Restarting engine...",
  apiKeyRequired: "API Key Required for Cloud Provider",
  /** Detail line for the missing-credential banner. */
  apiKeyRequiredDetail:
    "Enter and save an API key for the selected provider to enable cloud inference.",
  /** A cloud/server provider is selected but no model is. Committing would send
   * an empty model id to the engine, so the commit is blocked instead. */
  modelRequired: "Select a Model",
  modelRequiredDetail:
    "This provider has no model selected. Pick one from its catalog before applying.",
  applyAndRestart: "Apply & Restart",
  /** Icon-only variant of the commit button (small / mobile layouts). */
  restartOnly: "Restart",
  restoreDefaults: "Restore All Defaults",
  restoreAria: "Restore default settings",
  restorePrompt: "Restore settings",
  confirmRestore: "Confirm restore",
  confirmRestoreAria: "Confirm restore defaults",
  cancel: "Cancel",
  cancelRestore: "Cancel",
  cancelRestoreAria: "Cancel restore defaults",
  openDomain: "Open {label} settings",
  closeDomain: "Close {label} settings",
  openAllDomains: "Open all settings",
  closeAllDomains: "Clear all settings",
  /** Trailing CTA on the dashed `AddCard` tile variant. */
  addCardAction: "+ Configure",
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
  tabsAriaLabel: "Dictation configuration tabs",
  manualReleaseOnly: "Manual release only",
  autoCommitActive: (seconds: string | number) => `Auto-commit active (${seconds}s)`,
  toggleAutoStopAria: (enabled: boolean) => enabled ? "Disable silence auto-stop" : "Enable silence auto-stop",
  toggleAutoStopTooltip: (enabled: boolean) => enabled ? "Click to disable auto-stop" : "Click to enable auto-stop",
  autoStopDurationAria: "Silence auto-stop duration in seconds",
  autoStopDurationPlaceholder: "1.2",
  pttActive: (hotkey: string) => `Push-to-Talk active (${hotkey})`,
  continuousActive: (hotkey: string) => `Continuous / Passive active (${hotkey})`,
};

export const PERSONA_COPY = {
  cardTitle: "Persona",
  tabModular: "Modular",
  tabRealtime: "Realtime",
  modularPlaceholder: "Modular instruction prompt...",
  realtimePlaceholder: "Realtime instruction prompt...",
  emptyPrompt: "No instructions defined. Switch to Edit to write prompt directives.",
  tagsProtectedHint: "That edit touches protected system tags, so it wasn't saved.",
  doneEditing: "Done editing",
  editPersona: "Edit persona",
  cancelEditing: "Cancel editing",
};

export const PROVIDERS_CONFIG_DESK_COPY = {
  /** Header filter popover in the provider catalog. */
  filterLabel: "Filter providers",
  filterMenuTitle: "Provider Filters",
  reset: "Reset",
  localMeta: {
    STT: { name: "On-Device Speech Recognition", tagline: "High-accuracy local Whisper & Nemotron transcription. 100% offline." },
    LLM: { name: "On-Device Model", tagline: "Local GGUF neural reasoning on CPU or GPU. Zero telemetry, private." },
    TTS: { name: "On-Device Neural Voices", tagline: "Local Kokoro neural synthesis with peak normalization and zero latency." },
  },
  badges: {
    localHardware: "Local Hardware",
    active: "Active",
    offlineReady: "Offline Ready",
    keylessCloud: "Keyless Cloud",
    cloudVoice: "Cloud Voice",
    cloudGateway: "Cloud Gateway",
    selfHosted: "Self-Hosted",
  },
  actions: {
    editServer: "Edit Server",
    deleteServer: "Delete Server",
    deleteProvider: "Delete provider",
  },
  /** Dashed add-card copy in the provider catalog. */
  creationCard: {
    addVoiceServer: "Add Voice Server",
    addCustomServer: "Add Custom Server",
    voiceServerDescription: "Connect Chatterbox GPU node or remote synthesis host.",
    customServerDescription: "Connect self-hosted Ollama, vLLM, or LM Studio endpoint.",
    selfHostedFootnote: "Self-Hosted",
  },
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
      description: "Stream microphone audio frames directly to a self-hosted or dedicated remote Speech-to-Text inference node over secure WebSocket connection.",
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
      urlPlaceholder: "http://localhost:11434",
      apiKeyLabel: "API Key (Optional)",
      apiKeyPlaceholder: "Bearer token...",
      nameLabel: "Server Name (optional)",
      namePlaceholder: "Auto-detect (e.g. Ollama, vLLM, LM Studio)",
      namePlaceholderAuto: "Name (Auto)",
      probingState: "Probing endpoint...",
      autoDiscovery: "2s auto-discovery",
      save: "Save",
      discard: "Discard",
      saveServer: "Save Server",
    },
    cloud: {
      title: "Cloud Provider API",
      providerLabel: "Cloud Provider",
      apiKeyLabel: "API Key (Required)",
      searchPlaceholder: "Search providers...",
      noProvidersMatch: "No providers match your search",
      clearSearch: "Clear search",
      connect: "+ Connect",
      connectKey: "+ Connect Key",
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
    inDevelopment: "In Development",
    ready: "Ready",
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

export const WEB_SEARCH_COPY = {
  tab: "Web Search",
  title: "Live Web Grounding",
  description:
    "Retrieve live web results and current facts mid-conversation.",
  descriptionModular:
    "Retrieve live web results and current facts during modular LLM generation.",
  descriptionRealtime:
    "Stream live web results and current facts directly into your realtime voice session.",
  enabled: "Enabled",
  disabled: "Disabled",
  toggleAria: "Toggle live web search",
};

export const REMOTE_CONTROL_COPY = {
  tab: "Remote Control",
  title: "Mobile Companion",
  description:
    "Use your phone as a wireless mic, speaker, and live assistant HUD over Wi-Fi.",
  descriptionReady:
    "Ready to connect. Pair an Android device on your local Wi-Fi network.",
  comingSoon: "Pair Device",
  statusDisconnected: "Disconnected",
  statusSearching: "Searching for Controllers",
  statusConnected: "Controller Connected",
  disconnect: "Disconnect",
  connectedTo: "Connected to",
  wirelessAudioActive: ". Wireless audio and session control are active.",
  wirelessMicActive: ". Wireless microphone and session control active.",
  pairNewDevice: "Pair new device +",
  revokeDevice: "Revoke device",
  revokeDeviceAria: (name: string) => `Revoke ${name}`,
  revokeHost: "Revoke host",
  revokeHostAria: (name: string) => `Revoke ${name}`,
  scanQrPrompt: "Scan QR Code to Pair",
  scanQrButton: "Scan QR",
  connectedHost: "Connected Host",
  disconnectHostAria: "Disconnect from host",
  pairDeviceScreenPrompt: "Pair this device with your desktop screen to control Vox wirelessly.",
  enterIpManually: "Enter IP manually",
  linked: "Linked",
  revokeConfirmPrompt: "Revoke?",
  revokeConfirmAria: "Confirm revocation",
  revokeCancelAria: "Cancel revocation",
  disconnectConfirmPrompt: "Disconnect?",
  disconnectConfirmAria: "Confirm disconnect",
  disconnectCancelAria: "Cancel disconnect",
  successTitleHost: "Controller Paired & Connected",
  successSubtitleHost: (name: string) => `${name} is linked as remote controller.`,
  successTitleController: "Connected to Desktop Host",
  successSubtitleController: "Local microphone and speaker are ready.",
  errorTimeout:
    "Connection timed out. The computer did not respond. Check if both devices are on the same Wi-Fi and VPNs or firewalls are not blocking port 7700.",
  errorUnreachable:
    "Cannot reach desktop host. Ensure Vox is running on your computer.",
  errorRejected:
    "Pairing request was declined by the desktop host.",
  errorInvalidToken:
    "The pairing QR code has expired or is invalid. Please generate a new QR code on your computer.",
  errorVersionMismatch:
    "Protocol version mismatch between phone and computer.",
  errorUnknown: (detail: string) => `Connection failed: ${detail}`,
  errorGeneric: "Connection failed. Please check network and retry.",
} as const;

export const MEMORY_CARD_COPY = {
  cardTitle: "Memory",
  viewBasic: "Basic",
  viewAdvanced: "Advanced",
  tiles: {
    recall: {
      title: "Memory Recall",
      activeLabel: "Recall On",
      inactiveLabel: "Recall Off",
      activeSublabel: "Uses past memories in chat",
      inactiveSublabel: "Does not use past memories",
    },
    learning: {
      title: "Memory Learning",
      activeLabel: "Learning On",
      inactiveLabel: "Learning Off",
      activeSublabel: "Saves new facts from chat",
      inactiveSublabel: "Pauses saving new facts",
    },
    autoCompaction: {
      title: "Auto Compaction",
      activeLabel: "Auto Active",
      inactiveLabel: "Manual Review",
      activeSublabel: "Summarizes on idle",
      inactiveSublabel: "Prompt on uncompacted",
    },
  },
  tabs: {
    sessionStorage: "Storage",
    schedule: "Schedule",
    suggestions: "Suggestions",
    context: "Context",
    limit: "Limit",
    relevance: "Relevance",
  },
  sessionStorage: {
    title: "Session Storage",
    description:
      "When on, nothing from your chats is written to disk — no audio, no transcripts, no memories.",
    enabledLabel: "Incognito",
    disabledLabel: "Logged",
    enabledSub: "No traces saved",
    disabledSub: "Persistent Storage",
  },
  schedule: {
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
  limit: {
    title: "Memory Limit",
    description: "Maximum number of relevant memories Vox brings into each conversation.",
    unit: "memories",
  },
  relevance: {
    title: "Relevance Threshold",
    description: "How closely a memory must match your current conversation to be remembered.",
  },
  suggestions: {
    title: "Suggestion Policy",
    description: "Determine whether extracted memory suggestions require manual review or are automatically applied.",
    manualLabel: "Manual",
    autoLabel: "Auto-Apply",
  },
  context: {
    title: "Context Share Budget",
    description:
      "Percentage of LLM context window reserved for personal profile and working dialogue history.",
  },
};

export const TTS_VOICE_MANAGER_COPY = {
  tabs: {
    selectVoice: "Select Voice",
    voice: "Voice",
    speed: "Speech Rate",
  },
  voice: {
    title: "Voice Profile",
    prefix: "Choose an AI voice persona from the",
  },
  execution: {
    embedded: "Synthesized on-device.",
    cloud: "Synthesized in the cloud.",
    remote: "Synthesized on your own server.",
  },
  speed: {
    title: "Speech Rate",
    description: "Fine-tune speech synthesis playback tempo and velocity across assistant responses without pitch distortion.",
  },
  region: {
    previous: "Previous region",
    next: "Next region",
  },
  cloning: {
    processing: "Processing...",
    submit: "Clone Voice",
    tabRecord: "Record Audio",
    tabUpload: "Upload WAV",
    recordAgain: "Record Again",
    namePlaceholder: "e.g. My Voice",
  },
};

export const CARD_HEADER_COPY = {
  back: "Back",
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
  seekAria: "Seek preview audio",
  seekTitle: "Click or arrow keys to seek",
  customPromptPlaceholder: "Type phrase to synthesize...",
  applyPrompt: "Apply",
  cancelPrompt: "Cancel editing",
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
  noVoicesAvailable: "No voices available for this provider. Configure an API key or check provider connection.",
  noVoicesMatchFilters: "No voices match the selected filters.",
  noVoicesMatchSearch: (query: string) => `No voices match "${query}" or active filters.`,
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
  cloneVoiceRecord: "Record Audio",
  cloneVoiceStop: "Stop recording",
  cloneVoiceUpload: "Upload WAV",
  cloneVoiceRecordingNotice: "Speak clearly into your microphone...",
  cloneVoiceSampleRecorded: "Sample recorded",
  cloneVoiceRecordAgain: "Record Again",
  cloneVoiceReadyPrompt: "Read a short paragraph clearly (10–30s).",
  cloneVoiceSelectFile: "Select WAV Audio File",
  cloneVoiceChangeFile: "Change WAV",
  cloneVoiceNamePlaceholder: "Voice name...",
  cloneVoiceSubmit: "Clone Voice",
  cloneVoiceProcessing: "Cloning...",
  cloneVoiceNameRequired: "Please enter a voice name",
  cloneVoiceAudioRequired: "Please record or select audio first",
  cloneVoiceCancel: "Cancel",
  cloneVoiceAudioHint: "Record 5–30s or choose a WAV file",
  cloneVoicePcmFormat: "Chatterbox reference",
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
  copyModelId: "Copy Model ID",
  copiedModelId: "Copied Model ID",
  toolsSupported: "Supported",
  toolsNone: "None",
  toolsUnknown: "Unknown",
  languagesUnknown: "Unknown",
  managed: "Managed",
  managedContext: "Managed",
  computeTypes: {
    gpuAccel: "GPU Accel",
    serverGpu: "Server GPU",
    standard: "Standard",
  },
  metricTiles: {
    speed: "Speed",
    context: "Context",
    compute: "Compute",
    tools: "Tools",
  },
  filterTitle: "Model Filters",
  filterReset: "Reset",
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
  modalSearchPlaceholder: "Search models...",
  modalEmptyTitle: "No models found",
  modalEmptyHint: "No available models match your current search & filter criteria.",
  providerModalEmptyTitle: "No providers found",
  providerModalEmptyHint: "No providers match your search query or filter.",
  /** Title of the modal's providers (cloud connectors) view. */
  providersViewTitle: "Cloud Providers",
  /** Count line in the providers view: "Connectors · 5". */
  connectorsLabel: "Connectors",
  modelsModalSubtitle: "Explore, probe, and select remote models for conversation.",
  providersModalSubtitle: "Configure external AI model endpoints and authentication keys.",
  onDeviceTitle: "On-device models",
  noLocalModelsTitle: "No local models installed",
  noLocalModelsHint: "Download a model from the speech card or the setup guide.",
  badgeBuiltIn: "Built-in",
  badgeOnDevice: "On-device",
} as const;

/** The three LLM knobs, shown as glyphs in the model catalog header. */
export const LLM_TUNING_COPY = {
  custom: "Custom",
  responseLength: {
    title: "Response length",
    hint: "Tokens per reply. Native lets the model decide how long it answers.",
    native: "Native",
  },
  context: {
    title: "Context window",
    hint: "How much the model can hold at once. A hosted provider sets its own limit.",
    serverManaged: "Server managed",
  },
  creativity: {
    title: "Creativity",
    hint: "Higher values vary the wording more. Lower values keep answers plainer.",
  },
} as const;

/** Shared by every card catalog that can reveal its full list. */
export const CATALOG_REVEAL_COPY = {
  /**
   * Expands the catalog into its modal. "Expand" matches the maximize glyph:
   * the list is not truncated, so nothing is "shown": the view grows.
   */
  expand: "Expand",
} as const;

export const LANGUAGE_CARD_COPY = {
  title: "Language Model",
  selectModel: "Select model",
  providersAria: "Language providers",
  providerModalTitle: "Language providers",
  providerModalSubtitle: "Connect a hosted provider or register a self-hosted endpoint.",
  expandProviders: "Expand language providers",
  browseModels: "Browse models",
  backToProviders: "Back to providers",
  providerCount: "providers",
  needsKey: "Cloud · needs key",
  keySaved: "Cloud · key saved",
  onDevice: "On-device",
  addProvider: "Add provider",
  localPrivate: "Local GGUF · private",
  saveKey: "Save key",
  cancel: "Cancel",
  /** Row meta prefixes, before the " · " separator. */
  metaLocal: "Local",
  metaSelfHosted: "Self-hosted",
  metaCloud: "Cloud",
  /** Inline key editor placeholder/aria, appended after the provider name. */
  keyPlaceholder: "API key",
  /** Fallback name for an unnamed custom server row. */
  customServerFallback: "Custom Server",
} as const;

export const SPEECH_CARD_COPY = {
  title: "Speech Models",
  topologyAria: "Speech pipeline",
  selectVoice: "Select voice",
  openVoices: "Open voices",
  cloud: "Cloud",
  tierOnDevice: "On-device",
  tierCloud: "Cloud",
  tierBuiltIn: "Built-in",
  sourceFacet: "Source",
  facetAll: "All",
  facetLocal: "On-device",
  facetCloud: "Cloud",
  ttsModelsTitle: "Text to speech models",
  ttsModelsSubtitle: "Seven engines. Pick one, then open its voices.",
  ttsModelsAria: "Browse text to speech engines",
  expandTts: "Expand text to speech models",
  /** Header of the engine flow banner: text in, speech out. */
  engineFlow: {
    title: "Text in, speech out",
    caption: "Words are spoken by the selected engine, one voice at a time.",
    words: ["hello", "this", "is", "a", "preview", "of", "my", "voice"],
  },
  expandVoices: "Expand voice catalog",
  vadSettings: "Detection settings",
  backToVadModels: "Back to detection models",
  tabs: {
    vad: "Voice detection",
    vadShort: "Detection",
    stt: "Transcription",
    sttShort: "Transcription",
    tts: "Text to speech",
    ttsShort: "Speech",
  },
  tabCaptions: {
    tts: "Local and cloud voices",
    stt: "Speech to text, on-device",
    vad: "Find where speech starts",
  },
} as const;

export const MODEL_HUB_COPY = {
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
    /** Footprint label on the model row meta line. */
    ramLabel: "RAM",
  },
};

export const DIRTY_STATE_COPY = {
  category: "Unsaved changes in this category",
  stage: "Unsaved changes in this stage",
} as const;

/** Placeholder for numeric inputs that accept a value outside the presets. */
export const CUSTOM_VALUE_COPY = {
  placeholder: "Custom",
} as const;

export const APPEARANCE_COPY = {
  cardTitle: "Appearance",
  copyHex: "Copy hex code",
  copiedHex: "Copied",
} as const;

export const VAD_SETTINGS_COPY = {
  tabs: {
    sensitivity: "Sensitivity",
    silence: "Silence Cutoff",
    speechOnset: "Speech Onset",
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
};

export const TRANSLITERATION_COPY = {
  tab: "Transliteration",
  title: "Phonetic Script Conversion",
  description: "Converts recognized non-Latin scripts into phonetic text in real-time.",
  enabled: "Enabled",
  disabled: "Disabled",
  toggleAria: "Toggle phonetic script conversion",
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


