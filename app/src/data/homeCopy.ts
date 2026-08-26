export type TestClipLang = "en" | "hi";

export interface TestClip {
  id: string;
  lang: TestClipLang;
  label: string;
  desc: string;
}

export const TEST_CLIPS: TestClip[] = [
  // English clips
  { id: "clip_01_en_briefing", lang: "en", label: "Morning Briefing", desc: "Schedule & calendar query" },
  { id: "clip_02_en_weather", lang: "en", label: "Weather Query", desc: "Current forecast & rain check" },
  { id: "clip_03_en_code", lang: "en", label: "Code Refactoring", desc: "Rust async & mutex optimization" },
  { id: "clip_04_en_summary", lang: "en", label: "Meeting Summary", desc: "Design review action items" },
  { id: "clip_05_en_timer", lang: "en", label: "Focus Timer", desc: "25-min Pomodoro & mute notifications" },

  // Hindi clips
  { id: "clip_06_hi_greeting", lang: "hi", label: "सुबह की ब्रीफिंग", desc: "आज का शेड्यूल और अगली मीटिंग" },
  { id: "clip_07_hi_weather", lang: "hi", label: "मौसम की जानकारी", desc: "आज का मौसम और बारिश का अनुमान" },
  { id: "clip_08_hi_reminder", lang: "hi", label: "टास्क रिमाइंडर", desc: "शाम की प्रोजेक्ट रिव्यू मीटिंग" },
  { id: "clip_09_hi_system_cmd", lang: "hi", label: "सिस्टम कमांड", desc: "टर्मिनल और हाई परफॉरमेंस सर्वर" },
  { id: "clip_10_hi_qa", lang: "hi", label: "तकनीकी सवाल", desc: "स्पीच-टू-टेक्स्ट मॉडल एक्सप्लेनेशन" },
];

export const GOVERNOR_LABELS: Record<string, string> = {
  powersave: "Power Saver",
  performance: "High Performance",
  schedutil: "Balanced",
  ondemand: "Adaptive",
};
