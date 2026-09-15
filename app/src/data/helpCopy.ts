import {
  Mic,
  Search,
  SlidersHorizontal,
  Orbit,
  Trash2,
  FileText,
  Play,
  Sparkles,
  Download,
  Cpu,
  Bot,
  Brain,
  Palette,
  ShieldOff,
  Volume2,
  VolumeX,
  Keyboard,
  CheckCircle2,
  Key,
  RotateCcw,
} from "lucide-react";
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
  subtitle: "Talk to Vox naturally, control audio listening modes, and switch conversations.",
  sections: [
    {
      heading: "How to Talk",
      description: "Choose whichever way feels most comfortable to talk with Vox.",
      controls: [
        {
          icon: Mic,
          name: "Push-to-Talk",
          badge: "Hold to Talk",
          action: "Hold Spacebar (or click and hold the microphone button).",
          outcome: "Vox only listens while you hold the key. Release when you're done speaking to send immediately.",
          shortcut: "Space",
          tip: "Best for noisy environments or open offices.",
        },
        {
          icon: Volume2,
          name: "Voice Activation (Hands-Free)",
          badge: "Auto Listen",
          action: "Simply start talking naturally.",
          outcome: "Vox automatically detects your voice, transcribes what you say, and responds when you pause.",
          tip: "Turn this on in Settings > Interaction if you prefer hands-free talking.",
        },
        {
          icon: VolumeX,
          name: "Interrupt / Barge-in",
          badge: "Instant Stop",
          action: "Speak while Vox is talking.",
          outcome: "Vox immediately stops speaking and starts listening to your new question.",
        },
        {
          icon: VolumeX,
          name: "Mute Microphone",
          badge: "Mic Off",
          action: "Click the mute button in the bottom dock.",
          outcome: "Pauses all listening so nothing you say is heard or transcribed.",
          shortcut: "M",
        },
      ],
      tips: [
        "You can press Escape anytime to close open side panels.",
        "Drag the left edge or click the top-left icon to view all your chat sessions.",
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
  subtitle: "Browse, replay, and search all your past voice conversations.",
  sections: [
    {
      heading: "Browsing Past Chats",
      description: "Explore previous conversations by date or search for specific topics.",
      controls: [
        {
          icon: Orbit,
          name: "3D Orbit Wheel",
          badge: "Timeline",
          action: "Drag along the circle or press arrow keys to travel back in time.",
          outcome: "Each node represents a conversation. Larger circles mean longer, deeper discussions.",
          shortcut: "[ or ]",
        },
        {
          icon: FileText,
          name: "List View",
          badge: "Quick Search",
          action: "Click the list icon at the top to see a clean, sorted table of all past chats.",
          outcome: "Easily scan dates, message counts, and conversation lengths.",
        },
        {
          icon: Search,
          name: "Search Conversations",
          badge: "Find Text",
          action: "Type any keyword into the search bar.",
          outcome: "Instantly finds chats where that word or topic was discussed.",
        },
        {
          icon: Play,
          name: "Replay Voice Audio",
          badge: "Audio Replay",
          action: "Click any conversation to open its transcript.",
          outcome: "Read what was said and replay the voice audio for any turn.",
        },
        {
          icon: Trash2,
          name: "Delete a Chat",
          badge: "Clean Up",
          action: "Click the trash icon on any conversation.",
          outcome: "Permanently removes the conversation and its audio files.",
        },
      ],
      tips: [
        "Want private chats? Turn on Incognito Mode in Settings to chat without saving history.",
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
  subtitle: "See what Vox has learned about you and your preferences across chats.",
  sections: [
    {
      heading: "How Memory Works",
      description: "Vox automatically saves helpful facts from your conversations so you never have to repeat yourself.",
      controls: [
        {
          icon: Sparkles,
          name: "Your Profile (Center Crystal)",
          badge: "Profile",
          action: "Click the glowing crystal in the center of the screen.",
          outcome: "View and edit your personal profile facts (your name, preferences, work, and habits).",
        },
        {
          icon: Brain,
          name: "Memory Nodes (Floating Leaves)",
          badge: "Key Facts",
          action: "Click any floating node around the canopy.",
          outcome: "See the specific fact Vox remembered, when it was learned, and which conversation it came from.",
        },
        {
          icon: Download,
          name: "Backup Memory",
          badge: "Export",
          action: "Click Export in the memory drawer.",
          outcome: "Downloads a private backup file of all your memories to your computer.",
        },
      ],
      tips: [
        "All memories are stored privately on your computer.",
        "Vox automatically merges duplicate memories in the background to keep things neat.",
      ],
    },
  ],
};

// ─────────────────────────────────────────────────────────────────────────────
// 4. SETTINGS PAGE GUIDE (DIVIDED PER CARD)
// ─────────────────────────────────────────────────────────────────────────────
export type SettingsCardId = "models" | "interaction" | "persona" | "memory" | "history" | "appearance";

export interface SettingsCardHelp {
  id: SettingsCardId;
  label: string;
  badge: string;
  icon: any;
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
  subtitle: "Choose how Vox thinks, speaks, listens, and looks.",
  cards: [
    {
      id: "models",
      label: "AI & Voice",
      badge: "Thinking & Speech",
      icon: Cpu,
      overview: "Choose the AI brain you want Vox to think with, and pick the voice you want Vox to speak with.",
      controls: [
        {
          icon: Brain,
          name: "AI Thinking Model",
          badge: "The Brain",
          action: "Select which AI model handles your conversations.",
          outcome: "Pick local models (runs 100% offline on your machine) or connect to cloud models like Gemini, Claude, or OpenAI.",
        },
        {
          icon: Volume2,
          name: "Voice & Speech",
          badge: "Speaking Voice",
          action: "Choose the voice Vox speaks with.",
          outcome: "Select from realistic natural voices, adjust talking speed, or clone your own voice from a short audio clip.",
        },
        {
          icon: Mic,
          name: "Speech Recognition",
          badge: "Hearing",
          action: "Select how Vox transcribes your voice.",
          outcome: "Fast, accurate speech-to-text models that turn your spoken words into text in real time.",
        },
        {
          icon: RotateCcw,
          name: "Apply & Reload",
          badge: "Bottom Dock",
          action: "Click Apply & Reload after switching offline models.",
          outcome: "Restarts the AI engine smoothly with your newly chosen models.",
        },
      ],
      tips: [
        "Cloud models give the fastest and smartest answers, while local models run completely private and offline.",
      ],
    },
    {
      id: "interaction",
      label: "Talking & Mic",
      badge: "Microphone & Keys",
      icon: SlidersHorizontal,
      overview: "Set up how you speak to Vox, configure microphone devices, and set up system-wide dictation.",
      controls: [
        {
          icon: Mic,
          name: "Push-to-Talk vs Voice Activation",
          badge: "Trigger Mode",
          action: "Choose how Vox knows when you're speaking.",
          outcome: "Push-to-Talk listens only while holding a key. Voice Activation listens continuously and replies when you pause.",
        },
        {
          icon: Keyboard,
          name: "System Dictation",
          badge: "Type Anywhere",
          action: "Set a global keyboard shortcut for dictation.",
          outcome: "Press the shortcut anywhere on your computer (browser, code editor, notes) to speak and have your words typed automatically.",
        },
        {
          icon: Key,
          name: "Cloud API Keys",
          badge: "API Credentials",
          action: "Enter your API keys for cloud AI providers.",
          outcome: "Keys are saved securely on your device so Vox can connect directly to your chosen provider.",
        },
        {
          icon: Volume2,
          name: "Microphone & Speaker",
          badge: "Audio Devices",
          action: "Select your preferred microphone and speakers.",
          outcome: "Ensures Vox hears you through the right microphone and plays speech through your desired output.",
        },
      ],
      tips: [
        "If you work in a noisy room, Push-to-Talk prevents background chatter from accidentally triggering responses.",
      ],
    },
    {
      id: "persona",
      label: "Personality",
      badge: "Instructions & Tone",
      icon: Bot,
      overview: "Shape how Vox talks to you: friendly, brief, technical, or conversational.",
      controls: [
        {
          icon: Bot,
          name: "Custom Instructions",
          badge: "Prompt Editor",
          action: "Type instructions telling Vox who to be and how to answer.",
          outcome: "Give Vox a specific role, tell it what projects you're working on, or set rules like 'always give concise answers'.",
        },
        {
          icon: Sparkles,
          name: "Tone Presets",
          badge: "Style Changers",
          action: "Click preset styles like Concise, Technical, or Casual.",
          outcome: "Instantly sets Vox's conversation style without needing to write custom prompt text.",
        },
        {
          icon: CheckCircle2,
          name: "Automatic Saving",
          badge: "Instant Sync",
          action: "Type any changes in the prompt box.",
          outcome: "Saves automatically. Your new instructions take effect on your very next conversation turn.",
        },
      ],
    },
    {
      id: "memory",
      label: "Memory Settings",
      badge: "Recall & Context",
      icon: Brain,
      overview: "Control how much past context Vox remembers and uses when answering your questions.",
      controls: [
        {
          icon: Brain,
          name: "Memory Recall Depth",
          badge: "How Much",
          action: "Choose how many relevant past memories are retrieved per turn.",
          outcome: "Higher values give Vox more background context from past chats; lower values keep answers focused on the immediate topic.",
        },
        {
          icon: SlidersHorizontal,
          name: "Similarity Cutoff",
          badge: "Relevance",
          action: "Adjust how closely a past memory must match your current topic to be recalled.",
          outcome: "Higher settings recall only exact matches; lower settings allow broader connections.",
        },
        {
          icon: RotateCcw,
          name: "Session Chaining Window",
          badge: "Continuing Chats",
          action: "Set how many hours recent chat context stays warm.",
          outcome: "When you return to a conversation within this window, Vox picks up right where you left off.",
        },
      ],
      tips: [
        "Default memory settings work great for almost everyone. Adjust these only if you want deeper or lighter recall.",
      ],
    },
    {
      id: "history",
      label: "Privacy & History",
      badge: "Storage & Incognito",
      icon: ShieldOff,
      overview: "Manage your conversation privacy and choose whether chats are saved to disk.",
      controls: [
        {
          icon: ShieldOff,
          name: "Incognito Mode",
          badge: "Private Browsing",
          action: "Turn on Incognito Mode.",
          outcome: "Vox answers your questions normally, but deletes all audio, transcripts, and memories when you exit. Nothing is saved to disk.",
        },
        {
          icon: FileText,
          name: "Compact Dialogue View",
          badge: "Display Option",
          action: "Toggle collapsible conversation turns.",
          outcome: "Keeps long chat histories tidy by collapsing long messages into compact cards.",
        },
        {
          icon: Trash2,
          name: "Delete Conversations",
          badge: "Clean Up",
          action: "Delete chats from the History page.",
          outcome: "Permanently removes chosen conversations to free up space and keep your list clean.",
        },
      ],
    },
    {
      id: "appearance",
      label: "Look & Feel",
      badge: "Theme & Colors",
      icon: Palette,
      overview: "Customize Vox's visual theme and choose your favorite ambient accent color.",
      controls: [
        {
          icon: Palette,
          name: "Dark & Light Mode",
          badge: "Theme Mode",
          action: "Switch between Dark Obsidian and Light Glass themes.",
          outcome: "Dark mode gives a high-contrast deep space look; Light mode gives a clean, frosted glass aesthetic.",
        },
        {
          icon: Sparkles,
          name: "Accent Color Seed",
          badge: "Custom Color",
          action: "Pick any color from the color wheel.",
          outcome: "Instantly updates glowing lights, borders, buttons, and animations across the entire app.",
        },
        {
          icon: RotateCcw,
          name: "Restore Defaults",
          badge: "Reset",
          action: "Click Restore Defaults at the bottom of Settings.",
          outcome: "Resets all colors, themes, and layouts back to the original Vox Cyan (#00dbe9).",
        },
      ],
    },
  ],
};
