import type { LucideIcon } from "lucide-react";
import type { SetupStep } from "@/wizard/state/setupMachine";
import {
  Activity,
  AudioWaveform,
  CheckCircle2,
  Ear,
  Gauge,
  House,
  Layers,
  Mic2,
  Radio,
  ShieldCheck,
} from "lucide-react";
export interface WizardStep {
  id: SetupStep;
  label: string;
  icon: LucideIcon;
}

export const WIZARD_STEPS: WizardStep[] = [
  { id: "welcome", label: "Welcome", icon: House },
  { id: "checking", label: "Quick Check", icon: Gauge },
  { id: "downloading", label: "Downloads", icon: Layers },
  { id: "audio", label: "Microphone", icon: Mic2 },
  { id: "testing", label: "Try It Out", icon: Ear },
  { id: "completed", label: "All Set", icon: CheckCircle2 },
];

export interface StepHeader {
  step: string;
  title: string;
  description: string;
}

export const WIZARD_STEP_HEADERS: Record<string, StepHeader> = {
  checking: {
    step: "Step 2 of 6 · One Quick Check",
    title: "Let's Check Your Computer",
    description:
      "A few seconds to confirm Vox has what it needs to run. Nothing is installed yet.",
  },
  selection: {
    step: "Step 3 of 6 · What Vox Needs",
    title: "What Vox Needs to Download",
    description:
      "Because Vox runs on your own computer, the parts that do the work get downloaded once. Everything below is safe to keep.",
  },
  syncing: {
    step: "Step 3 of 6 · Downloading",
    title: "Downloading",
    description:
      "This happens one time. After it finishes, Vox works even with the internet switched off.",
  },
  audio: {
    step: "Step 4 of 6 · Your Microphone",
    title: "Which Microphone?",
    description:
      "This is the one Vox will listen through. You can point it somewhere else later in Settings.",
  },
  testing: {
    step: "Step 5 of 6 · Say Something",
    title: "Say Something",
    description:
      "Speak the way you would in a call. Your words should show up here a moment later.",
  },
  completed: {
    step: "Step 6 of 6 · All Set",
    title: "You're All Set",
    description: "Vox is ready and will stay out of your way until you want it.",
  },
};

export interface WelcomeSubStep {
  title: string;
  tagline: string;
}

export const WELCOME_SUBSTEPS: WelcomeSubStep[] = [
  {
    title: "Meet Vox.",
    tagline:
      "Talk to your computer the way you'd talk to a person. Vox hears you, thinks it over, and answers back.",
  },
  {
    title: "Nothing Leaves Your Machine",
    tagline:
      "No account to create, nothing to log in to, no waiting on someone else's computer. Vox works on its own.",
  },
  {
    title: "You See Every Word",
    tagline:
      "Your transcript appears while you talk, so you can catch a slip before it goes anywhere.",
  },
];

export interface FeatureCard {
  title: string;
  desc: string;
  icon: LucideIcon;
}

export const WELCOME_FEATURE_CARDS: FeatureCard[] = [
  { icon: ShieldCheck, title: "Private", desc: "Stays on your computer" },
  { icon: Gauge, title: "Quick", desc: "Answers while you talk" },
  { icon: Radio, title: "Always Ready", desc: "One click from your menu bar" },
  { icon: Activity, title: "In Your Sight", desc: "You see every word it hears" },
];

export const WELCOME_TOOLTIPS = {
  status: {
    title: "It's Listening",
    desc: "This dot lights up the moment Vox hears you — and only when you ask it to.",
  },
  mic: {
    title: "Talk",
    desc: "Click and hold to talk, or set a shortcut you already use. Either way, you decide when it listens.",
  },
  copy: {
    title: "Copy It",
    desc: "Copies your finished transcript, ready to paste straight into whatever you're writing.",
  },
  history: {
    title: "Recent Chats",
    desc: "Step back through your last few conversations without opening anything else.",
  },
  renderer: {
    title: "Live Transcript",
    desc: "Your words show up as you say them, so you can fix a mistake before it's sent.",
  },
} as const;

export const WELCOME_STEP_LABEL = "Step 1 of 6 · Welcome";

export const WELCOME_NAV_COPY = {
  prev: "Back",
  next: "Next",
} as const;

export const WELCOME_DEMO_DEFAULT = {
  title: "See How It Works",
  desc: "Hover over, tap, or Tab to any part of Vox's window to see what it does.",
  statsActive: "Active",
  demoRam: "42MB",
  listeningHint: "Listening... say something.",
} as const;

export const SYSTEM_CHECK_LABELS = ["FREE SPACE", "MICROPHONE", "FOLDER ACCESS", "YOUR MACHINE"] as const;

