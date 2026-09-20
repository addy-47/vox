import { useEffect } from "react";
import { useMemoryProfilerActions } from "@/shared/context/MemoryProfilerContext";

/**
 * Lightweight hook to trace component mount/unmount lifecycles for memory profiling.
 * Low overhead: consumes stable action callbacks, preventing re-renders on trace updates.
 */
export function useMemoryTrace(componentName: string): void {
  const { registerMount, registerUnmount, isProfilerActive } = useMemoryProfilerActions();

  useEffect(() => {
    if (!isProfilerActive) return;
    registerMount(componentName);

    return () => {
      registerUnmount(componentName);
    };
  }, [componentName, registerMount, registerUnmount, isProfilerActive]);
}

