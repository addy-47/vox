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
    step: "Step 1.0 • Initialization",
    title: "Welcome to Vox.",
    description:
      "Vox is a low-latency audio intelligence system designed to live in your system tray and provide real-time interaction.",
  },
  checking: {
    step: "Step 2.0 • Infrastructure",
    title: "Environment Scan",
    description:
      "Analyzing system environment for local AI execution. We ensure your hardware meets the requirements for a seamless experience.",
  },
  selection: {
    step: "Step 2.1 • Selection",
    title: "AI Components",
    description:
      "Customize your local AI stack. Mandatory core ensures functional interaction, while optional layers unlock deep reasoning.",
  },
  syncing: {
    step: "Step 2.2 • Synchronizing",
    title: "Deploying AI",
    description:
      "Vox is deploying selected components to your local hardware. This process is fully encrypted and sandboxed.",
  },
  audio: {
    step: "Step 3.0 • Audio Input",
    title: "Device Selection",
    description:
      "Configuring audio input for real-time interaction. Select your primary microphone to enable voice understanding.",
  },
  testing: {
    step: "Step 4.0 • Voice Showcase",
    title: "Voice Experience",
    description:
      "Experience real-time local Voice Activity Detection (VAD) and Speech-to-Text (STT) understanding. Say something to watch the live local transcription.",
  },
  completed: {
    step: "Step 5.0 • Completion",
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
      "Vox is a low-latency audio intelligence system designed to live in your system tray and provide real-time interaction.",
  },
  {
    title: "The AI Core",
    tagline:
      "Powered by edge AI models. Experience low-latency intelligence that processes everything locally on your hardware.",
  },
  {
    title: "Vox Live HUD",
    tagline:
      "An AI transcription overlay that follows your voice. It appears instantly when you speak and disappears when finished.",
  },
];

export interface FeatureCard {
  title: string;
  desc: string;
  icon: LucideIcon;
}

export const WELCOME_FEATURE_CARDS: FeatureCard[] = [
  { icon: ShieldCheck, title: "Privacy", desc: "100% On-device" },
  { icon: Zap, title: "Latency", desc: "Low-Latency Inference" },
  { icon: Globe, title: "Native", desc: "System Integration" },
  { icon: Activity, title: "Status", desc: "Awaiting Initialization" },
];

export const WELCOME_TOOLTIPS = {
  status: {
    title: "Live Status",
    desc: "Passive VAD detection shows when Vox is actively listening to your environment.",
  },
  mic: {
    title: "Push-To-Talk",
    desc: "Override passive listening for absolute control. Perfect for high-precision input in crowded environments.",
  },
  copy: {
    title: "Instant Copy",
    desc: "One-click to move the finalized transcript to your clipboard for any application.",
  },
  history: {
    title: "Ephemeral History",
    desc: "Quickly browse the last 10 transcripts without leaving your current window.",
  },
  renderer: {
    title: "Fluid Streaming",
    desc: "Transcripts stream character-by-character with sub-50ms latency.",
  },
} as const;

export const WELCOME_DEMO_DEFAULT = {
  title: "Interactive Demo",
  desc: "Hover over HUD elements to explore features. This visual guide mirrors the real-time overlay — showing exactly what you will see when speaking.",
} as const;

export const SYSTEM_CHECK_LABELS = ["STORAGE", "AUDIO", "PERMISSIONS", "HARDWARE"] as const;

export interface StatusCardData {
  label: string;
  value: string;
  subValue: string;
}

export const COMPLETED_STATUS_CARDS: StatusCardData[] = [
  { label: "SPEECH ENGINE", value: "READY", subValue: "Runs completely offline" },
  { label: "AI MODELS", value: "READY", subValue: "Configured on your device" },
  { label: "SYSTEM TRAY", value: "RUNNING", subValue: "Access from your menu bar" },
  { label: "PRIVACY", value: "SECURED", subValue: "100% private & safe" },
];

export const COMPLETED_TIP = {
  title: "Quick Tip",
  text: "Click the Vox icon in your menu bar or press your shortcut key to start talking.",
} as const;

export const WIZARD_CTA_LABELS = {
  beginSetup: "Begin Setup",
  beginSynchronization: "Begin Synchronization",
  synchronizing: "Synchronizing...",
  fetchingCatalog: "Fetching Catalog...",
  proceedToModelSync: "Proceed to Model Sync",
  continueToVerification: "Continue to Verification",
  startUsingVox: "Start Using Vox",
  continueSetup: "Continue Setup",
  returnToSelection: "Return to Selection",
} as const;
