import React, { createContext, memo, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { HelpDrawer } from "./HelpDrawer";

interface HelpDrawerContextValue {
  openHelp: (deepLink?: string | null) => void;
  closeHelp: () => void;
  isOpen: boolean;
  deepLink: string | null;
}

const HelpDrawerContext = createContext<HelpDrawerContextValue | null>(null);

export function useHelp(): HelpDrawerContextValue {
  const ctx = useContext(HelpDrawerContext);
  if (!ctx) throw new Error("useHelp must be used within HelpDrawerProvider");
  return ctx;
}

interface HelpDrawerProviderProps {
  children: React.ReactNode;
}

const HelpDrawerProviderInner = memo(({ children }: HelpDrawerProviderProps) => {
  const [isOpen, setIsOpen] = useState(false);
  const [deepLink, setDeepLink] = useState<string | null>(null);

  const openHelp = useCallback((link?: string | null) => {
    setDeepLink(link ?? null);
    setIsOpen(true);
  }, []);

  const closeHelp = useCallback(() => {
    setIsOpen(false);
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "?" || !e.shiftKey) return;
      const target = e.target as HTMLElement | null;
      if (target) {
        const tag = target.tagName;
        if (tag === "INPUT" || tag === "TEXTAREA" || target.isContentEditable) return;
      }
      e.preventDefault();
      setIsOpen((prev) => !prev);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [isOpen]);

  const value = useMemo<HelpDrawerContextValue>(
    () => ({ openHelp, closeHelp, isOpen, deepLink }),
    [openHelp, closeHelp, isOpen, deepLink]
  );

  return (
    <HelpDrawerContext.Provider value={value}>
      {children}
      <HelpDrawer open={isOpen} onClose={closeHelp} deepLink={deepLink} />
    </HelpDrawerContext.Provider>
  );
});
HelpDrawerProviderInner.displayName = "HelpDrawerProviderInner";

export const HelpDrawerProvider = HelpDrawerProviderInner;
