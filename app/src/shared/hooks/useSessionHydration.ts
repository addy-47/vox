import { useEffect, useRef } from "react";
import { getSettings } from "@/services/settingsService";
import { getRuntimeSnapshot } from "@/services/pipelineService";
import { getTurns } from "@/services/historyService";

interface HydrationResult {
  interactionMode: string | null;
  pipelineMode: "modular" | "realtime" | null;
  cpuWarning: { governor: string } | null;
  activeSessionId: number | null;
  dialogueHistory: Array<{ user: string; assistant: string; id: number }>;
  turnIdCounter: number;
}

export function useSessionHydration(
  onHydrated: (result: HydrationResult) => void,
): void {
  const hydrated = useRef(false);

  useEffect(() => {
    let cancelled = false;

    const hydrate = async () => {
      try {
        const res = await getSettings();
        const interactionMode = res.settings?.interaction?.mode ?? null;
        const pipelineMode = res.settings?.interaction?.pipeline_mode
          ? (res.settings.interaction.pipeline_mode.toLowerCase() as "modular" | "realtime")
          : null;

        const snap = await getRuntimeSnapshot();
        if (cancelled) return;

        const cpuWarning = snap?.cpu_governor && !snap.cpu_governor_optimal
          ? { governor: snap.cpu_governor }
          : null;

        let activeSessionId: number | null = null;
        let dialogueHistory: Array<{ user: string; assistant: string; id: number }> = [];
        let turnIdCounter = 0;

        if (snap?.conversation_id && snap.conversation_id !== 0) {
          activeSessionId = snap.conversation_id;
          const turns = await getTurns(snap.conversation_id);
          if (cancelled) return;
          const history = turns.slice(-100).map((t) => ({
            user: t.user_text,
            assistant: t.assistant_text,
            id: t.turn_id,
          }));
          dialogueHistory = history;
          if (history.length > 0) {
            turnIdCounter = Math.max(...history.map((h) => h.id));
          }
        }

        if (!cancelled) {
          onHydrated({ interactionMode, pipelineMode, cpuWarning, activeSessionId, dialogueHistory, turnIdCounter });
          hydrated.current = true;
        }
      } catch (e) {
        if (!cancelled) {
          console.warn("[SessionHydration] Failed to sync initial state:", e);
        }
      }
    };

    // Run hydration after subscription is established (caller ensures this)
    const id = setTimeout(hydrate, 0);
    return () => { cancelled = true; clearTimeout(id); };
  }, [onHydrated]);
}
