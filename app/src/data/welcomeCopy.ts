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
  welcome: {
    step: "Step 1 of 6 · Getting Started",
    title: "Welcome to Vox.",
    description:
      "Vox is your personal voice assistant. It listens, answers, and lives quietly in your menu bar — everything runs privately on your computer.",
  },
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
      "Vox is your personal voice assistant — fast, always ready, and fully private. Everything runs on your computer, never in the cloud.",
  },
  {
    title: "Your Voice Assistant",
    tagline:
      "Vox listens and replies using on-device voice AI. It works offline, instantly, and privately on your hardware.",
  },
  {
    title: "The Voice Overlay",
    tagline:
      "When you speak, a live transcript follows your voice. It appears the moment you talk and fades when you stop.",
  },
];

export interface FeatureCard {
  title: string;
  desc: string;
  icon: LucideIcon;
}

export const WELCOME_FEATURE_CARDS: FeatureCard[] = [
  { icon: ShieldCheck, title: "Private", desc: "Never leaves your device" },
  { icon: Zap, title: "Instant", desc: "Replies in under a second" },
  { icon: Globe, title: "Always On", desc: "Lives in your menu bar" },
  { icon: Activity, title: "Ready", desc: "Set up in minutes" },
];

export const WELCOME_TOOLTIPS = {
  status: {
    title: "Live Status",
    desc: "Shows when Vox is listening for your voice in the background.",
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
    desc: "Words appear as you speak them — no waiting for the transcript.",
  },
} as const;

export const WELCOME_DEMO_DEFAULT = {
  title: "Interactive Demo",
  desc: "Hover over Vox's screen to see what each part does.",
} as const;

export const SYSTEM_CHECK_LABELS = ["STORAGE", "AUDIO", "PERMISSIONS", "HARDWARE"] as const;

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

export const WIZARD_CTA_LABELS = {
  beginSetup: "Begin Setup",
  beginSynchronization: "Download Models",
  synchronizing: "Downloading...",
  fetchingCatalog: "Loading Models...",
  proceedToModelSync: "Continue to Download",
  continueToVerification: "Continue",
  startUsingVox: "Start Using Vox",
  continueSetup: "Continue Setup",
  returnToSelection: "Return to Selection",
} as const;
