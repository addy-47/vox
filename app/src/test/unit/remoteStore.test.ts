import { describe, it, expect, beforeEach } from "vitest";
import { useRemoteStore } from "@/store/remoteStore";
import { setTransport, type Transport, type UnlistenFn } from "@/services/transport";
import type {
  RemoteStateDto,
  PairedDeviceDto,
  RemotePairingRequestedPayload,
  CommandClass,
} from "@/services/remoteService";

interface MockTransport extends Transport {
  invokes: Array<{ cmd: string; args?: Record<string, unknown> }>;
  listeners: Map<string, (event: { payload: unknown }) => void>;
  unlistenCalls: number;
  triggerEvent: <T>(event: string, payload: T) => void;
  mockResponses: Record<string, unknown>;
}

function createMockTransport(): MockTransport {
  const invokes: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
  const listeners = new Map<string, (event: { payload: unknown }) => void>();
  const mockResponses: Record<string, unknown> = {};
  let unlistenCalls = 0;

  return {
    invokes,
    listeners,
    unlistenCalls,
    mockResponses,
    triggerEvent<T>(event: string, payload: T) {
      const handler = listeners.get(event);
      if (handler) {
        handler({ payload });
      }
    },
    invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
      invokes.push({ cmd, args });
      if (cmd in mockResponses) {
        const resp = mockResponses[cmd];
        if (resp instanceof Error) {
          return Promise.reject(resp);
        }
        return Promise.resolve(resp as T);
      }
      return Promise.resolve(null as unknown as T);
    },
    listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<UnlistenFn> {
      listeners.set(event, handler as (event: { payload: unknown }) => void);
      return Promise.resolve(() => {
        unlistenCalls += 1;
        listeners.delete(event);
      });
    },
    convertFileSrc(filePath: string, protocol?: string): string {
      return `stub://${protocol ?? "asset"}/${filePath}`;
    },
  };
}

