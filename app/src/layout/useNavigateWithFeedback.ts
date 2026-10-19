import { useCallback, useContext } from "react";
import { useNavigate, type NavigateOptions } from "react-router-dom";
import { RouteNavigationContext } from "./routeNavigationContext";

export function useNavigateWithFeedback() {
  const routerNavigate = useNavigate();
  const routeNavigation = useContext(RouteNavigationContext);

  return useCallback(
    (to: string, onReady?: () => void, options?: NavigateOptions) => {
      if (routeNavigation) {
        routeNavigation.navigate(to, onReady, options);
        return;
      }

      void routerNavigate(to, options);
      if (onReady) requestAnimationFrame(onReady);
    },
    [routeNavigation, routerNavigate]
  );
}
