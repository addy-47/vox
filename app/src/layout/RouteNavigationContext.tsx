import { useContext, useEffect, type ReactNode } from "react";
import { RouteNavigationContext, type RouteNavigationContextValue } from "./routeNavigationContext";

export function RouteNavigationProvider({
  value,
  children,
}: {
  value: RouteNavigationContextValue;
  children: ReactNode;
}) {
  return (
    <RouteNavigationContext.Provider value={value}>
      {children}
    </RouteNavigationContext.Provider>
  );
}

export function RouteReady({
  pathname,
  children,
}: {
  pathname: string;
  children: ReactNode;
}) {
  const routeNavigation = useContext(RouteNavigationContext);

  useEffect(() => {
    routeNavigation?.pageReady(pathname);
  }, [pathname, routeNavigation]);

  return children;
}
