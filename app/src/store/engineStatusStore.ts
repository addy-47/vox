import { create } from "zustand";
import { listen } from "@/services/transport";

export interface EngineRestartStatePayload {
  restarting: boolean;
  target?: string | null;
}

interface EngineStatusState {
  restarting: boolean;
  target: string | null;
  initListeners: () => Promise<() => void>;
}

export const useEngineStatusStore = create<EngineStatusState>((set) => ({
  restarting: false,
  target: null,

  initListeners: async () => {
    const unlisten = await listen<EngineRestartStatePayload>(
      "engine-restart-state",
      (event) => {
        set({
          restarting: event.payload.restarting,
          target: event.payload.target ?? null,
        });
      }
    );
    return () => {
      unlisten();
    };
  },
}));