import React, { createContext, useContext, useRef, useCallback, memo } from "react";

export interface DrawerHandlers {
  open: () => void;
  close: () => void;
}

interface PageDrawerContextValue {
  registerPageDrawer: (handlers: DrawerHandlers) => () => void;
  openActiveDrawer: () => void;
  closeActiveDrawer: () => void;
}

const PageDrawerContext = createContext<PageDrawerContextValue | null>(null);

export const PageDrawerProvider: React.FC<{ children: React.ReactNode }> = memo(({ children }) => {
  const currentHandlersRef = useRef<DrawerHandlers | null>(null);

  const registerPageDrawer = useCallback((handlers: DrawerHandlers) => {
    currentHandlersRef.current = handlers;
    return () => {
      if (currentHandlersRef.current === handlers) {
        currentHandlersRef.current = null;
      }
    };
  }, []);

  const openActiveDrawer = useCallback(() => {
    currentHandlersRef.current?.open();
  }, []);

  const closeActiveDrawer = useCallback(() => {
    currentHandlersRef.current?.close();
  }, []);

  const value = React.useMemo(
    () => ({
      registerPageDrawer,
      openActiveDrawer,
      closeActiveDrawer,
    }),
    [registerPageDrawer, openActiveDrawer, closeActiveDrawer]
  );

  return (
    <PageDrawerContext.Provider value={value}>
      {children}
    </PageDrawerContext.Provider>
  );
});

PageDrawerProvider.displayName = "PageDrawerProvider";

export function usePageDrawer(): PageDrawerContextValue {
  const ctx = useContext(PageDrawerContext);
  if (!ctx) {
    throw new Error("usePageDrawer must be used within PageDrawerProvider");
  }
  return ctx;
}

export function useRegisterPageDrawer(handlers: DrawerHandlers) {
  const { registerPageDrawer } = usePageDrawer();
  React.useEffect(() => {
    return registerPageDrawer(handlers);
  }, [registerPageDrawer, handlers]);
}
