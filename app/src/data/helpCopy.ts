export type HelpTier = "1A" | "1B" | "2A" | "2B" | "3";

export type HelpGroup = "page" | "settings" | "wizard" | "faq";

export interface HelpShortcut {
  keys: string;
  label: string;
}

export interface HelpSection {
  heading: string;
  paragraphs?: readonly string[];
  bullets?: readonly string[];
  shortcuts?: readonly HelpShortcut[];
  tip?: { title: string; body: string };
}

export interface HelpTip {
  tier: HelpTier;
  title: string;
  body: string;
}

export interface HelpArticle {
  id: string;
  group: HelpGroup;
  title: string;
  pinnedFrom?: string;
  visibleOnTiers?: readonly HelpTier[];
  tips?: readonly HelpTip[];
  sections: readonly HelpSection[];
}

export const HELP_TOC_GROUPS: readonly { id: HelpGroup; label: string }[] = [
  { id: "page", label: "Pages" },
  { id: "settings", label: "Settings" },
  { id: "wizard", label: "First-time setup" },
  { id: "faq", label: "Quick answers" },
];

export const HELP_ARTICLES: readonly HelpArticle[] = [
  {
    id: "page:home",
    group: "page",
    title: "Home",
    pinnedFrom: "Home",
    sections: [
      {
        heading: "What this screen is",
        paragraphs: [
          "Home is the central ambient surface. The animated orb reacts to the voice pipeline in real time and reflects what Vox is doing under the hood.",
        ],
      },
      {
        heading: "How to start a session",
        bullets: [
          "Tap the orb, or use the activation hotkey from Settings to engage Vox.",
          "Speak naturally. The orb shifts to listening the moment your voice is detected.",
          "Stop talking for a beat and Vox responds in turn.",
        ],
      },
      {
        heading: "Reading the orb's mood",
        bullets: [
          "Calm glow: idle or sleeping.",
          "Bright pulse: actively listening to you.",
          "Swirling center: thinking and preparing a response.",
          "Expanding ripples: speaking back to you.",
        ],
      },
      {
        heading: "Controls around the orb",
        bullets: [
          "Bottom nav switches between Home, History, Memory, and Settings.",
          "Monitoring popover on the bottom-left shows live system vitals.",
          "Profiler opens a memory snapshot of the running app.",
        ],
      },
    ],
  },
  {
    id: "page:history",
    group: "page",
    title: "History",
    pinnedFrom: "History",
    sections: [
      {
        heading: "What lives here",
        paragraphs: [
          "Every past conversation is kept as a session card. Open one to read the full transcript, copy useful passages, or resume the thread.",
        ],
      },
      {
        heading: "Reading a session",
        bullets: [
          "Your words appear on the left, Vox's replies on the right.",
          "Tap any card to open it in the bottom drawer.",
          "Use the search bar to find a session by keyword.",
        ],
      },
      {
        heading: "Privacy",
        paragraphs: [
          "Enable Private Mode in Settings to stop new turns from being recorded. Existing sessions are not deleted automatically; you can clear them manually from a session's overflow menu.",
        ],
        tip: {
          title: "Quick toggle",
          body: "Private Mode is found under Settings → History. It only affects new sessions, not ones already saved.",
        },
      },
    ],
  },
  {
    id: "page:memory",
    group: "page",
    title: "Memory",
    pinnedFrom: "Memory",
    sections: [
      {
        heading: "What Vox remembers",
        paragraphs: [
          "Memory is a long-term knowledge graph. When processing is on, Vox quietly extracts facts, preferences, and entities from your sessions and links them together.",
        ],
      },
      {
        heading: "Reading the graph",
        bullets: [
          "Each node is one fact or entity. Lines show how they connect.",
          "Drag to rotate, scroll to zoom, click a node to inspect it.",
          "Color groups are collections; the legend on the right explains them.",
        ],
      },
      {
        heading: "Turning memory on and off",
        paragraphs: [
          "There are two switches in Settings → Memory: Retrieval and Processing. Retrieval injects stored facts into each new turn. Processing adds new facts as you talk.",
        ],
        tip: {
          title: "Tiers that support memory",
          body: "Memory is only fully active on tiers 1B and above. On tier 1A the graph stays empty because the embedded model cannot extract facts locally.",
        },
      },
    ],
  },
  {
    id: "page:monitoring",
    group: "page",
    title: "Monitoring",
    pinnedFrom: "Monitoring",
    sections: [
      {
        heading: "What you can see here",
        paragraphs: [
          "Monitoring is a live read-out of CPU, memory, model load, and pipeline state. It is read-only — nothing you change here affects Vox.",
        ],
      },
      {
        heading: "Reading the panels",
        bullets: [
          "Top row: pipeline mood and current state (Idle, Listening, Thinking, etc.).",
          "Middle row: model latency and queue depth.",
          "Bottom row: process memory and system RAM headroom.",
        ],
      },
      {
        heading: "If something looks wrong",
        bullets: [
          "High memory use on tier 1A is expected during long sessions.",
          "A stuck 'Thinking' state for over a minute usually means the LLM is being reloaded — wait it out or open Profiler to confirm.",
          "Profiling lets you snapshot exactly what the webview is holding at this instant.",
        ],
      },
    ],
  },
  {
    id: "settings:overview",
    group: "settings",
    title: "Settings overview",
    pinnedFrom: "Settings",
    sections: [
      {
        heading: "The radial hub",
        paragraphs: [
          "Settings is laid out as a wheel. The center is a status core, and each spoke is a domain of configuration. Tap a spoke to open that card; tap the center to open or close everything at once.",
        ],
      },
      {
        heading: "The six domains",
        bullets: [
          "Persona — who Vox sounds like and what it is told to do.",
          "Models — the engines that hear, think, and speak.",
          "History — what is saved and what is private.",
          "Memory — what is remembered and how it is recalled.",
          "Appearance — colors, theme, and ambient mood.",
          "Interaction — how Vox wakes up and what cloud keys it uses.",
        ],
      },
      {
        heading: "Saving changes",
        bullets: [
          "Most options save automatically the moment you change them.",
          "Changes that need a model reload show a Restart bar with a tick to apply.",
          "Restore Defaults at the top-right resets every domain at once.",
        ],
        tip: {
          title: "Cloud API keys",
          body: "If you pick a cloud provider without entering an API key, the Save button stays disabled until the key is filled in.",
        },
      },
    ],
  },
  {
    id: "settings:persona",
    group: "settings",
    title: "Persona",
    pinnedFrom: "Persona",
    sections: [
      {
        heading: "What this controls",
        paragraphs: [
          "Persona is the instruction prompt Vox reads before answering. You can write different prompts for the modular pipeline and the realtime duplex provider.",
        ],
      },
      {
        heading: "Modular vs Realtime",
        bullets: [
          "Modular is used when the pipeline is split into STT, LLM, and TTS stages.",
          "Realtime is used when a single cloud duplex model (like Gemini Live) does all three in one pass.",
        ],
      },
      {
        heading: "Template variables",
        paragraphs: [
          "The modular prompt supports <lang> and <script> placeholders. They are replaced at runtime with the language Vox detected from your speech.",
        ],
        tip: {
          title: "Keep it short",
          body: "Long prompts consume context window and slow the LLM. Two or three sentences of clear style guidance usually beats a full character sheet.",
        },
      },
    ],
  },
  {
    id: "settings:models",
    group: "settings",
    title: "Models",
    pinnedFrom: "Models",
    sections: [
      {
        heading: "Three engines, one screen",
        paragraphs: [
          "Models is where you pick the engines that listen (STT), think (LLM), and speak (TTS). The card switches layout depending on whether you are on the modular pipeline or a realtime duplex provider.",
        ],
      },
      {
        heading: "Choosing a provider",
        bullets: [
          "Embedded — runs entirely on this device. No network, no key, slower on weak hardware.",
          "Server — talks to a self-hosted engine you point it at (Ollama, custom HTTP).",
          "Cloud — talks to a hosted provider. Fastest, but needs an API key.",
        ],
      },
      {
        heading: "Restart-required changes",
        paragraphs: [
          "Switching engines, the LLM context window, or thread count triggers a model reload. Use the Apply & Reload bar at the bottom of the card to confirm.",
        ],
        tip: {
          title: "Downloaded vs not",
          body: "Embedded models only appear if the weights are on disk. If your model is missing, use the Download action on the card to fetch it before changing the selection.",
        },
      },
    ],
    tips: [
      {
        tier: "1A",
        title: "Tier 1A — embedded only",
        body: "On tier 1A only embedded providers are usable. Cloud and server providers are visible but stay disabled because the host hardware cannot guarantee sub-200ms response.",
      },
    ],
  },
  {
    id: "settings:history",
    group: "settings",
    title: "History",
    pinnedFrom: "History",
    sections: [
      {
        heading: "What lives here",
        paragraphs: [
          "History settings control whether new sessions are saved and how many past turns stay in the tray window's quick-recent list.",
        ],
      },
      {
        heading: "The two options",
        bullets: [
          "Private Mode — when on, new turns are not written to the history database.",
          "Tray History Limit — how many recent sessions appear in the floating tray's quick menu.",
        ],
      },
      {
        heading: "Compaction",
        paragraphs: [
          "When auto-compaction is on, old sessions are summarized and pruned in the background to keep the database lean. Compaction only runs while Vox is idle or paused.",
        ],
      },
    ],
  },
  {
    id: "settings:memory",
    group: "settings",
    title: "Memory",
    pinnedFrom: "Memory",
    sections: [
      {
        heading: "Two switches",
        paragraphs: [
          "Retrieval injects stored facts into every new turn. Processing extracts new facts from your sessions and adds them to the graph.",
        ],
      },
      {
        heading: "The dials",
        bullets: [
          "Recall Fact Limit — how many long-term facts enter each turn.",
          "Relevance Cutoff — minimum similarity score for a fact to be considered.",
          "Knowledge Graph Hops — how many relationship steps the retriever can follow.",
          "Context Budget — maximum share of the LLM window given to memory.",
          "Conversation Window — how long a topic stays chained as active context.",
        ],
      },
      {
        heading: "When to turn things off",
        bullets: [
          "Turn Retrieval off if Vox keeps pulling in irrelevant old facts.",
          "Turn Processing off if you want a session to be a clean one-off conversation.",
        ],
        tip: {
          title: "Tier 1A caveat",
          body: "On tier 1A Processing is always off because the embedded model cannot extract facts locally. Retrieval is available but the graph stays empty.",
        },
      },
    ],
    tips: [
      {
        tier: "1A",
        title: "Tier 1A",
        body: "Memory is read-only on tier 1A. The graph cannot grow because no extraction engine is loaded.",
      },
    ],
  },
  {
    id: "settings:appearance",
    group: "settings",
    title: "Appearance",
    pinnedFrom: "Appearance",
    sections: [
      {
        heading: "Theme and accent",
        paragraphs: [
          "Switch between the dark ambient surface and the light glass theme. The accent color tints the orb, focus rings, and active states throughout the app.",
        ],
      },
      {
        heading: "Picking a seed",
        bullets: [
          "The accent is generated from a seed, not picked from a swatch. Try a few — each one reshapes the personality of the UI.",
          "Your selection is saved instantly. No reload needed.",
        ],
      },
    ],
  },
  {
    id: "settings:interaction",
    group: "settings",
    title: "Interaction",
    pinnedFrom: "Interaction",
    sections: [
      {
        heading: "Pipeline mode",
        paragraphs: [
          "Modular splits hearing, thinking, and speaking into separate stages you can mix and match. Realtime sends audio to a single duplex cloud model that does all three in one pass.",
        ],
      },
      {
        heading: "Activation mode",
        bullets: [
          "Continuous — Vox is always listening and only responds when addressed.",
          "Push-to-Talk — you hold a hotkey while speaking; nothing is heard otherwise.",
        ],
      },
      {
        heading: "Cloud keys",
        paragraphs: [
          "When you select a cloud provider on any of the engines, its API key is collected here. The key is stored locally and only sent to the matching provider.",
        ],
        tip: {
          title: "Switching modes",
          body: "Switching between Modular and Realtime is not auto-saved — confirm the change with the tick in the card footer to apply.",
        },
      },
    ],
  },
  {
    id: "wizard:welcome",
    group: "wizard",
    title: "Welcome",
    pinnedFrom: "Wizard · Welcome",
    sections: [
      {
        heading: "What this is",
        paragraphs: [
          "The first-time setup walks you through hardware checks, model downloads, and a live voice test. It only runs once.",
        ],
      },
      {
        heading: "What you will do",
        bullets: [
          "Confirm your hardware and microphone.",
          "Pick the models you want Vox to use.",
          "Run a five-second voice test to make sure everything is wired up.",
        ],
      },
      {
        heading: "Skipping it",
        paragraphs: [
          "You can leave at any time and come back via Settings, but the app will not work fully until at least the audio test has passed.",
        ],
      },
    ],
  },
  {
    id: "wizard:system",
    group: "wizard",
    title: "System check",
    pinnedFrom: "Wizard · System check",
    sections: [
      {
        heading: "What we are checking",
        bullets: [
          "Available RAM and CPU cores.",
          "Microphone permissions.",
          "Disk space for downloaded model weights.",
        ],
      },
      {
        heading: "If a check fails",
        paragraphs: [
          "Each red row shows what to fix. Most issues are microphone permissions or a disk almost full of other things.",
        ],
      },
    ],
  },
  {
    id: "wizard:model",
    group: "wizard",
    title: "Model setup",
    pinnedFrom: "Wizard · Model setup",
    sections: [
      {
        heading: "Choosing a starting set",
        paragraphs: [
          "A minimal set that fits your hardware is preselected. You can swap any engine later from Settings.",
        ],
      },
      {
        heading: "Downloads",
        bullets: [
          "Each download shows a progress bar and the on-disk size.",
          "You can keep talking to the rest of the wizard while a model downloads in the background.",
        ],
      },
    ],
  },
  {
    id: "wizard:audio",
    group: "wizard",
    title: "Audio setup",
    pinnedFrom: "Wizard · Audio setup",
    sections: [
      {
        heading: "Pick your input",
        paragraphs: [
          "Choose the microphone you want Vox to listen through. The selected device is remembered for future sessions.",
        ],
      },
      {
        heading: "Levels",
        bullets: [
          "Speak normally and watch the meter — it should peak near the middle.",
          "If it sits at the bottom, raise the system input volume.",
          "If it pegs at the top, lower the input or move back from the mic.",
        ],
      },
    ],
  },
  {
    id: "wizard:test",
    group: "wizard",
    title: "Live test",
    pinnedFrom: "Wizard · Live test",
    sections: [
      {
        heading: "A five-second sanity check",
        paragraphs: [
          "Say a short sentence. Vox will transcribe it, run it through the LLM, and speak a reply. If you can hear a reply, the full pipeline is working.",
        ],
      },
      {
        heading: "If something fails",
        bullets: [
          "No transcription: your microphone input is too quiet or the STT model is still loading.",
          "No reply: the LLM is still loading or your context window is full.",
          "Stuttering reply: realtime CPU is saturated — close other heavy apps.",
        ],
      },
    ],
  },
  {
    id: "faq:shortcuts",
    group: "faq",
    title: "Keyboard shortcuts",
    pinnedFrom: "Shortcuts",
    sections: [
      {
        heading: "Global",
        shortcuts: [
          { keys: "Shift + /", label: "Open this help drawer" },
          { keys: "Esc", label: "Close the topmost overlay" },
        ],
      },
      {
        heading: "Settings",
        shortcuts: [
          { keys: "Esc", label: "Close the topmost settings card" },
        ],
      },
      {
        heading: "Voice typing",
        paragraphs: [
          "The voice-typing hotkey is configured under Settings → Interaction. It is a global hotkey that works even when Vox is not focused.",
        ],
      },
    ],
  },
  {
    id: "faq:tiers",
    group: "faq",
    title: "About tiers",
    pinnedFrom: "Tiers",
    sections: [
      {
        heading: "What a tier is",
        paragraphs: [
          "A tier is a summary of what your hardware can comfortably run. Vox picks the right balance of local vs cloud engines for you based on it.",
        ],
      },
      {
        heading: "The five tiers",
        bullets: [
          "1A — 8 GB RAM, no GPU. Embedded engines only, no memory graph.",
          "1B — 8 GB+ with a dedicated GPU. Full local pipeline plus memory.",
          "2A — Remote LLM, local audio. Speech and voice stay on device, thinking happens on your self-hosted server.",
          "2B — Cloud LLM, local audio. Best of both — local latency, hosted reasoning.",
          "3 — Realtime duplex. A single cloud model handles hearing, thinking, and speaking in one stream.",
        ],
      },
    ],
  },
  {
    id: "faq:privacy",
    group: "faq",
    title: "Privacy and data",
    pinnedFrom: "Privacy",
    sections: [
      {
        heading: "What stays on this device",
        bullets: [
          "All embedded model inference runs locally.",
          "Session history is stored in a local database.",
          "Memory graph data stays in your local app data folder.",
        ],
      },
      {
        heading: "What leaves the device",
        paragraphs: [
          "Only what you explicitly route to a server or cloud provider. Toggling an engine to Server or Cloud in Settings is the moment audio or text starts leaving the machine.",
        ],
      },
      {
        heading: "Clearing your data",
        bullets: [
          "Private Mode in Settings → History stops new turns from being recorded.",
          "Restore Defaults does not delete history — use the History page's clear action for that.",
          "Uninstalling the app removes the local database.",
        ],
      },
    ],
  },
];

export const HELP_DRAWER_COPY = {
  triggerLabel: "Help & guide",
  headerTitle: "Help & guide",
  headerSubtitle: "Walkthroughs, settings reference, and quick answers",
  tocHeading: "Contents",
  searchPlaceholder: "Search the guide",
  pinnedCrumbPrefix: "Pinned to",
  pinnedCrumbClear: "Clear pin",
  tierBadgePrefix: "Your tier",
  emptyStateTitle: "No article matches that context",
  emptyStateBody: "The page you opened help from does not have a guide yet. Browse the contents on the left or use the shortcuts below.",
  scrollTop: "Back to top",
  shortcutsHeading: "Shortcuts",
};
