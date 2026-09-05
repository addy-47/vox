import type { LucideIcon } from "lucide-react";
import {
  Activity,
  CheckCircle2,
  Globe,
  Home,
  Mic2,
  Settings2,
  Shield,
  ShieldCheck,
  Sparkles,
  Zap,
} from "lucide-react";

export interface WizardStep {
  id: string;
  label: string;
  icon: LucideIcon;
}

export const WIZARD_STEPS: WizardStep[] = [
  { id: "welcome", label: "Welcome", icon: Home },
  { id: "checking", label: "System Check", icon: Shield },
  { id: "downloading", label: "AI Models", icon: Settings2 },
  { id: "audio", label: "Audio Pipeline", icon: Mic2 },
  { id: "testing", label: "Live Test", icon: Sparkles },
  { id: "completed", label: "Setup Done", icon: CheckCircle2 },
];

export interface StepHeader {
  step: string;
  title: string;
  description: string;
}

export const WIZARD_STEP_HEADERS: Record<string, StepHeader> = {
  checking: {
    step: "Step 2 of 6 · Checking Your Computer",
    title: "Checking Your Computer",
    description:
      "Making sure your computer is ready for Vox. We verify everything works before we move on.",
  },
  selection: {
    step: "Step 3 of 6 · Choosing Voice Models",
    title: "Choose Your Voice Models",
    description:
      "Pick which voice features Vox uses. The essential ones make conversation work; the extra ones unlock smarter replies and memory.",
  },
  syncing: {
    step: "Step 3 of 6 · Downloading Voice Models",
    title: "Downloading Voice Models",
    description:
      "Vox is downloading the voice models to your computer. They run locally, so your voice never leaves your device.",
  },
  audio: {
    step: "Step 4 of 6 · Choosing Your Microphone",
    title: "Choose Your Microphone",
    description:
      "Pick the microphone Vox will listen to. This is how Vox hears you.",
  },
  testing: {
    step: "Step 5 of 6 · Test Your Voice",
    title: "Try It Out",
    description:
      "Say something and watch Vox understand you in real time — entirely on your computer.",
  },
  completed: {
    step: "Step 6 of 6 · All Done",
    title: "Setup Complete.",
    description: "Vox is installed and ready to use.",
  },
};

export interface WelcomeSubStep {
  title: string;
  tagline: string;
}

export const WELCOME_SUBSTEPS: WelcomeSubStep[] = [
  {
    title: "Welcome to Vox.",
    tagline:
      "Vox listens to your voice, understands what you say, and talks back — all on your own computer. It lives quietly in your menu bar, ready when you need it.",
  },
  {
    title: "The AI Core",
    tagline:
      "Powered by on-device voice AI. Everything runs locally on your hardware — fast, private, and offline.",
  },
  {
    title: "Voice Overlay",
    tagline:
      "A live transcript that follows your voice. It appears the moment you speak and fades when you stop.",
  },
];

export interface FeatureCard {
  title: string;
  desc: string;
  icon: LucideIcon;
}

export const WELCOME_FEATURE_CARDS: FeatureCard[] = [
  { icon: ShieldCheck, title: "Privacy", desc: "100% On-device" },
  { icon: Zap, title: "Speed", desc: "Instant Responses" },
  { icon: Globe, title: "Always On", desc: "Lives in Your Menu Bar" },
  { icon: Activity, title: "Status", desc: "Ready to Start" },
];

export const WELCOME_TOOLTIPS = {
  status: {
    title: "Listening",
    desc: "Shows when Vox is hearing you. It only listens while you hold the button or talk.",
  },
  mic: {
    title: "Push-To-Talk",
    desc: "Hold the button to talk. Gives you full control over when Vox is listening.",
  },
  copy: {
    title: "Instant Copy",
    desc: "Click once to copy the finished transcript, ready to paste anywhere.",
  },
  history: {
    title: "Recent Chats",
    desc: "Browse your last few conversations without leaving the current window.",
  },
  renderer: {
    title: "Instant Words",
    desc: "Your words appear on screen as you speak, almost immediately.",
  },
} as const;

export const WELCOME_DEMO_DEFAULT = {
  title: "Interactive Demo",
  desc: "Hover over Vox's screen to see what each part does.",
  statsActive: "Active",
  listeningHint: "Listening... your words will appear here.",
} as const;

export const SYSTEM_CHECK_LABELS = ["STORAGE SPACE", "MICROPHONE", "PERMISSIONS", "HARDWARE"] as const;

export interface StatusCardData {
  label: string;
  value: string;
  subValue: string;
}

export const COMPLETED_STATUS_CARDS: StatusCardData[] = [
  { label: "VOICE ENGINE", value: "READY", subValue: "Runs completely offline" },
  { label: "VOICE MODELS", value: "READY", subValue: "Configured on your device" },
  { label: "SYSTEM TRAY", value: "RUNNING", subValue: "Access from your menu bar" },
  { label: "PRIVACY", value: "SECURED", subValue: "100% private & safe" },
];

export const COMPLETED_TIP = {
  title: "Quick Tip",
  text: "Click the Vox icon in your menu bar or press your shortcut key to start talking.",
} as const;

export const WIZARD_STATUS_COPY = {
  systemReady: "System Ready",
  unknownState: "Unknown State",
} as const;

export const AUDIO_SETUP_COPY = {
  liveLabel: "Your Voice",
  listTitle: "Choose a Microphone",
  empty: "No microphones detected",
} as const;

export const LIVE_TEST_COPY = {
  confirmContinue: "Confirm & Continue",
  engineErrorTitle: "Couldn't Start the Voice Engine",
  tryAgain: "Try Again",
  voiceLevel: "Voice Level",
  demoHint: "Your Words",
  processed: "Processed",
  voiceDetected: "Voice Detected",
  listening: "Listening...",
  textReceived: "Text Received",
  waiting: "Waiting...",
} as const;

export const MODEL_SETUP_COPY = {
  readyTitle: "Models Ready",
  readyBody: "All selected voice models have been downloaded and checked on your system.",
  retryLoad: "Retry Load",
  back: "Back",
  downloadError: "Download Error",
  totalSuffix: "Total",
} as const;

export const MODEL_CATEGORY_COPY = {
  mandatory: "Mandatory",
  optional: "Optional",
} as const;

export const WIZARD_CTA_LABELS = {
  beginSetup: "Begin Setup",
  beginSynchronization: "Download Models",
  synchronizing: "Downloading...",
  fetchingCatalog: "Loading Models...",
  continueToVerification: "Continue",
  startUsingVox: "Start Using Vox",
  continueSetup: "Continue Setup",
  returnToSelection: "Return to Selection",
  back: "Back",
  skip: "Skip",
  processing: "Processing...",
  continueToModels: "Continue to Models",
} as const;
