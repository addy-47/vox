import { useEffect, useRef } from 'react';

export interface TelemetryData {
  energy: number;
  vad_prob: number;
  low: number;
  mid: number;
  high: number;
}

/**
 * useTelemetry provides access to high-frequency audio telemetry data
 * without triggering React re-renders. It updates a ref that can be
 * consumed by requestAnimationFrame loops.
 */
export const useTelemetry = () => {
  const telemetryRef = useRef<TelemetryData>({ energy: 0, vad_prob: 0, low: 0, mid: 0, high: 0 });

  useEffect(() => {
    let isMounted = true;
    let unlisten: (() => void) | null = null;

    const setup = async () => {
      try {
        if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
          const { getCurrentWindow } = await import('@tauri-apps/api/window');
          const appWindow = getCurrentWindow();
          const unlistenFn = await appWindow.listen<TelemetryData>('telemetry', (event) => {
            if (isMounted) {
              telemetryRef.current = event.payload;
            }
          });
          if (isMounted) {
            unlisten = unlistenFn;
          } else {
            unlistenFn();
          }
        }
      } catch (err) {
        console.error('[Telemetry] Failed to setup listener:', err);
      }
    };

    setup();

    return () => {
      isMounted = false;
      if (unlisten) unlisten();
    };
  }, []);

  return telemetryRef;
};
