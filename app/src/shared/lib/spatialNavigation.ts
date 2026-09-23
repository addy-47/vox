import {
  init as initSpatialNav,
  pause as pauseSpatialNav,
  resume as resumeSpatialNav,
} from "@noriginmedia/norigin-spatial-navigation";

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

function isEditableElement(el: EventTarget | null): boolean {
  if (!el || !(el instanceof HTMLElement)) return false;
  const tag = el.tagName.toLowerCase();
  return (
    tag === "input" ||
    tag === "textarea" ||
    el.isContentEditable ||
    el.getAttribute("contenteditable") === "true"
  );
}

export function initSpatialNavigation() {
  if (initialized || typeof window === "undefined") return;
  initialized = true;

  initSpatialNav({
    debug: false,
    visualDebug: false,
    shouldUseNativeEvents: true,
  });

  window.addEventListener("focusin", (e: FocusEvent) => {
    if (isEditableElement(e.target)) {
      pauseSpatialNav();
    }
  });

  window.addEventListener("focusout", (e: FocusEvent) => {
    if (isEditableElement(e.target)) {
      resumeSpatialNav();
    }
  });

  window.addEventListener("keydown", (e: KeyboardEvent) => {
    if (isEditableElement(e.target)) {
      return;
    }

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
