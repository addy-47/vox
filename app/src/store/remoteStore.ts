import { create } from "zustand";
import { listen, type UnlistenFn } from "@/services/transport";
import {
  type RemoteLinkState,
  type RemoteFailureKind,
  type RemoteStateDto,
  type PairNewDeviceDto,
  type PairedDeviceDto,
  type RemotePairingRequestedPayload,
  type CommandClass,
  type ConnectHostParams,
  pairNewDevice as apiPairNewDevice,
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
  isPairingPending: boolean;
  pairedDevices: PairedDeviceDto[];
  commandRouting: Record<string, CommandClass>;
  loading: boolean;
  error: string | null;

  init: () => Promise<UnlistenFn>;
  hydrate: () => Promise<void>;
  pairNewDevice: () => Promise<PairNewDeviceDto>;
  approvePairing: (decision: "Approve" | "Reject") => Promise<void>;
  dismissPairingPrompt: () => void;
  listPairedDevices: () => Promise<PairedDeviceDto[]>;
  revokeDevice: (deviceId: string) => Promise<void>;
  disconnectDevice: (deviceId: string) => Promise<void>;
  connectHost: (params: ConnectHostParams) => Promise<void>;
  disconnectHost: () => Promise<void>;
}

export const useRemoteStore = create<RemoteStoreState>((set, get) => ({
  linkState: "Disconnected",
  failureReason: null,
  failureKind: null,
  connectedPeer: null,
  pendingPairingPrompt: null,
  isPairingPending: false,
  pairedDevices: [],
  commandRouting: {},
  loading: false,
  error: null,

  init: async () => {
    await get().hydrate();

    const unlistenState = await listen<RemoteStateDto>("remote_state_changed", (event) => {
      const dto = event.payload;
      const peer =
        dto.deviceId && dto.displayName
          ? { deviceId: dto.deviceId, displayName: dto.displayName }
          : null;

      const isTerminal =
        dto.link === "Connected" || dto.link === "Failed" || dto.link === "Disconnected";

      set({
        linkState: dto.link,
        failureReason: dto.failure ?? null,
        failureKind: dto.failureKind ?? null,
        connectedPeer: peer,
        ...(isTerminal ? { isPairingPending: false } : {}),
      });

      if (dto.link === "Connected") {
        // Only refresh the paired-devices list when the peer isn't already
        // known locally — avoids an extra IPC round-trip on the close path.
        const known = get().pairedDevices.some((d) => d.deviceId === dto.deviceId);
        if (!known) {
          get().listPairedDevices().catch((e) => {
            console.debug("[remoteStore] Failed to refresh paired devices:", e);
          });
        }
      }
    });

    const unlistenPrompt = await listen<RemotePairingRequestedPayload>(
      "remote_pairing_requested",
      (event) => {
        set({ pendingPairingPrompt: event.payload });
      }
    );

    const unlistenPending = await listen("remote_pairing_pending", () => {
      set({ isPairingPending: true });
    });

    return () => {
      unlistenState();
      unlistenPrompt();
      unlistenPending();
    };
  },

  hydrate: async () => {
    try {
      set({ loading: true, error: null });
      const [stateDto, devices, routing] = await Promise.all([
        getLinkState().catch((e) => {
          console.debug("[remoteStore] getLinkState not available:", e);
          return { link: "Disconnected" } as RemoteStateDto;
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

  approvePairing: async (decision: "Approve" | "Reject") => {
    await apiApprovePairing(decision);
    set({ pendingPairingPrompt: null });
    await get().listPairedDevices();
  },

  dismissPairingPrompt: () => {
    set({ pendingPairingPrompt: null });
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
    await get().listPairedDevices();
  },

  disconnectDevice: async (deviceId: string) => {
    await apiDisconnectDevice(deviceId);
  },

  connectHost: async (params: ConnectHostParams) => {
    await apiConnectHost(params);
  },

  disconnectHost: async () => {
    await apiDisconnectHost();
  },
}));
