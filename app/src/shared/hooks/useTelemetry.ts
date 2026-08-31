import { useEffect, useRef } from 'react';
import { onTelemetry, type TelemetryData } from '@/services/eventsService';

export type { TelemetryData };

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

    try {
      if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
        unlisten = onTelemetry((payload) => {
          if (isMounted) {
            telemetryRef.current = payload;
          }
        });
      }
    } catch (err) {
      if (isMounted) {
        console.error('[Telemetry] Failed to setup listener:', err);
      }
    }

    return () => {
      isMounted = false;
      if (unlisten) unlisten();
    };
  }, []);

  return telemetryRef;
};