describe("remoteStore unit suite", () => {
  let transport: MockTransport;

  beforeEach(() => {
    transport = createMockTransport();
    setTransport(transport);

    // Reset Zustand store state to defaults
    useRemoteStore.setState({
      linkState: "Unpaired",
      failureReason: null,
      connectedPeer: null,
      pendingPairingPrompt: null,
      pairingFailureReason: null,
      isPairingModalOpen: false,
      pairedDevices: [],
      commandRouting: {},
      loading: false,
      error: null,
    });
  });

  it("hydrates initial link state, paired devices, and command routing from backend", async () => {
    const mockState: RemoteStateDto = {
      link: "Connecting",
      deviceId: "peer_host_1",
      displayName: "Laptop",
    };
    const mockDevices: PairedDeviceDto[] = [
      {
        deviceId: "ctrl_1",
        displayName: "Pixel 9",
        role: "controller",
        pairedAt: 1700000000,
        hints: [],
      },
    ];
    const mockRouting: Record<string, CommandClass> = {
      start_session: "Remote",
      appearance: "Local",
      get_settings: "Split",
    };

    transport.mockResponses["get_link_state"] = mockState;
    transport.mockResponses["list_paired_devices"] = mockDevices;
    transport.mockResponses["get_command_routing"] = mockRouting;

    await useRemoteStore.getState().hydrate();

    const state = useRemoteStore.getState();
    expect(state.linkState).toBe("Connecting");
    expect(state.connectedPeer).toEqual({ deviceId: "peer_host_1", displayName: "Laptop" });
    expect(state.pairedDevices).toEqual(mockDevices);
    expect(state.commandRouting).toEqual(mockRouting);
    expect(state.loading).toBe(false);
    expect(state.error).toBeNull();
  });

  it("subscribes to remote_state_changed and updates store accordingly", async () => {
    transport.mockResponses["get_link_state"] = { link: "Unpaired" } as RemoteStateDto;
    transport.mockResponses["list_paired_devices"] = [] as PairedDeviceDto[];
    transport.mockResponses["get_command_routing"] = {};

    const cleanup = await useRemoteStore.getState().init();

    // Trigger state change to Connected
    transport.triggerEvent<RemoteStateDto>("remote_state_changed", {
      link: "Connected",
      deviceId: "phone_xyz",
      displayName: "Pixel 9 Pro",
    });

    let state = useRemoteStore.getState();
    expect(state.linkState).toBe("Connected");
    expect(state.connectedPeer).toEqual({ deviceId: "phone_xyz", displayName: "Pixel 9 Pro" });
    expect(state.failureReason).toBeNull();

    // Trigger state change to Failed with reason
    transport.triggerEvent<RemoteStateDto>("remote_state_changed", {
      link: "Failed",
      failure: "HandshakeTimeout",
    });

    state = useRemoteStore.getState();
    expect(state.linkState).toBe("Failed");
    expect(state.failureReason).toBe("HandshakeTimeout");
    expect(state.connectedPeer).toBeNull();

    // Cleanup listeners
    cleanup();
    expect(transport.listeners.has("remote_state_changed")).toBe(false);
  });

  it("subscribes to remote_pairing_requested and sets pendingPairingPrompt", async () => {
    transport.mockResponses["get_link_state"] = { link: "Unpaired" } as RemoteStateDto;
    transport.mockResponses["list_paired_devices"] = [] as PairedDeviceDto[];
    transport.mockResponses["get_command_routing"] = {};

    const cleanup = await useRemoteStore.getState().init();

    const payload: RemotePairingRequestedPayload = {
      deviceId: "req_device_1",
      displayName: "Ada's Phone",
    };
    // QR pairing modal is open when the phone requests authorization.
    useRemoteStore.setState({ isPairingModalOpen: true });

    transport.triggerEvent<RemotePairingRequestedPayload>("remote_pairing_requested", payload);

    expect(useRemoteStore.getState().pendingPairingPrompt).toEqual(payload);
    // Only one remote surface may be visible: the QR modal must close.
    expect(useRemoteStore.getState().isPairingModalOpen).toBe(false);

    useRemoteStore.getState().dismissPairingPrompt();
    expect(useRemoteStore.getState().pendingPairingPrompt).toBeNull();

    cleanup();
  });

  it("survives React StrictMode double-mount lifecycle without losing listeners", async () => {
    transport.mockResponses["get_link_state"] = { link: "Unpaired" } as RemoteStateDto;
    transport.mockResponses["list_paired_devices"] = [] as PairedDeviceDto[];
    transport.mockResponses["get_command_routing"] = {};

    // Simulate Mount 1
    let isMounted1 = true;
    let cleanup1: (() => void) | null = null;
    const p1 = useRemoteStore.getState().init();
    p1.then((u) => {
      if (isMounted1) cleanup1 = u;
      else u();
    });

    // Simulate StrictMode unmount 1 before p1 resolves
    isMounted1 = false;
    if (cleanup1) (cleanup1 as () => void)();

    // Simulate Remount 2
    let isMounted2 = true;
    let cleanup2: (() => void) | null = null;
    const p2 = useRemoteStore.getState().init();
    await p2.then((u) => {
      if (isMounted2) cleanup2 = u;
      else u();
    });

    const payload: RemotePairingRequestedPayload = {
      deviceId: "strict_dev_1",
      displayName: "Strict Phone",
    };
    transport.triggerEvent<RemotePairingRequestedPayload>("remote_pairing_requested", payload);

    expect(useRemoteStore.getState().pendingPairingPrompt).toEqual(payload);
    if (cleanup2) (cleanup2 as () => void)();
  });

  it("does not open the QR pairing modal while an authorization prompt is pending", async () => {
    useRemoteStore.setState({
      pendingPairingPrompt: { deviceId: "req_device_2", displayName: "Ada's Phone" },
    });

    useRemoteStore.getState().openPairingModal();

    expect(useRemoteStore.getState().isPairingModalOpen).toBe(false);
  });

  it("records a pairing denial without corrupting linkState, and dismisses it", async () => {
    transport.mockResponses["get_link_state"] = { link: "Unpaired" } as RemoteStateDto;
    transport.mockResponses["list_paired_devices"] = [] as PairedDeviceDto[];
    transport.mockResponses["get_command_routing"] = {};

    const cleanup = await useRemoteStore.getState().init();

    transport.triggerEvent("remote_pairing_state_changed", {
      state: "Denied",
      failureReason: "Pairing denied by host: User denied pairing",
    });

    let state = useRemoteStore.getState();
    expect(state.pairingFailureReason).toBe(
      "Pairing denied by host: User denied pairing"
    );
    expect(state.isPairingPending).toBe(false);
    // Spec §5.5: pairing failure must not corrupt the transport state.
    expect(state.linkState).toBe("Unpaired");

    useRemoteStore.getState().dismissPairingFailure();
    state = useRemoteStore.getState();
    expect(state.pairingFailureReason).toBeNull();

    cleanup();
  });

  it("dismissFailure resets linkState to Paired when connectedPeer is retained", () => {
    useRemoteStore.setState({
      linkState: "Failed",
      failureReason: "Connection timed out",
      failureKind: "timeout",
      connectedPeer: { deviceId: "host_1", displayName: "Laptop Host" },
    });

    useRemoteStore.getState().dismissFailure();

    const state = useRemoteStore.getState();
    expect(state.linkState).toBe("Paired");
    expect(state.failureReason).toBeNull();
    expect(state.failureKind).toBeNull();
    expect(state.connectedPeer).toEqual({ deviceId: "host_1", displayName: "Laptop Host" });
  });

  it("clears the pairing failure on a fresh approval request", async () => {
    useRemoteStore.setState({ pairingFailureReason: "Pairing denied by host" });

    transport.mockResponses["get_link_state"] = { link: "Unpaired" } as RemoteStateDto;
    transport.mockResponses["list_paired_devices"] = [] as PairedDeviceDto[];
    transport.mockResponses["get_command_routing"] = {};

    const cleanup = await useRemoteStore.getState().init();

    transport.triggerEvent("remote_pairing_state_changed", {
      state: "AwaitingApproval",
    });

    expect(useRemoteStore.getState().pairingFailureReason).toBeNull();

    cleanup();
  });

  it("approves pairing and refreshes device list", async () => {
    transport.mockResponses["approve_pairing"] = null;
    transport.mockResponses["list_paired_devices"] = [
      {
        deviceId: "approved_dev",
        displayName: "Ada's Phone",
        role: "controller",
        pairedAt: 1700000000,
        hints: [],
      },
    ];

    useRemoteStore.setState({
      pendingPairingPrompt: { deviceId: "approved_dev", displayName: "Ada's Phone" },
    });

    await useRemoteStore.getState().approvePairing("Approve");

    expect(transport.invokes).toContainEqual({
      cmd: "approve_pairing",
      args: { decision: "Approve" },
    });
    expect(useRemoteStore.getState().pendingPairingPrompt).toBeNull();
    expect(useRemoteStore.getState().pairedDevices).toHaveLength(1);
    expect(useRemoteStore.getState().pairedDevices[0]?.deviceId).toBe("approved_dev");
  });

  it("invokes pairDevice, revokeDevice, disconnectDevice, connectHost, and disconnectHost", async () => {
    transport.mockResponses["pair_device"] = null;
    transport.mockResponses["revoke_device"] = null;
    transport.mockResponses["disconnect_device"] = null;
    transport.mockResponses["connect_host"] = null;
    transport.mockResponses["disconnect_host"] = null;
    transport.mockResponses["list_paired_devices"] = [];

    await useRemoteStore.getState().pairDevice({
      address: "192.168.1.100:7700",
      token: "tok123",
      publicKey: "pub456",
      deviceId: "dev789",
    });
    expect(transport.invokes).toContainEqual({
      cmd: "pair_device",
      args: {
        address: "192.168.1.100:7700",
        token: "tok123",
        publicKey: "pub456",
        deviceId: "dev789",
      },
    });

    await useRemoteStore.getState().revokeDevice("device_revoke_1");
    expect(transport.invokes).toContainEqual({
      cmd: "revoke_device",
      args: { deviceId: "device_revoke_1" },
    });

    await useRemoteStore.getState().disconnectDevice("device_disc_1");
    expect(transport.invokes).toContainEqual({
      cmd: "disconnect_device",
      args: { deviceId: "device_disc_1" },
    });

    await useRemoteStore.getState().connectHost({ deviceId: "peer_1", address: "192.168.1.100:7700" });
    expect(transport.invokes).toContainEqual({
      cmd: "connect_host",
      args: { deviceId: "peer_1", address: "192.168.1.100:7700" },
    });

    await useRemoteStore.getState().disconnectHost();
    expect(transport.invokes).toContainEqual({
      cmd: "disconnect_host",
      args: undefined,
    });
  });
});
