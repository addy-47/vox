import React, { createContext, useContext, useState, useCallback, useMemo, useRef, ReactNode } from "react";

export interface ComponentTraceData {
  componentName: string;
  mountCount: number;
  activeInstances: number;
  firstMountedAt: number;
  lastMountedAt: number;
}

export interface MemoryProfilerActions {
  isProfilerActive: boolean;
  setIsProfilerActive: (active: boolean) => void;
  registerMount: (componentName: string) => void;
  registerUnmount: (componentName: string) => void;
  resetTraces: () => void;
  getComponentTraces: () => Record<string, ComponentTraceData>;
}

export interface MemoryProfilerData {
  componentTraces: Record<string, ComponentTraceData>;
}

export type MemoryProfilerContextValue = MemoryProfilerActions & MemoryProfilerData;

const MemoryProfilerActionsContext = createContext<MemoryProfilerActions | null>(null);
const MemoryProfilerDataContext = createContext<MemoryProfilerData | null>(null);

export const MemoryProfilerProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [isProfilerActive, setIsProfilerActive] = useState<boolean>(true);
  const [componentTraces, setComponentTraces] = useState<Record<string, ComponentTraceData>>({});
  const tracesRef = useRef<Record<string, ComponentTraceData>>({});
  const updatePendingRef = useRef<boolean>(false);

  // Batched trace state sync to avoid synchronous re-render waves across React tree
  const scheduleSync = useCallback(() => {
    if (updatePendingRef.current) return;
    updatePendingRef.current = true;
    queueMicrotask(() => {
      updatePendingRef.current = false;
      setComponentTraces({ ...tracesRef.current });
    });
  }, []);

  const registerMount = useCallback((componentName: string) => {
    const now = performance.now();
    const current = tracesRef.current[componentName] || {
      componentName,
      mountCount: 0,
      activeInstances: 0,
      firstMountedAt: now,
      lastMountedAt: now,
    };

    tracesRef.current[componentName] = {
      ...current,
      mountCount: current.mountCount + 1,
      activeInstances: current.activeInstances + 1,
      lastMountedAt: now,
    };
    scheduleSync();
  }, [scheduleSync]);

  const registerUnmount = useCallback((componentName: string) => {
    const current = tracesRef.current[componentName];
    if (!current) return;

    tracesRef.current[componentName] = {
      ...current,
      activeInstances: Math.max(0, current.activeInstances - 1),
    };
    scheduleSync();
  }, [scheduleSync]);

  const resetTraces = useCallback(() => {
    tracesRef.current = {};
    setComponentTraces({});
  }, []);

  const getComponentTraces = useCallback(() => tracesRef.current, []);

  const actionsValue = useMemo<MemoryProfilerActions>(
    () => ({
      isProfilerActive,
      setIsProfilerActive,
      registerMount,
      registerUnmount,
      resetTraces,
      getComponentTraces,
    }),
    [isProfilerActive, registerMount, registerUnmount, resetTraces, getComponentTraces]
  );

  const dataValue = useMemo<MemoryProfilerData>(
    () => ({
      componentTraces,
    }),
    [componentTraces]
  );

  return (
    <MemoryProfilerActionsContext.Provider value={actionsValue}>
      <MemoryProfilerDataContext.Provider value={dataValue}>
        {children}
      </MemoryProfilerDataContext.Provider>
    </MemoryProfilerActionsContext.Provider>
  );
};

export function useMemoryProfilerActions(): MemoryProfilerActions {
  const ctx = useContext(MemoryProfilerActionsContext);
  if (!ctx) {
    return {
      isProfilerActive: false,
      setIsProfilerActive: () => {},
      registerMount: () => {},
      registerUnmount: () => {},
      resetTraces: () => {},
      getComponentTraces: () => ({}),
    };
  }
  return ctx;
}

export function useMemoryProfilerData(): MemoryProfilerData {
  const ctx = useContext(MemoryProfilerDataContext);
  if (!ctx) {
    return { componentTraces: {} };
  }
  return ctx;
}

export function useMemoryProfilerContext(): MemoryProfilerContextValue {
  const actions = useMemoryProfilerActions();
  const data = useMemoryProfilerData();
  return { ...actions, ...data };
}
