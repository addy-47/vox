import { invoke } from "./transport";

export type RemoteLinkState =
  | "Disabled"
  | "Disconnected"
  | "Connecting"
  | "Connected"
  | "Reconnecting"
  | "Failed";

export type CommandClass = "Remote" | "Local" | "Split" | "Denied";

export interface RemoteStateDto {
  link: RemoteLinkState;
  failure?: string;
  deviceId?: string;
  displayName?: string;
}

export interface PairNewDeviceDto {
  payload: string;
  qrDataUrl: string;
  expiresAt: number;
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

export function pairNewDevice(): Promise<PairNewDeviceDto> {
  return invoke<PairNewDeviceDto>("pair_new_device");
}

export function approvePairing(decision: "Approve" | "Reject"): Promise<void> {
  return invoke<void>("approve_pairing", { decision });
}

export function listPairedDevices(): Promise<PairedDeviceDto[]> {
  return invoke<PairedDeviceDto[]>("list_paired_devices");
}

export function revokeDevice(deviceId: string): Promise<void> {
  return invoke<void>("revoke_device", { deviceId });
}

export function disconnectDevice(deviceId: string): Promise<void> {
  return invoke<void>("disconnect_device", { deviceId });
}

export function connectHost(params: { deviceId?: string; address?: string }): Promise<void> {
  return invoke<void>("connect_host", params);
}

export function disconnectHost(): Promise<void> {
  return invoke<void>("disconnect_host");
}

export function getLinkState(): Promise<RemoteStateDto> {
  return invoke<RemoteStateDto>("get_link_state");
}

export function getCommandRouting(): Promise<Record<string, CommandClass>> {
  return invoke<Record<string, CommandClass>>("get_command_routing");
}
