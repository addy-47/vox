import { createContext } from "react";
import type { NavigateOptions } from "react-router-dom";

export interface RouteNavigationContextValue {
  navigate: (to: string, onReady?: () => void, options?: NavigateOptions) => void;
  pageReady: (pathname: string) => void;
}

export const RouteNavigationContext = createContext<RouteNavigationContextValue | null>(null);
