import {
  Archive,
  Bot,
  Brain,
  Calendar,
  CheckCircle2,
  Clock,
  Cpu,
  Database,
  Eye,
  FileText,
  Filter,
  Globe,
  History,
  Keyboard,
  Key,
  List,
  Mic,
  MicOff,
  Moon,
  Palette,
  Pause,
  Pencil,
  Play,
  Power,
  Radio,
  RotateCcw,
  Search,
  ShieldOff,
  SlidersHorizontal,
  Sparkles,
  Timer,
  Trash2,
  Volume2,
  VolumeX,
  X,
  XCircle,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { HelpControlItem } from "@/shared/components/help/HelpControlCard";

export type HelpTier = "1A" | "1B" | "2A" | "2B" | "3";

export interface PageHelpGuide {
  title: string;
  badge: string;
  subtitle: string;
  sections: Array<{
    heading: string;
    description?: string;
    controls?: HelpControlItem[];
    tips?: string[];
  }>;
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. HOME PAGE GUIDE
// ─────────────────────────────────────────────────────────────────────────────
export const HOME_PAGE_HELP: PageHelpGuide = {
  title: "Home Voice Stage",
  badge: "/",
  subtitle: "Talk with Vox out loud, pause it, or type when silence suits you better.",
  sections: [
    {
      heading: "How to speak",
      description: "Wake Vox up first, then pick whichever way of speaking suits the moment.",
      controls: [
        {
          icon: Power,
          name: "Engage Vox",
          badge: "Start",
          action: "Press Ctrl+Space or the power button.",
          outcome: "Press Ctrl+Space or the power button to wake Vox. The orb lights up when it is ready to listen.",
          shortcut: "Ctrl+Space",
        },
        {
          icon: Mic,
          name: "Hold to speak",
          badge: "Push-to-talk",
          action: "Hold Space and speak, then release.",
          outcome: "Hold Space and speak, then release to send. Vox only listens while you hold, so background noise never triggers it.",
          shortcut: "Space",
          tip: "Only works while Vox is engaged and Push-to-talk mode is on.",
        },
        {
          icon: Radio,
          name: "Speak hands-free",
          badge: "Continuous",
          action: "Simply start talking.",
          outcome: "Just start talking. In Continuous mode Vox notices your voice on its own and replies when you pause.",
          tip: "Switch modes in Settings > Interaction.",
        },
        {
          icon: Keyboard,
          name: "Type instead",
          badge: "Text mode",
          action: "Press T and type your message.",
          outcome: "Press T to open the text box, type your message and hit Enter. Useful in meetings or quiet rooms.",
          shortcut: "T",
        },
      ],
    },
    {
      heading: "During a conversation",
      description: "Stay in control while Vox is talking or listening.",
      controls: [
        {
          icon: VolumeX,
          name: "Interrupt Vox",
          badge: "Barge in",
          action: "Talk while Vox is speaking.",
          outcome: "Talk while Vox is speaking to cut in. It stops instantly and listens to your new question instead.",
        },
        {
          icon: Pause,
          name: "Pause everything",
          badge: "Pause",
          action: "Press P to freeze the pipeline.",
          outcome: "Press P to freeze listening and audio together. Press it again to resume exactly where you left off.",
          shortcut: "P",
        },
        {
          icon: MicOff,
          name: "Mute the microphone",
          badge: "Mic off",
          action: "Press M to stop being heard.",
          outcome: "Press M and Vox cannot hear anything at all. Press it again to let it listen once more.",
          shortcut: "M",
        },
        {
          icon: Volume2,
          name: "Mute the speaker",
          badge: "Silent voice",
          action: "Press Shift+M to silence playback.",
          outcome: "Press Shift+M to silence Vox's voice while the conversation keeps going on screen.",
          shortcut: "Shift+M",
        },
      ],
    },
    {
      heading: "Sessions and panels",
      description: "Move between conversations and keep the screen tidy.",
      controls: [
        {
          icon: History,
          name: "Switch conversations",
          badge: "Sessions",
          action: "Open the sessions rail.",
          outcome: "Press Ctrl+S or pull the left-edge handle to open your session list and jump between chats.",
          shortcut: "Ctrl+S",
        },
        {
          icon: Sparkles,
          name: "Start a temporary chat",
          badge: "No saving",
          action: "Toggle the temporary session switch.",
          outcome: "Flip the temporary session switch for a chat that is never saved — no history entries and no memories learned.",
        },
        {
          icon: X,
          name: "Close panels",
          badge: "Dismiss",
          action: "Press Escape.",
          outcome: "Press Escape to close the topmost panel or cancel whatever you are doing.",
          shortcut: "Escape",
        },
      ],
      tips: [
        "Vox saves every conversation automatically, unless the session is temporary.",
      ],
    },
  ],
};

// ─────────────────────────────────────────────────────────────────────────────
// 2. HISTORY PAGE GUIDE
// ─────────────────────────────────────────────────────────────────────────────
export const HISTORY_PAGE_HELP: PageHelpGuide = {
  title: "Conversation History",
  badge: "/history",
  subtitle: "Find any past conversation, replay what was said, and keep the list tidy.",
  sections: [
    {
      heading: "Find a conversation",
      description: "Search by words, narrow by date, or walk across sessions.",
      controls: [
        {
          icon: Search,
          name: "Search everything",
          badge: "Find text",
          action: "Type into the search bar.",
          outcome: "Type a word or topic into the search bar to keep only the sessions that mention it.",
        },
        {
          icon: List,
          name: "Filter by date",
          badge: "Time window",
          action: "Pick a date filter.",
          outcome: "Choose All Dates, Today, Past 7 Days or Past 30 Days to narrow the list to that window.",
        },
        {
          icon: Calendar,
          name: "Day and month views",
          badge: "Browse",
          action: "Switch the timeline view.",
          outcome: "Use the day view to read one day at a time, or the month view to scan the whole month and tap a day to open it.",
        },
        {
          icon: History,
          name: "Walk through time",
          badge: "Arrow keys",
          action: "Press the left and right arrows.",
          outcome: "Press the left and right arrows to step across sessions on the timeline without touching the mouse.",
          shortcut: "← / →",
        },
      ],
    },
    {
      heading: "Read and replay",
      description: "Open any session to see the full exchange and hear it again.",
      controls: [
        {
          icon: FileText,
          name: "Open a transcript",
          badge: "Read",
          action: "Click a session or press Enter.",
          outcome: "Click any session, or focus it and press Enter, to open the complete transcript of that conversation.",
          shortcut: "Enter / Space",
        },
        {
          icon: Play,
          name: "Replay the audio",
          badge: "Listen",
          action: "Press play on any turn.",
          outcome: "Press play on any turn to hear the original voice recording next to the written text.",
        },
      ],
    },
    {
      heading: "Tidy up",
      description: "Remove what you no longer need and compress the rest.",
      controls: [
        {
          icon: Trash2,
          name: "Delete a session",
          badge: "Remove",
          action: "Press Delete, then confirm.",
          outcome: "Focus a session and press Delete, then Enter to confirm or Escape to cancel. Deleting also removes its stored audio.",
          shortcut: "Delete",
        },
        {
          icon: Archive,
          name: "Compact a long session",
          badge: "Summarize",
          action: "Compact the session.",
          outcome: "Compact a long session to fold its history into a short summary, keeping the list fast without losing the thread.",
        },
      ],
      tips: [
        "Prefer chatting off the record? Turn on the private session switch in Settings > Working memory.",
      ],
    },
  ],
};

// ─────────────────────────────────────────────────────────────────────────────
// 3. MEMORY PAGE GUIDE
// ─────────────────────────────────────────────────────────────────────────────
export const MEMORY_PAGE_HELP: PageHelpGuide = {
  title: "Memory & What Vox Knows",
  badge: "/memory",
  subtitle: "See what Vox has learned about you, approve new additions, and fix mistakes.",
  sections: [
    {
      heading: "What Vox remembers",
      description: "Everything Vox knows about you lives here, on this computer only.",
      controls: [
        {
          icon: Brain,
          name: "Your profile",
          badge: "About you",
          action: "Open the personal memory dossier.",
          outcome: "Your profile holds lasting facts about you — name, work, preferences. Press Shift+Up on the Memory page to open and edit it.",
          shortcut: "Shift+Up",
        },
        {
          icon: Sparkles,
          name: "Observations",
          badge: "New findings",
          action: "Review newly noticed facts.",
          outcome: "Vox writes down things it notices in conversation as observations. Fresh ones wait as staged until you approve them; older ones are already integrated or still queued.",
        },
        {
          icon: Eye,
          name: "Memory graph",
          badge: "Explore",
          action: "Click any floating node.",
          outcome: "Each floating node is one remembered fact. Click a node to see the fact itself, its category, and the conversation it came from.",
          shortcut: "Enter / Space",
        },
      ],
    },
    {
      heading: "Review and approve",
      description: "Nothing new lands in your profile without your say-so — unless you allow it.",
      controls: [
        {
          icon: CheckCircle2,
          name: "Accept a suggestion",
          badge: "Approve",
          action: "Accept a proposed change.",
          outcome: "Open the staging view to read each proposed change in place. Accept the ones that are right and they are written into your profile immediately.",
        },
        {
          icon: XCircle,
          name: "Reject a suggestion",
          badge: "Discard",
          action: "Reject a proposed change.",
          outcome: "Rejecting throws away that change only — every other decision you made stays exactly as you left it.",
        },
        {
          icon: SlidersHorizontal,
          name: "Manual or automatic",
          badge: "Policy",
          action: "Choose the suggestion policy.",
          outcome: "In Settings > Personal memory you decide: approve every change yourself, or let Vox integrate new findings automatically.",
        },
      ],
    },
    {
      heading: "Search and versions",
      description: "Find any fact, look back in time, and rebuild when things feel messy.",
      controls: [
        {
          icon: Search,
          name: "Search memories",
          badge: "Find fact",
          action: "Press Ctrl+K and type.",
          outcome: "Press Ctrl+K and start typing to find any remembered fact instantly.",
          shortcut: "Ctrl+K",
        },
        {
          icon: History,
          name: "Version history",
          badge: "Look back",
          action: "Preview an older version.",
          outcome: "Every approved change saves a new version. Preview any older one freely — browsing never changes anything — and press Restore only if you want it back.",
        },
        {
          icon: RotateCcw,
          name: "Rebuild the profile",
          badge: "Fresh start",
          action: "Press Regenerate.",
          outcome: "Press Regenerate to rebuild your profile cleanly from already-approved facts when the wording feels messy.",
        },
      ],
      tips: [
        "Memories live only on this computer. Nothing is uploaded anywhere.",
      ],
    },
  ],
};

// ─────────────────────────────────────────────────────────────────────────────
// 4. SETTINGS PAGE GUIDE (DIVIDED PER DOMAIN CARD)
// ─────────────────────────────────────────────────────────────────────────────
export type SettingsCardId =
  | "models"
  | "interaction"
  | "persona"
  | "working_memory"
  | "personal_memory"
  | "appearance";

export interface SettingsCardHelp {
  id: SettingsCardId;
  label: string;
  badge: string;
  icon: LucideIcon;
  overview: string;
  controls: HelpControlItem[];
  tips?: string[];
}

export const SETTINGS_PAGE_HELP: {
  title: string;
  badge: string;
  subtitle: string;
  cards: SettingsCardHelp[];
} = {
  title: "Settings & Options",
  badge: "/settings",
  subtitle: "Decide how Vox thinks, listens, speaks, remembers, and looks. Changes save on their own.",
  cards: [
    {
      id: "models",
      label: "Models",
      badge: "Thinking & voice",
      icon: Cpu,
      overview: "Pick the brains and voices Vox uses — running on this machine, or in the cloud.",
      controls: [
        {
          icon: Brain,
          name: "Reasoning model",
          badge: "The brain",
          action: "Choose which AI answers you.",
          outcome: "Choose which AI answers you. Local models run fully offline and private; cloud providers answer faster and smarter but need an API key.",
        },
        {
          icon: Key,
          name: "Cloud API keys",
          badge: "Credentials",
          action: "Paste your provider key once.",
          outcome: "Paste your provider key once. It is stored only on this device and unlocks the cloud models you pay for.",
        },
        {
          icon: Mic,
          name: "Listening model",
          badge: "Hearing",
          action: "Choose the speech recognition.",
          outcome: "Choose how Vox turns your speech into text. Local recognition keeps your audio on this machine from start to finish.",
        },
        {
          icon: Volume2,
          name: "Speaking voice",
          badge: "Voice",
          action: "Pick a voice and its speed.",
          outcome: "Pick the voice Vox speaks with and adjust its speed, or clone a voice from a short recording of your own.",
        },
        {
          icon: Cpu,
          name: "Modular or Realtime",
          badge: "Pipeline",
          action: "Pick how voice flows.",
          outcome: "Modular chains separate models for hearing, thinking and speaking. Realtime opens one direct voice connection with sub-second replies.",
        },
      ],
      tips: [
        "Switching the active model rebuilds the voice engine once — Vox tells you while it restarts.",
      ],
    },
    {
      id: "interaction",
      label: "Interaction",
      badge: "Mic & dictation",
      icon: SlidersHorizontal,
      overview: "How Vox hears you inside this app, and how you dictate into any other app.",
      controls: [
        {
          icon: Radio,
          name: "Continuous or push-to-talk",
          badge: "Trigger",
          action: "Pick when Vox listens.",
          outcome: "Continuous listens all the time and replies when you pause. Push-to-talk only listens while you hold Space — better for noisy rooms.",
        },
        {
          icon: Keyboard,
          name: "Dictation hotkey",
          badge: "Global key",
          action: "Press Alt+V anywhere, or rebind it.",
          outcome: "Press Alt+V anywhere on your system to dictate into other apps. Click the hotkey box to record your own combination.",
        },
        {
          icon: FileText,
          name: "Where dictation goes",
          badge: "Destination",
          action: "Pick Paste, Clipboard or Tray.",
          outcome: "Paste types directly into the active window, Clipboard copies silently, and Tray shows a small floating window with a live waveform.",
        },
        {
          icon: Timer,
          name: "Auto-stop on silence",
          badge: "Silence cutoff",
          action: "Set how dictation ends.",
          outcome: "Vox finishes your dictation automatically after a short silence, or waits until you release the hotkey when auto-stop is off.",
        },
      ],
      tips: [
        "On some Linux desktops synthetic typing is blocked — Clipboard mode always works as a fallback.",
      ],
    },
    {
      id: "persona",
      label: "Persona",
      badge: "Tone & instructions",
      icon: Bot,
      overview: "Tell Vox who to be and how to talk to you.",
      controls: [
        {
          icon: Pencil,
          name: "Custom instructions",
          badge: "Your words",
          action: "Write how Vox should act.",
          outcome: "Write who Vox is and how it should answer — its role, your projects, rules like always be concise. It applies from your very next message.",
        },
        {
          icon: Bot,
          name: "Separate realtime instructions",
          badge: "Voice mode",
          action: "Write shorter spoken-style rules.",
          outcome: "The direct voice mode gets its own shorter instructions, because spoken conversation needs briefer answers than text.",
        },
        {
          icon: Eye,
          name: "Preview first",
          badge: "Read-only",
          action: "Read the assembled prompt.",
          outcome: "Read the assembled instructions in Preview before editing, so you can see exactly what the AI receives.",
        },
      ],
    },
    {
      id: "working_memory",
      label: "Working memory",
      badge: "Sessions & context",
      icon: History,
      overview: "What Vox keeps from the current conversation, and whether it may search the web.",
      controls: [
        {
          icon: ShieldOff,
          name: "Private session",
          badge: "Off the record",
          action: "Turn the private switch on.",
          outcome: "When on, nothing from your chats is written to disk — no audio, no transcripts, no memories. Turn it off to save history again.",
        },
        {
          icon: Archive,
          name: "Automatic summaries",
          badge: "Long chats",
          action: "Let Vox compact long chats.",
          outcome: "Long conversations are folded into short summaries so Vox keeps up without slowing down. Turn this off to be asked before each summary.",
        },
        {
          icon: Globe,
          name: "Web search",
          badge: "Live facts",
          action: "Allow live lookups.",
          outcome: "Let Vox look up live facts mid-conversation when its own knowledge is out of date.",
        },
        {
          icon: Database,
          name: "Context share",
          badge: "Attention split",
          action: "Balance profile versus present.",
          outcome: "Decide how much of the AI's attention goes to your profile and past dialogue versus the question in front of it.",
        },
      ],
    },
    {
      id: "personal_memory",
      label: "Personal memory",
      badge: "Learning & recall",
      icon: Brain,
      overview: "What Vox learns about you over time, and how much of it joins each answer.",
      controls: [
        {
          icon: Brain,
          name: "Memory recall",
          badge: "Use past",
          action: "Turn recall on or off.",
          outcome: "When on, Vox brings relevant memories into each answer. Turn it off and it only uses the current conversation.",
        },
        {
          icon: Sparkles,
          name: "Memory learning",
          badge: "Save new",
          action: "Turn learning on or off.",
          outcome: "When on, Vox writes down new facts from your chats. Turn it off to pause all learning without deleting anything.",
        },
        {
          icon: Database,
          name: "Memory limit",
          badge: "How many",
          action: "Set memories per answer.",
          outcome: "The maximum number of memories Vox pulls into one answer. More context helps hard questions; fewer keeps answers focused.",
        },
        {
          icon: Filter,
          name: "Relevance threshold",
          badge: "How close",
          action: "Set how strict matching is.",
          outcome: "How closely a memory must match your question to be included. Higher means only close matches; lower allows loose connections.",
        },
        {
          icon: Clock,
          name: "Update schedule",
          badge: "When to fold in",
          action: "Pick manual or daily.",
          outcome: "Decide how often Vox folds new observations into your profile — on demand with the Integrate button, or automatically every day at your chosen time.",
        },
        {
          icon: CheckCircle2,
          name: "Suggestion policy",
          badge: "Review step",
          action: "Pick manual review or auto.",
          outcome: "Manual review stages every change for your approval on the Memory page. Auto-apply skips the queue and integrates new findings directly.",
        },
      ],
      tips: [
        "The defaults suit almost everyone — change these only when recall feels too thin or too noisy.",
      ],
    },
    {
      id: "appearance",
      label: "Appearance",
      badge: "Theme & color",
      icon: Palette,
      overview: "Make Vox look the way you like.",
      controls: [
        {
          icon: Moon,
          name: "Dark or light",
          badge: "Theme",
          action: "Switch the theme.",
          outcome: "Dark Obsidian is a deep-space look; Light Glass is clean and frosted. Pick whichever strains your eyes less.",
        },
        {
          icon: Palette,
          name: "Accent color",
          badge: "Your color",
          action: "Pick any color.",
          outcome: "Pick any color and every glow, border and animation across the app follows it.",
        },
        {
          icon: RotateCcw,
          name: "Restore defaults",
          badge: "Reset all",
          action: "Reset everything.",
          outcome: "One click returns every setting — colors, models, memory, persona — to factory defaults. This cannot be undone.",
        },
      ],
    },
  ],
};
