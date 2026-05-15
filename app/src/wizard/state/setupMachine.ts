import { createMachine, assign } from 'xstate';

export type SetupStep = 
  | 'welcome'
  | 'checking'
  | 'downloading'
  | 'audio'
  | 'testing'
  | 'completed';

export interface ModelProgress {
  id: string;
  progress: number;
  step: string;
  bytesDownloaded: number;
  totalBytes: number;
}

export interface SetupContext {
  currentStep: SetupStep;
  models: Record<string, ModelProgress>;
  totalProgress: number;
  manifestReady: boolean;
  setupComplete: boolean;
  error?: string;
  systemInfo?: {
    cpuCount: number;
    ramTotal: number;
    diskAvailable: number;
  };
}

export const setupMachine = createMachine({
  id: 'setup',
  initial: 'welcome',
  context: {
    currentStep: 'welcome',
    models: {},
    totalProgress: 0,
    manifestReady: false,
    setupComplete: false,
  } as SetupContext,
  states: {
    welcome: {
      on: {
        MANIFEST_READY: {
          actions: assign({ manifestReady: true })
        },
        NEXT: {
          target: 'checking',
          guard: ({ context }) => context.manifestReady
        }
      }
    },
    checking: {
      on: {
        SUCCESS: 'downloading',
        FAILURE: {
          target: 'error',
          actions: assign({ error: ({ event }) => (event as any).message || 'System check failed' })
        },
        BACK: 'welcome'
      }
    },
    downloading: {
      on: {
        FINISH: {
          target: 'audio',
          actions: assign({ setupComplete: true })
        },
        BACK: 'checking',
        PROGRESS: {
          actions: assign(({ context, event }) => {
            const data = (event as any).data;
            if (!data) return {};
            
            const { model_id, progress, step, bytes_downloaded, total_bytes, error } = data;
            const newModels = {
              ...context.models,
              [model_id]: {
                id: model_id,
                progress,
                step,
                bytesDownloaded: bytes_downloaded,
                totalBytes: total_bytes,
              }
            };
            
            const modelList = Object.values(newModels);
            const totalProgress = modelList.length > 0 
              ? modelList.reduce((acc: number, m) => acc + (m as ModelProgress).progress, 0) / modelList.length
              : 0;

            return {
              models: newModels,
              totalProgress,
              error: error || context.error,
            };
          })
        },
        ERROR: {
          target: 'downloading', // Stay in downloading but show error in UI
          actions: assign({ error: ({ event }) => (event as any).message })
        },
        RETRY: {
          target: 'downloading',
          actions: assign({ error: undefined })
        }
      }
    },
    audio: {
      on: {
        NEXT: 'testing',
        BACK: 'downloading'
      }
    },
    testing: {
      on: {
        NEXT: 'completed',
        BACK: 'audio'
      }
    },
    completed: {
      type: 'final'
    },
    error: {
      on: {
        RETRY: 'checking',
        BACK: 'welcome'
      }
    }
  }
});
