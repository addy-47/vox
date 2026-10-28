import { create } from "zustand";
import { listen, type UnlistenFn } from "@/services/transport";
import {
  type RemoteLinkState,
  type RemoteFailureKind,
  type RemoteStateDto,
  type PairNewDeviceDto,
  type PairedDeviceDto,
  type PairedDeviceDto as PairedDeviceDtoAlias,
  type RemotePairingRequestedPayload,
  type CommandClass,
  type ConnectHostParams,
  type PairDeviceParams,
  type PairingStateDto,
  pairNewDevice as apiPairNewDevice,
  pairDevice as apiPairDevice,
  approvePairing as apiApprovePairing,
  listPairedDevices as apiListPairedDevices,
  revokeDevice as apiRevokeDevice,
  disconnectDevice as apiDisconnectDevice,
  connectHost as apiConnectHost,
  disconnectHost as apiDisconnectHost,
  getLinkState,
  getCommandRouting,
} from "@/services/remoteService";

export interface ConnectedPeer {
  deviceId: string;
  displayName: string;
}

export interface RemoteStoreState {
  linkState: RemoteLinkState;
  failureReason: string | null;
  failureKind: RemoteFailureKind | null;
  connectedPeer: ConnectedPeer | null;
  pendingPairingPrompt: RemotePairingRequestedPayload | null;
  /** Controller-side failure from the ephemeral PairingState (kept out of linkState). */
  pairingFailureReason: string | null;
  isPairingModalOpen: boolean;
  isPairingPending: boolean;
  pairedDevices: PairedDeviceDto[];
  commandRouting: Record<string, CommandClass>;
  loading: boolean;
  error: string | null;

  init: () => Promise<UnlistenFn>;
  hydrate: () => Promise<void>;
  pairNewDevice: () => Promise<PairNewDeviceDto>;
  pairDevice: (params: PairDeviceParams) => Promise<void>;
  openPairingModal: () => void;
  closePairingModal: () => void;
  approvePairing: (decision: "Approve" | "Reject") => Promise<void>;
  dismissPairingPrompt: () => void;
  dismissPairingFailure: () => void;
  dismissFailure: () => void;
  listPairedDevices: () => Promise<PairedDeviceDtoAlias[]>;
  revokeDevice: (deviceId: string) => Promise<void>;
  disconnectDevice: (deviceId: string) => Promise<void>;
  connectHost: (params: ConnectHostParams) => Promise<void>;
  disconnectHost: () => Promise<void>;
}

let subscriptionRefCount = 0;
let teardownListeners: (() => void) | null = null;
let activeInitPromise: Promise<void> | null = null;

