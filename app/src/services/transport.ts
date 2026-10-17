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
 * Invoke an IPC command with a hard timeout. The default matches the pipeline
 * orchestration poll budget used by the session lifecycle services.
 * Note: Tauri IPC cannot be aborted mid-flight — the timeout abandons the
 * pending invocation rather than cancelling it.
 */
export function invokeWithTimeout<T>(
  cmd: string,
  args?: Record<string, unknown>,
  timeoutMs: number = 8000,
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`${cmd} timed out after ${timeoutMs}ms`));
    }, timeoutMs);
    activeTransport
      .invoke<T>(cmd, args)
      .then((value) => {
        clearTimeout(timer);
        resolve(value);
      })
      .catch((err: unknown) => {
        clearTimeout(timer);
        reject(err);
      });
  });
}

export interface InvokeRetryOptions {
  /** Additional attempts after the first failure. @default 2 */
  retries?: number;
  /** Delay (ms) before each retry, indexed by retry number; the last value repeats. @default [250, 500] */
  backoffMs?: number[];
}

/**
 * Invoke an IPC command with bounded retries and backoff. Delays are indexed by
 * retry number (first retry waits `backoffMs[0]`); a shorter array clamps to
 * its last value for all remaining retries. Rejects with the final error.
 */
export async function invokeWithRetry<T>(
  cmd: string,
  args?: Record<string, unknown>,
  options?: InvokeRetryOptions,
): Promise<T> {
  const retries = options?.retries ?? 2;
  const backoffMs = options?.backoffMs ?? [250, 500];
  let lastErr: unknown;
  for (let attempt = 0; attempt <= retries; attempt++) {
    if (attempt > 0) {
      const delay = backoffMs[Math.min(attempt - 1, backoffMs.length - 1)] ?? 0;
      await new Promise((r) => setTimeout(r, delay));
    }
    try {
      return await activeTransport.invoke<T>(cmd, args);
    } catch (err: unknown) {
      lastErr = err;
    }
  }
  throw lastErr;
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
