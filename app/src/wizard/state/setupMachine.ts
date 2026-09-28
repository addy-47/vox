import { setup, assign } from 'xstate';

export type SetupStep =
  | 'welcome'
  | 'checking'
  | 'downloading'
  | 'audio'
  | 'testing'
  | 'completed';

export interface SetupContext {
  currentStep: SetupStep;
  manifestReady: boolean;
  setupComplete: boolean;
  error?: string;
  systemInfo?: {
    cpuCount: number;
    ramTotal: number;
    diskAvailable: number;
  };
  maxReachedIndex: number;
}

export type SetupEvent =
  | { type: 'GO_TO'; targetStep: SetupStep }
  | { type: 'MANIFEST_READY' }
  | { type: 'NEXT' }
  | { type: 'SUCCESS' }
  | { type: 'FAILURE'; message: string }
  | { type: 'BACK' }
  | { type: 'FINISH' }
  | { type: 'ERROR'; message: string }
  | { type: 'RETRY' };

export const setupMachine = setup({
  types: {
    context: {} as SetupContext,
    events: {} as SetupEvent,
  },
}).createMachine({
  id: 'setup',
  initial: 'welcome',
  context: {
    currentStep: 'welcome',
    manifestReady: false,
    setupComplete: false,
    maxReachedIndex: 0,
  },

  on: {
    GO_TO: [
      { target: '.welcome', guard: ({ context, event }) => event.targetStep === 'welcome' && context.maxReachedIndex >= 0 },
      { target: '.checking', guard: ({ context, event }) => event.targetStep === 'checking' && context.maxReachedIndex >= 1 },
      { target: '.downloading', guard: ({ context, event }) => event.targetStep === 'downloading' && context.maxReachedIndex >= 2 },
      { target: '.audio', guard: ({ context, event }) => event.targetStep === 'audio' && context.maxReachedIndex >= 3 },
      { target: '.testing', guard: ({ context, event }) => event.targetStep === 'testing' && context.maxReachedIndex >= 4 },
      { target: '.completed', guard: ({ context, event }) => event.targetStep === 'completed' && context.maxReachedIndex >= 5 }
    ]
  },
  states: {
    welcome: {
      entry: assign({ currentStep: 'welcome' }),
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
      entry: assign({ currentStep: 'checking', maxReachedIndex: ({ context }) => Math.max(context.maxReachedIndex, 1) }),
      on: {
        SUCCESS: 'downloading',
        FAILURE: {
          target: 'error',
          actions: assign({ error: ({ event }) => event.message || 'System check failed' })
        },
        BACK: 'welcome'
      }
    },
    downloading: {
      entry: assign({ currentStep: 'downloading', maxReachedIndex: ({ context }) => Math.max(context.maxReachedIndex, 2) }),
      on: {
        FINISH: {
          target: 'audio',
          actions: assign({ setupComplete: true })
        },
        BACK: 'checking',
        ERROR: {
          target: 'downloading', // Stay in downloading but show error in UI
          actions: assign({ error: ({ event }) => event.message })
        },
        RETRY: {
          target: 'downloading',
          actions: assign({ error: undefined })
        }
      }
    },
    audio: {
      entry: assign({ currentStep: 'audio', maxReachedIndex: ({ context }) => Math.max(context.maxReachedIndex, 3) }),
      on: {
        NEXT: 'testing',
        BACK: 'downloading'
      }
    },
    testing: {
      entry: assign({ currentStep: 'testing', maxReachedIndex: ({ context }) => Math.max(context.maxReachedIndex, 4) }),
      on: {
        NEXT: 'completed',
        BACK: 'audio'
      }
    },
    completed: {
      entry: assign({ currentStep: 'completed', maxReachedIndex: ({ context }) => Math.max(context.maxReachedIndex, 5) }),
      on: {
        BACK: 'testing'
      }
    },
    error: {
      on: {
        RETRY: 'checking',
        BACK: 'welcome'
      }
    }
  }
});