export const useRemoteStore = create<RemoteStoreState>((set, get) => ({
  // Ticket 05: initial state is "Unpaired" — no "Disconnected" variant exists
  linkState: "Unpaired",
  failureReason: null,
  failureKind: null,
  connectedPeer: null,
  pendingPairingPrompt: null,
  pairingFailureReason: null,
  isPairingModalOpen: false,
  isPairingPending: false,
  pairedDevices: [],
  commandRouting: {},
  loading: false,
  error: null,

  init: async () => {
    subscriptionRefCount += 1;

    if (!activeInitPromise) {
      activeInitPromise = (async () => {
        await get().hydrate();

        // Ticket 05: canonical event — remote_state_changed is the sole source of truth.
        const unlistenState = await listen<RemoteStateDto>("remote_state_changed", (event) => {
          const dto = event.payload;
          const peer =
            dto.deviceId && dto.displayName
              ? { deviceId: dto.deviceId, displayName: dto.displayName }
              : null;

          // Ticket 05: terminal states that clear the pairing-pending flag
          const isTerminal =
            dto.link === "Connected" ||
            dto.link === "Failed" ||
            dto.link === "Paired" ||
            dto.link === "Unpaired";

          set({
            linkState: dto.link,
            failureReason: dto.failure ?? null,
            failureKind: dto.failureKind ?? null,
            connectedPeer: peer,
            ...(isTerminal ? { isPairingPending: false, pendingPairingPrompt: null } : {}),
          });
        });

        const unlistenPrompt = await listen<RemotePairingRequestedPayload>(
          "remote_pairing_requested",
          (event) => {
            // Only one remote surface may be visible at a time: an incoming
            // authorization request takes over from the QR pairing modal.
            set({ pendingPairingPrompt: event.payload, isPairingModalOpen: false });
          }
        );

        const unlistenPairingState = await listen<PairingStateDto>(
          "remote_pairing_state_changed",
          (event) => {
            const dto = event.payload;
            if (dto.state === "AwaitingApproval") {
              set({ isPairingPending: true, pairingFailureReason: null });
            } else if (dto.state === "Denied") {
              set({
                isPairingPending: false,
                pendingPairingPrompt: null,
                pairingFailureReason: dto.failureReason ?? "Pairing failed.",
              });
            } else if (dto.state === "Approved" || dto.state === "Idle") {
              set({
                isPairingPending: false,
                pendingPairingPrompt: null,
                pairingFailureReason: null,
              });
            }
          }
        );

        teardownListeners = () => {
          unlistenState();
          unlistenPrompt();
          unlistenPairingState();
          teardownListeners = null;
        };
      })().catch((err) => {
        activeInitPromise = null;
        throw err;
      });
    }

    await activeInitPromise;

    let cleanedUp = false;
    return () => {
      if (cleanedUp) return;
      cleanedUp = true;
      subscriptionRefCount = Math.max(0, subscriptionRefCount - 1);
      if (subscriptionRefCount === 0) {
        activeInitPromise = null;
        teardownListeners?.();
      }
    };
  },

  hydrate: async () => {
    try {
      set({ loading: true, error: null });
      const [stateDto, devices, routing] = await Promise.all([
        getLinkState().catch((e) => {
          console.debug("[remoteStore] getLinkState not available:", e);
          // Ticket 05: fallback to Unpaired (no "Disconnected")
          return { link: "Unpaired" } as RemoteStateDto;
        }),
        apiListPairedDevices().catch((e) => {
          console.debug("[remoteStore] listPairedDevices error:", e);
          return [] as PairedDeviceDto[];
        }),
        getCommandRouting().catch((e) => {
          console.debug("[remoteStore] getCommandRouting error:", e);
          return {} as Record<string, CommandClass>;
        }),
      ]);

      const peer =
        stateDto.deviceId && stateDto.displayName
          ? { deviceId: stateDto.deviceId, displayName: stateDto.displayName }
          : null;

      set({
        linkState: stateDto.link,
        failureReason: stateDto.failure ?? null,
        failureKind: stateDto.failureKind ?? null,
        connectedPeer: peer,
        pairedDevices: devices,
        commandRouting: routing,
        loading: false,
        error: null,
      });
    } catch (e) {
      console.warn("[remoteStore] Failed to hydrate remote store:", e);
      set({ loading: false, error: String(e) });
    }
  },

  pairNewDevice: async () => {
    return apiPairNewDevice();
  },

  // Ticket 05: pairDevice action — Tier 1 only, no session start
  pairDevice: async (params: PairDeviceParams) => {
    await apiPairDevice(params);
  },

  openPairingModal: () => {
    // A pending authorization prompt owns the screen until it is resolved.
    if (get().pendingPairingPrompt) return;
    set({ isPairingModalOpen: true });
  },

  closePairingModal: () => {
    set({ isPairingModalOpen: false, pendingPairingPrompt: null });
  },

  approvePairing: async (decision: "Approve" | "Reject") => {
    try {
      await apiApprovePairing(decision);
    } finally {
      set({ pendingPairingPrompt: null, isPairingModalOpen: false });
      await get().listPairedDevices().catch(() => {});
    }
  },

  dismissPairingPrompt: () => {
    set({ pendingPairingPrompt: null, isPairingModalOpen: false });
  },

  dismissPairingFailure: () => {
    if (get().pairingFailureReason === null) return;
    set({ pairingFailureReason: null });
  },

  dismissFailure: () => {
    set((state) => ({
      linkState: state.connectedPeer ? "Paired" : "Unpaired",
      failureReason: null,
      failureKind: null,
    }));
  },

  listPairedDevices: async () => {
    try {
      const devices = await apiListPairedDevices();
      set({ pairedDevices: devices });
      return devices;
    } catch (e) {
      console.warn("[remoteStore] Failed to list paired devices:", e);
      return [];
    }
  },

  revokeDevice: async (deviceId: string) => {
    await apiRevokeDevice(deviceId);
    await get().listPairedDevices().catch(() => {});
  },

  disconnectDevice: async (deviceId: string) => {
    await apiDisconnectDevice(deviceId);
  },

  // Ticket 05: connectHost accepts { deviceId, address? } only (Tier 2)
  connectHost: async (params: ConnectHostParams) => {
    await apiConnectHost(params);
  },

  disconnectHost: async () => {
    await apiDisconnectHost();
  },
}));
