import React, { useEffect, ReactNode } from "react";
import { useMemoryProfilerContext } from "@/shared/context/MemoryProfilerContext";

/**
 * Lightweight hook to trace component mount/unmount lifecycles for memory profiling.
 * Low overhead: only active when profiler context is active.
 */
export function useMemoryTrace(componentName: string): void {
  const { registerMount, registerUnmount, isProfilerActive } = useMemoryProfilerContext();

  useEffect(() => {
    if (!isProfilerActive) return;
    registerMount(componentName);

    return () => {
      registerUnmount(componentName);
    };
  }, [componentName, registerMount, registerUnmount, isProfilerActive]);
}

export interface MemoryTrackedProps {
  name: string;
  children: ReactNode;
}

/**
 * Declarative wrapper component for memory attribution tracing.
 */
export const MemoryTracked: React.FC<MemoryTrackedProps> = ({ name, children }) => {
  useMemoryTrace(name);
  return <>{children}</>;
};
