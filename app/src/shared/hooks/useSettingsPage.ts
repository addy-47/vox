import { useState, useEffect, useLayoutEffect, useRef, useMemo, useCallback } from "react";
import { SETTINGS_DOMAINS as DOMAINS, type SettingsDomainId as DomainId } from "@/data/settingsCopy";
import { useSettingsStore } from "@/store/settingsStore";
import { isCompactWidth } from "@/layout/breakpoints";
import { isRealMobileDevice } from "@/shared/hooks/useMobileViewport";

interface LineCoords {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

export function useSettingsPage() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [activeDomains, setActiveDomains] = useState<DomainId[]>(() => {
    if (typeof window !== "undefined") {
      const params = new URLSearchParams(window.location.search);
      const tab = params.get("tab");
      const match = tab ? DOMAINS.find((d) => d.id === tab) : undefined;
      if (match) {
        return [match.id];
      }
    }
    return [];
  });
  const [windowWidth, setWindowWidth] = useState(() =>
    typeof window !== "undefined" ? window.innerWidth : 1280
  );
  const [windowHeight, setWindowHeight] = useState(() =>
    typeof window !== "undefined" ? window.innerHeight : 800
  );
  const [isCompact, setIsCompact] = useState(() =>
    typeof window !== "undefined" ? isRealMobileDevice() || isCompactWidth() : false
  );

  const [lines, setLines] = useState<Record<DomainId, { x1: number; y1: number; x2: number; y2: number } | null>>({
    persona: null,
    models: null,
    working_memory: null,
    personal_memory: null,
    appearance: null,
    interaction: null,
  });

  const lastActiveDomains = useRef<DomainId[]>([]);
  useEffect(() => {
    const closed = lastActiveDomains.current.filter((d) => !activeDomains.includes(d));
    if (closed.length > 0) {
      // Survive, don't discard: flush any pending autosave immediately so hot
      // changes commit instead of dying with the 600ms timer. Restart-classified
      // dirty state stays in the draft (no prompt, no auto-apply) and is still
      // there — with its Apply button — when the card reopens. Explicit
      // per-card discard remains available in SettingsCardWrapper.
      useSettingsStore.getState().flushPendingAutosave();
    }
    lastActiveDomains.current = activeDomains;
  }, [activeDomains]);

  useEffect(() => {
    let rafId: number;
    const checkSize = () => {
      cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(() => {
        setWindowWidth(window.innerWidth);
        setWindowHeight(window.innerHeight);
        setIsCompact(isRealMobileDevice() || isCompactWidth());
      });
    };
    window.addEventListener("resize", checkSize);
    return () => {
      window.removeEventListener("resize", checkSize);
      cancelAnimationFrame(rafId);
    };
  }, []);

  // In the current main layout (>= 1440w and >= 850h), the radial hub maintains its standard size.
  // In constrained / resized window layouts (like second image), the center node dynamically pulls
  // satellite nodes closer to avoid crowding surrounding cards while strictly preserving the hub shape.
  const dynamicPull = useMemo(() => {
    const widthRatio = Math.min(1.0, windowWidth / 1440);
    const heightRatio = Math.min(1.0, windowHeight / 850);
    return Math.max(0.72, Math.min(1.0, Math.min(widthRatio, heightRatio)));
  }, [windowWidth, windowHeight]);

  const baselineRadiusX = useMemo(
    () => Math.round(Math.max(68, Math.min(120, (windowWidth * 0.09 - 10) * dynamicPull))),
    [windowWidth, dynamicPull]
  );
  const baselineRadiusY = useMemo(
    () => Math.round(Math.max(58, Math.min(120, (windowHeight * 0.14 - 8) * dynamicPull))),
    [windowHeight, dynamicPull]
  );

  // Dynamic inward pull states: dynamically pull radial nodes inward if surrounding cards sit close
  const [dynamicRadiusX, setDynamicRadiusX] = useState<number | null>(null);
  const [dynamicRadiusY, setDynamicRadiusY] = useState<number | null>(null);

