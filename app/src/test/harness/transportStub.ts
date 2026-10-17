import type { Transport, UnlistenFn } from "@/services/transport";

/**
 * Recording Transport stub shared by store/service tests.
 *
 * Valid helper per /create-test Phase 1: it is replaceable by a direct
 * production call (setTransport(s => ...) accepts any Transport), changes no
 * production code, and implements only the Transport interface. It records
 * every invoke so tests assert on real arg shapes, not on their own mocks.
 */

export interface InvokeRecord {
  cmd: string;
  args?: Record<string, unknown>;
}

export interface TransportStubOptions {
  /** Maps cmd → resolved value. Unlisted cmds resolve to null. */
  responses?: Record<string, unknown>;
  /** Maps cmd → rejection. Takes priority over `responses`. */
  rejections?: Record<string, Error>;
  /** Capture point for every invoke call. */
  calls?: InvokeRecord[];
  /** Capture point for every listen registration. */
  listened?: string[];
}

export function makeTransportStub(options: TransportStubOptions = {}): Transport {
  const calls = options.calls ?? [];
  const listened = options.listened ?? [];

  return {
    invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
      calls.push({ cmd, args });
      const rejection = options.rejections?.[cmd];
      if (rejection) {
        return Promise.reject(rejection) as unknown as Promise<T>;
      }
      const res = options.responses?.[cmd];
      return Promise.resolve(
        (res === undefined ? null : res) as unknown as T
      );
    },
    listen<T>(event: string, _handler: (event: { payload: T }) => void): Promise<UnlistenFn> {
      listened.push(event);
      return Promise.resolve(() => {});
    },
    convertFileSrc(filePath: string, protocol?: string): string {
      return `stub://${protocol ?? "asset"}/${filePath}`;
    },
  };
}

