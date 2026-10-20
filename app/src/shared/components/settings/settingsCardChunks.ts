export const loadPersona = () => import("./persona/PersonaCard").then(m => ({ default: m.PersonaCard }));
export const loadSpeech = () => import("./speech/SpeechCard").then(m => ({ default: m.SpeechCard }));
export const loadLanguage = () => import("./language/LanguageCard").then(m => ({ default: m.LanguageCard }));
export const loadRealtime = () => import("./realtime/RealtimeCard").then(m => ({ default: m.RealtimeCard }));
export const loadMemory = () => import("./memory/MemoryCard").then(m => ({ default: m.MemoryCard }));
export const loadAppearance = () => import("./appearance/AppearanceCard").then(m => ({ default: m.AppearanceCard }));
export const loadInteraction = () => import("./interaction/InteractionCard").then(m => ({ default: m.InteractionCard }));

export const preloadSettingsCardChunks = () =>
  Promise.all([
    loadPersona(),
    loadSpeech(),
    loadLanguage(),
    loadRealtime(),
    loadMemory(),
    loadAppearance(),
    loadInteraction(),
  ]);
