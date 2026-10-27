import { invoke } from "./transport";

// ─── Ticket 01: Updated type union (Disconnected removed) ───────────────────
export type RemoteLinkState =
  | "Disabled"
  | "Unpaired"
  | "Paired"
  | "Connecting"
  | "Connected"
  | "Reconnecting"
  | "Failed";

export type RemoteFailureKind =
  | "timeout"
  | "unreachable"
  | "rejected_by_host"
  | "invalid_token"
  | "version_mismatch"
  | "unknown";

export type CommandClass = "Remote" | "Local" | "Split" | "Denied";

export interface RemoteStateDto {
  link: RemoteLinkState;
  failure?: string;
  failureKind?: RemoteFailureKind;
  deviceId?: string;
  displayName?: string;
}

// ─── Ticket 01: Ephemeral pairing state ──────────────────────────────────────
export interface PairingStateDto {
  state: "Idle" | "AwaitingApproval" | "Approved" | "Denied";
  deviceId?: string;
  displayName?: string;
  /** Unix millisecond timestamp of the approval deadline (AwaitingApproval only). */
  expiresAtMs?: number;
  failureReason?: string;
}

export interface PairNewDeviceDto {
  payload: string;
  qrDataUrl: string;
  expiresAt: number;
  expiresAtMs: number;
}

export interface PairedDeviceDto {
  deviceId: string;
  displayName: string;
  role: string;
  pairedAt: number;
  lastSeenAt?: number;
  hints: unknown[];
}

export interface RemotePairingRequestedPayload {
  deviceId: string;
  displayName: string;
}

// ─── Host-side Tier 1 ────────────────────────────────────────────────────────

export function pairNewDevice(): Promise<PairNewDeviceDto> {
  return invoke<PairNewDeviceDto>("pair_new_device");
}

export function approvePairing(decision: "Approve" | "Reject"): Promise<void> {
  return invoke<void>("approve_pairing", { decision });
}

// ─── Ticket 02: Controller-side Tier 1 ───────────────────────────────────────

export type PairDeviceParams = {
  address: string;
  token: string;
  publicKey: string;
  deviceId: string;
};

/**
 * Controller performs Tier 1 pairing handshake.
 * On success the device is written to paired_devices and linkState → "Paired".
 * Does NOT start a Tier 2 session. Call connectHost() for that.
 */
export function pairDevice(params: PairDeviceParams): Promise<void> {
  return invoke<void>("pair_device", params);
}

// ─── Tier 2 — transport session ──────────────────────────────────────────────

export type ConnectHostParams = {
  /** ID of a device already in paired_devices. */
  deviceId: string;
  /** Optional address override; falls back to paired_devices hints if absent. */
  address?: string;
};

/**
 * Controller starts a Tier 2 transport session with an already-paired host.
 * Spec §4.4: device must already be in paired_devices (run pairDevice first).
 */
export function connectHost(params: ConnectHostParams): Promise<void> {
  return invoke<void>("connect_host", params);
}

export function disconnectHost(): Promise<void> {
  return invoke<void>("disconnect_host");
}

// ─── Tier 1 management ───────────────────────────────────────────────────────

export function listPairedDevices(): Promise<PairedDeviceDto[]> {
  return invoke<PairedDeviceDto[]>("list_paired_devices");
}

export function revokeDevice(deviceId: string): Promise<void> {
  return invoke<void>("revoke_device", { deviceId });
}

export function disconnectDevice(deviceId: string): Promise<void> {
  return invoke<void>("disconnect_device", { deviceId });
}

// ─── Queries ─────────────────────────────────────────────────────────────────

export function getLinkState(): Promise<RemoteStateDto> {
  return invoke<RemoteStateDto>("get_link_state");
}

export function getCommandRouting(): Promise<Record<string, CommandClass>> {
  return invoke<Record<string, CommandClass>>("get_command_routing");
}
