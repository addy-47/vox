export interface TestClip {
  id: string;
  label: string;
  desc: string;
}

export const TEST_CLIPS: TestClip[] = [
  { id: "short_en", label: "English Speech", desc: "Short English conversational audio" },
  { id: "short_hi", label: "Hindi Speech", desc: "Short Hindi conversational audio" },
  { id: "hinglish", label: "Hinglish Mix", desc: "Code-switched Hinglish sample" },
  { id: "command", label: "Voice Command", desc: "Desktop action command phrase" },
  { id: "expressive", label: "Long Narrative", desc: "Multi-sentence continuous stream" },
];

export const GOVERNOR_LABELS: Record<string, string> = {
  powersave: "Power Saver",
  performance: "High Performance",
  schedutil: "Balanced",
  ondemand: "Adaptive",
};