  const radiusX = dynamicRadiusX ?? baselineRadiusX;
  const radiusY = dynamicRadiusY ?? baselineRadiusY;

  const layoutMode = useMemo<"full-max" | "full-min" | "small">( () => {
    if (isCompact) return "small";
    if (windowWidth < 1366 || activeDomains.length > 1) return "full-min";
    return "full-max";
  }, [isCompact, windowWidth, activeDomains.length]);

  // In constrained vertical viewports (like window height < 840), side cards collapse into
  // a unified carousel card to prevent vertical crowding, converging diagonal connectors.
  const isSideCarousel = useMemo(() => {
    if (isCompact) return false;
    return windowHeight < 840;
  }, [isCompact, windowHeight]);

  // ── Dedicated Dynamic Radial Hub Clearance Effect ──
  // Always runs to guarantee that the radial nodes never overlap surrounding cards (such as Persona above).
  useLayoutEffect(() => {
    if (isCompact) return;

    const measureClearance = () => {
      const centerNodeEl = document.getElementById("center-node");
      if (!centerNodeEl || !containerRef.current) return;

      const centerRect = centerNodeEl.getBoundingClientRect();
      const cy = (centerRect.top + centerRect.bottom) / 2;
      const cx = (centerRect.left + centerRect.right) / 2;

      let maxSafeRadiusY = baselineRadiusY;
      let maxSafeRadiusX = baselineRadiusX;

      // 1. Persona card (12:00 node at cy - radiusY). Node radius is 20px, min gap is 24px => 44px
      const personaCardEl = document.getElementById("card-persona");
      if (personaCardEl) {
        const personaRect = personaCardEl.getBoundingClientRect();
        const clearanceY = (cy - personaRect.bottom) - 44;
        if (clearanceY > 15) {
          maxSafeRadiusY = Math.min(maxSafeRadiusY, Math.round(clearanceY));
        }
      }

      // 2. Appearance card (6:00 node at cy + radiusY)
      const appearanceCardEl = document.getElementById("card-appearance");
      if (appearanceCardEl) {
        const appearanceRect = appearanceCardEl.getBoundingClientRect();
        const clearanceY = (appearanceRect.top - cy) - 44;
        if (clearanceY > 15) {
          maxSafeRadiusY = Math.min(maxSafeRadiusY, Math.round(clearanceY));
        }
      }

      // 3. Left side cards (10:00 & 8:00 nodes at cx - radiusX * cos(30°))
      const leftCardEl = document.getElementById("card-left-carousel") || document.getElementById("card-interaction") || document.getElementById("card-personal_memory");
      if (leftCardEl) {
        const leftRect = leftCardEl.getBoundingClientRect();
        const clearanceX = (cx - leftRect.right - 44) / 0.866;
        if (clearanceX > 25) {
          maxSafeRadiusX = Math.min(maxSafeRadiusX, Math.round(clearanceX));
        }
      }

      // 4. Right side cards (2:00 & 4:00 nodes at cx + radiusX * cos(30°))
      const rightCardEl = document.getElementById("card-right-carousel") || document.getElementById("card-models") || document.getElementById("card-working_memory");
      if (rightCardEl) {
        const rightRect = rightCardEl.getBoundingClientRect();
        const clearanceX = (rightRect.left - cx - 44) / 0.866;
        if (clearanceX > 25) {
          maxSafeRadiusX = Math.min(maxSafeRadiusX, Math.round(clearanceX));
        }
      }

      const targetRadiusX = Math.max(50, Math.min(baselineRadiusX, maxSafeRadiusX));
      const targetRadiusY = Math.max(40, Math.min(baselineRadiusY, maxSafeRadiusY));

      setDynamicRadiusX((prev) => (prev === null || Math.abs(prev - targetRadiusX) > 1.5 ? targetRadiusX : prev));
      setDynamicRadiusY((prev) => (prev === null || Math.abs(prev - targetRadiusY) > 1.5 ? targetRadiusY : prev));
    };

    measureClearance();
    const t1 = setTimeout(measureClearance, 50);
    const t2 = setTimeout(measureClearance, 220);

    let ro: ResizeObserver | null = null;
    if (typeof ResizeObserver !== "undefined" && containerRef.current) {
      ro = new ResizeObserver(measureClearance);
      ro.observe(containerRef.current);
      ["persona", "appearance", "interaction", "models", "working_memory", "personal_memory"].forEach((id) => {
        const el = document.getElementById(`card-${id}`);
        if (el && ro) ro.observe(el);
      });
      const leftCar = document.getElementById("card-left-carousel");
      if (leftCar && ro) ro.observe(leftCar);
      const rightCar = document.getElementById("card-right-carousel");
      if (rightCar && ro) ro.observe(rightCar);
    }

    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
      if (ro) ro.disconnect();
    };
  }, [isCompact, windowWidth, windowHeight, baselineRadiusX, baselineRadiusY, activeDomains]);

  useEffect(() => {
    if (isCompact) return;
    const handleOutsideClick = (e: MouseEvent) => {
      if (activeDomains.length === 0) return;
      const target = e.target as HTMLElement;
      if (!containerRef.current || !containerRef.current.contains(target)) return;

      const clickedInsideNodeOrCard = DOMAINS.some((domain) => {
        const nodeEl = document.getElementById(`node-${domain.id}`);
        const cardEl = document.getElementById(`card-${domain.id}`);
        return (nodeEl && nodeEl.contains(target)) || (cardEl && cardEl.contains(target));
      });

      const centerNodeEl = document.getElementById("center-node");
      const clickedCenter = centerNodeEl && centerNodeEl.contains(target);

      const leftCarouselEl = document.getElementById("card-left-carousel");
      const rightCarouselEl = document.getElementById("card-right-carousel");
      const clickedCarousel = (leftCarouselEl && leftCarouselEl.contains(target)) || (rightCarouselEl && rightCarouselEl.contains(target));

      if (!clickedInsideNodeOrCard && !clickedCenter && !clickedCarousel) {
        setActiveDomains((prev) => prev.slice(0, -1));
      }
    };

    document.addEventListener("mousedown", handleOutsideClick);
    return () => document.removeEventListener("mousedown", handleOutsideClick);
  }, [activeDomains, isCompact]);

  useEffect(() => {
    if (isCompact || activeDomains.length === 0) {
      setLines({
        persona: null,
        models: null,
        working_memory: null,
        personal_memory: null,
        appearance: null,
        interaction: null,
      });
      return;
    }

    let calcRafId: number;
    const calculate = () => {
      if (!containerRef.current) return;
      cancelAnimationFrame(calcRafId);
      calcRafId = requestAnimationFrame(() => {
        if (!containerRef.current) return;
        const containerRect = containerRef.current.getBoundingClientRect();

        const calculatedLines: Record<string, LineCoords | null> = {};

        DOMAINS.forEach((domain) => {
          if (!activeDomains.includes(domain.id)) {
            calculatedLines[domain.id] = null;
            return;
          }

          const nodeEl = document.getElementById(`node-${domain.id}`);
          let cardEl = document.getElementById(`card-${domain.id}`);
          if (!cardEl && isSideCarousel) {
            if (domain.id === "interaction" || domain.id === "personal_memory") {
              cardEl = document.getElementById("card-left-carousel");
            } else if (domain.id === "models" || domain.id === "working_memory") {
              cardEl = document.getElementById("card-right-carousel");
            }
          }

          if (nodeEl && cardEl) {
            const nodeRect = nodeEl.getBoundingClientRect();
            const cardRect = cardEl.getBoundingClientRect();

            const x1 = (nodeRect.left + nodeRect.right) / 2 - containerRect.left;
            const y1 = (nodeRect.top + nodeRect.bottom) / 2 - containerRect.top;

            let x2 = 0;
            let y2 = 0;

            switch (domain.id) {
              case "persona":
                x2 = (cardRect.left + cardRect.right) / 2 - containerRect.left;
                y2 = cardRect.bottom - containerRect.top;
                break;
              case "appearance":
                x2 = (cardRect.left + cardRect.right) / 2 - containerRect.left;
                y2 = cardRect.top - containerRect.top;
                break;
              case "models":
              case "working_memory":
                x2 = cardRect.left - containerRect.left;
                y2 = (cardRect.top + cardRect.bottom) / 2 - containerRect.top;
                break;
              case "personal_memory":
              case "interaction":
                x2 = cardRect.right - containerRect.left;
                y2 = (cardRect.top + cardRect.bottom) / 2 - containerRect.top;
                break;
            }

            if (!isNaN(x1) && !isNaN(y1) && !isNaN(x2) && !isNaN(y2)) {
              calculatedLines[domain.id] = { x1, y1, x2, y2 };
            } else {
              calculatedLines[domain.id] = null;
            }
          } else {
            calculatedLines[domain.id] = null;
          }
        });

        setLines((prevLines) => {
          let changed = false;
          const newLines = { ...prevLines };

          DOMAINS.forEach((domain) => {
            const calculated = calculatedLines[domain.id];
            const existing = prevLines[domain.id];

            if (calculated === null || calculated === undefined) {
              if (existing !== null) {
                newLines[domain.id] = null;
                changed = true;
              }
            } else {
              if (
                !existing ||
                Math.abs(existing.x1 - calculated.x1) > 0.5 ||
                Math.abs(existing.y1 - calculated.y1) > 0.5 ||
                Math.abs(existing.x2 - calculated.x2) > 0.5 ||
                Math.abs(existing.y2 - calculated.y2) > 0.5
              ) {
                newLines[domain.id] = calculated;
                changed = true;
              }
            }
          });

          return changed ? newLines : prevLines;
        });
      });
    };

    calculate();
    const timer1 = setTimeout(calculate, 100);
    const timer2 = setTimeout(calculate, 320);
    const timer3 = setTimeout(calculate, 600);

    // Watch for card and container resizes (e.g. when lazy-loaded card chunks mount and expand)
    let observer: ResizeObserver | null = null;
    if (typeof ResizeObserver !== "undefined" && containerRef.current) {
      observer = new ResizeObserver(() => {
        calculate();
      });
      observer.observe(containerRef.current);
      DOMAINS.forEach((d) => {
        const el = document.getElementById(`card-${d.id}`);
        if (el && observer) observer.observe(el);
      });
      const leftCar = document.getElementById("card-left-carousel");
      if (leftCar && observer) observer.observe(leftCar);
      const rightCar = document.getElementById("card-right-carousel");
      if (rightCar && observer) observer.observe(rightCar);
    }

    return () => {
      clearTimeout(timer1);
      clearTimeout(timer2);
      clearTimeout(timer3);
      cancelAnimationFrame(calcRafId);
      if (observer) observer.disconnect();
    };
  }, [activeDomains, isCompact, windowWidth, windowHeight, radiusX, radiusY]);

  const handleSelect = useCallback((id: DomainId) => {
    setActiveDomains((prev) => {
      if (prev.includes(id)) {
        return prev.filter((d) => d !== id);
      } else {
        if (isCompact) {
          return [id];
        }
        return [...prev, id];
      }
    });
  }, [isCompact]);

  const handleCenterClick = useCallback(() => {
    setActiveDomains((prev) => (prev.length > 0 ? [] : DOMAINS.map((d) => d.id)));
  }, []);

  return {
    containerRef,
    activeDomains,
    setActiveDomains,
    isCompact,
    lines,
    radiusX,
    radiusY,
    layoutMode,
    isSideCarousel,
    handleSelect,
    handleCenterClick,
  };
}
