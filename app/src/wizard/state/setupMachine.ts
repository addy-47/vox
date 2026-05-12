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
        FAILURE: 'error',
        RETRY: 'checking'
      }
    },
    downloading: {
      on: {
        FINISH: 'audio',
        PROGRESS: {
          actions: assign(({ context, event }) => {
            const data = (event as any).data;
            if (!data) return {};
            
            const { model_id, progress, step, bytes_downloaded, total_bytes } = data;
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
            };
          })
        },
        ERROR: 'error'
      }
    },
    audio: {
      on: {
        NEXT: 'testing'
      }
    },
    testing: {
      on: {
        NEXT: 'completed'
      }
    },
    completed: {
      type: 'final'
    },
    error: {
      on: {
        RETRY: 'checking'
      }
    }
  }
});
