import { invoke as tauriInvoke, convertFileSrc as tauriConvertFileSrc } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";

export type { UnlistenFn };

/**
 * Common Transport interface for dispatching IPC commands and listening for backend events.
 * Decouples frontend services from the direct Tauri runtime so alternative carriers
 * (WebSocket for remote control, mock transports for testing) can be plugged in seamlessly.
 */
export interface Transport {
  invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<UnlistenFn>;
  convertFileSrc(filePath: string, protocol?: string): string;
}

/**
 * Standard Tauri IPC carrier delegating to `@tauri-apps/api/core` and `@tauri-apps/api/event`.
 */
export class TauriTransport implements Transport {
  invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    return tauriInvoke<T>(cmd, args);
  }

  listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<UnlistenFn> {
    return tauriListen<T>(event, handler);
  }

  convertFileSrc(filePath: string, protocol?: string): string {
    return tauriConvertFileSrc(filePath, protocol);
  }
}

let activeTransport: Transport = new TauriTransport();

/** Retrieve the current active transport carrier. */
export function getTransport(): Transport {
  return activeTransport;
}

/** Swap the active transport carrier (e.g. for remote control or test mocks). */
export function setTransport(transport: Transport): void {
  activeTransport = transport;
}

/**
 * Invoke an IPC command via the active transport carrier.
 */
export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return activeTransport.invoke<T>(cmd, args);
}

/**
 * Listen for an event via the active transport carrier.
 */
export function listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<UnlistenFn> {
  return activeTransport.listen<T>(event, handler);
}

/**
 * Convert a file path into an asset protocol URL via the active transport carrier.
 */
export function convertFileSrc(filePath: string, protocol?: string): string {
  return activeTransport.convertFileSrc(filePath, protocol);
}