export const SYSTEM_CHECK_COPY = {
  checkingValue: "Checking...",
  measuringSub: "Looking at your free space...",
  testingSub: "Looking for a microphone...",
  verifyingSub: "Checking folder access...",
  scanningSub: "Reading your system...",
  diskOkSub: "Plenty of room for everything Vox needs",
  insufficient: "NOT ENOUGH",
  tenGbNote: "Free up around 10 GB to continue",
  diskUnknown: "UNKNOWN",
  diskUnknownSub: "Could not measure free space",
  detected: "FOUND",
  missing: "NONE FOUND",
  micOkSub: "Ready to listen",
  micMissingSub: "No microphone plugged in",
  granted: "YES",
  denied: "BLOCKED",
  writeOkSub: "Vox can save its files here",
  writeDeniedSub: "Vox needs permission to this folder",
  threadsSuffix: "CPU THREADS",
  ramDetected: "GB of memory",
  storagePermsError: "Vox needs free space and folder access to keep going.",
  micWarnError: "No microphone found — you can pick one later in Settings.",
  checkFailedTitle: "Check Failed",
  micWarnTitle: "No Microphone",
} as const;

export interface StatusCardData {
  label: string;
  value: string;
  subValue: string;
  icon: LucideIcon;
}

export const COMPLETED_STATUS_CARDS: StatusCardData[] = [
  { label: "VOICE ENGINE", value: "READY", subValue: "Runs here, no internet needed", icon: AudioWaveform },
  { label: "VOICE MODELS", value: "INSTALLED", subValue: "Already sitting on this computer", icon: Layers },
  { label: "MENU BAR", value: "READY", subValue: "Vox is waiting for you there", icon: Radio },
  { label: "PRIVACY", value: "LOCAL ONLY", subValue: "Your voice never leaves this computer", icon: ShieldCheck },
];

export const COMPLETED_TIP = {
  title: "One Thing to Try",
  text: "Click the Vox icon in your menu bar and start talking. If you'd rather skip the menu, set a shortcut in Settings.",
} as const;

export const WIZARD_STATUS_COPY = {
  unknownState: "Unknown State",
} as const;

export const WIZARD_ERROR_COPY = {
  title: "Something Didn't Work",
  retry: "Try Again",
  backToStart: "Start Over",
} as const;

export const AUDIO_SETUP_COPY = {
  liveLabel: "Your Voice",
  listTitle: "Which microphone?",
  empty: "No microphones found — plug one in, or pick one later in Settings.",
} as const;

export const LIVE_TEST_COPY = {
  confirmContinue: "Sounds Good",
  engineErrorTitle: "Vox Can't Start Yet",
  tryAgain: "Try Again",
  voiceLevel: "Mic Level",
  demoHint: "Your Words",
  processed: "Done",
  voiceDetected: "Hearing You",
  listening: "Listening...",
  textReceived: "Heard You",
  waiting: "Waiting",
  engineStarting: "Warming up...",
  waitingForVoice: "Go ahead, say something",
  speakNow: "Say anything — your words will appear here.",
  startingModels: "Getting ready...",
} as const;

export const MODEL_SETUP_COPY = {
  readyTitle: "Everything's Ready",
  readyBody: "Everything Vox needs is downloaded and ready to use.",
  retryLoad: "Try Again",
  back: "Back",
  downloadError: "Download Problem",
  totalSuffix: "total",
  changeLaterNote: "You can add or remove these any time in Settings — nothing here is locked in.",
  catalogLoadError: "Couldn't load the download list.",
} as const;

/**
 * Human-readable names for the manifest's model categories. Written from a new
 * user's point of view — what the piece does for them, not the subsystem name.
 */
export const MODEL_CATEGORY_META = [
  { id: "vad", label: "Hearing When You Speak", fallback: "Knows when you start and stop talking" },
  { id: "stt", label: "Turning Speech Into Text", fallback: "Works out what you actually said" },
  { id: "translit", label: "Hindi Typed in English", fallback: "Writes spoken Hindi in English letters" },
  { id: "embedding", label: "Remembering Context", fallback: "Helps Vox connect related memories" },
  { id: "llm", label: "Thinking and Replying", fallback: "Comes up with what to say back" },
  { id: "tts", label: "Speaking Out Loud", fallback: "Reads its reply back to you" },
] as const;

/**
 * Progress-step wording for the download view. The backend `SetupStep` enum
 * ("Downloading", "Extracting", "Verifying", "Cancelled") is internal
 * machinery, so it is translated here rather than shown raw.
 */
export const MODEL_PROGRESS_STEPS: Record<string, string> = {
  idle: "Waiting to start",
  queued: "Waiting to start",
  downloading: "Downloading",
  extracting: "Unpacking",
  verifying: "Checking the files",
  completed: "Ready",
  failed: "Didn't finish",
  cancelled: "Stopped",
  unknown: "Working...",
};

export const MODEL_CATEGORY_COPY = {
  mandatory: "Required",
  optional: "Optional",
  versionPrefix: "Version",
  expandPrefix: "Expand",
  collapsePrefix: "Collapse",
} as const;

export const WIZARD_CTA_LABELS = {
  beginSetup: "Let's Go",
  beginSynchronization: "Download",
  synchronizing: "Downloading...",
  fetchingCatalog: "Loading...",
  continueToVerification: "Continue",
  startUsingVox: "Start Talking",
  continueSetup: "Continue",
  returnToSelection: "Change Selection",
  back: "Back",
  skip: "Skip for Now",
  processing: "Working...",
  proceed: "Continue",
  errorTitle: "Something Went Wrong",
  continueToModels: "Continue",
} as const;
