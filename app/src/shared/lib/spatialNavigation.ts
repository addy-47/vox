import { init as initSpatialNav } from "@noriginmedia/norigin-spatial-navigation";

export const SPATIAL_CONTAINERS = {
  STAGE: "STAGE",
  DOCK: "DOCK",
  CLUSTER: "CLUSTER",
  RAIL: "RAIL",
} as const;

let initialized = false;
let isNavigatingWithKeys = false;
let keyNavTimer: ReturnType<typeof setTimeout> | null = null;
const listeners = new Set<(navigating: boolean) => void>();

function notify(val: boolean) {
  isNavigatingWithKeys = val;
  if (typeof document !== "undefined") {
    if (val) {
      document.body.setAttribute("data-spatial-navigating", "true");
    } else {
      document.body.removeAttribute("data-spatial-navigating");
    }
  }
  listeners.forEach((fn) => fn(val));
}

export function initSpatialNavigation() {
  if (initialized || typeof window === "undefined") return;
  initialized = true;

  initSpatialNav({
    debug: false,
    visualDebug: false,
  });

  window.addEventListener("keydown", (e: KeyboardEvent) => {
    if (
      e.key === "ArrowUp" ||
      e.key === "ArrowDown" ||
      e.key === "ArrowLeft" ||
      e.key === "ArrowRight"
    ) {
      notify(true);
      if (keyNavTimer) clearTimeout(keyNavTimer);
      keyNavTimer = setTimeout(() => {
        notify(false);
      }, 800);
    }
  });

  window.addEventListener("mousemove", () => {
    if (isNavigatingWithKeys) {
      if (keyNavTimer) clearTimeout(keyNavTimer);
      notify(false);
    }
  }, { passive: true });
}

export function isSpatialNavigating(): boolean {
  return isNavigatingWithKeys;
}

export function subscribeSpatialNavigating(fn: (navigating: boolean) => void): () => void {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}
