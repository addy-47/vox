export const loadPersona = () => import("./persona/PersonaCard").then(m => ({ default: m.PersonaCard }));
export const loadModels = () => import("./models/ModelsCard").then(m => ({ default: m.ModelsCard }));
export const loadRealtime = () => import("./realtime/RealtimeCard").then(m => ({ default: m.RealtimeCard }));
export const loadWorkingMemory = () => import("./working_memory/WorkingMemoryCard").then(m => ({ default: m.WorkingMemoryCard }));
export const loadPersonalMemory = () => import("./personal_memory/PersonalMemoryCard").then(m => ({ default: m.PersonalMemoryCard }));
export const loadAppearance = () => import("./appearance/AppearanceCard").then(m => ({ default: m.AppearanceCard }));
export const loadInteraction = () => import("./interaction/InteractionCard").then(m => ({ default: m.InteractionCard }));

export const preloadSettingsCardChunks = () =>
  Promise.all([
    loadPersona(),
    loadModels(),
    loadRealtime(),
    loadWorkingMemory(),
    loadPersonalMemory(),
    loadAppearance(),
    loadInteraction(),
  ]);
