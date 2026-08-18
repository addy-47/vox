import React, { createContext, useContext, useState, useCallback, useRef, ReactNode } from "react";

export interface ComponentTraceData {
  componentName: string;
  mountCount: number;
  activeInstances: number;
  firstMountedAt: number;
  lastMountedAt: number;
}

export interface MemoryProfilerContextValue {
  isProfilerActive: boolean;
  setIsProfilerActive: (active: boolean) => void;
  componentTraces: Record<string, ComponentTraceData>;
  registerMount: (componentName: string) => void;
  registerUnmount: (componentName: string) => void;
  resetTraces: () => void;
}

const MemoryProfilerContext = createContext<MemoryProfilerContextValue | null>(null);

export const MemoryProfilerProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [isProfilerActive, setIsProfilerActive] = useState<boolean>(true);
  const [componentTraces, setComponentTraces] = useState<Record<string, ComponentTraceData>>({});
  const tracesRef = useRef<Record<string, ComponentTraceData>>({});

  const registerMount = useCallback((componentName: string) => {
    const now = performance.now();
    const current = tracesRef.current[componentName] || {
      componentName,
      mountCount: 0,
      activeInstances: 0,
      firstMountedAt: now,
      lastMountedAt: now,
    };

    const updated: ComponentTraceData = {
      ...current,
      mountCount: current.mountCount + 1,
      activeInstances: current.activeInstances + 1,
      lastMountedAt: now,
    };

    tracesRef.current = {
      ...tracesRef.current,
      [componentName]: updated,
    };
    setComponentTraces(tracesRef.current);
  }, []);

  const registerUnmount = useCallback((componentName: string) => {
    const current = tracesRef.current[componentName];
    if (!current) return;

    const updated: ComponentTraceData = {
      ...current,
      activeInstances: Math.max(0, current.activeInstances - 1),
    };

    tracesRef.current = {
      ...tracesRef.current,
      [componentName]: updated,
    };
    setComponentTraces(tracesRef.current);
  }, []);

  const resetTraces = useCallback(() => {
    tracesRef.current = {};
    setComponentTraces({});
  }, []);

  return (
    <MemoryProfilerContext.Provider
      value={{
        isProfilerActive,
        setIsProfilerActive,
        componentTraces,
        registerMount,
        registerUnmount,
        resetTraces,
      }}
    >
      {children}
    </MemoryProfilerContext.Provider>
  );
};

export function useMemoryProfilerContext(): MemoryProfilerContextValue {
  const ctx = useContext(MemoryProfilerContext);
  if (!ctx) {
    // Return safe fallback if not wrapped in provider
    return {
      isProfilerActive: false,
      setIsProfilerActive: () => {},
      componentTraces: {},
      registerMount: () => {},
      registerUnmount: () => {},
      resetTraces: () => {},
    };
  }
  return ctx;
}
