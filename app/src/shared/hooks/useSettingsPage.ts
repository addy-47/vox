import { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { SETTINGS_DOMAINS as DOMAINS, type SettingsDomainId as DomainId } from "@/data/settingsCopy";
import { useSettingsStore } from "@/store/settingsStore";

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
      if (tab && DOMAINS.some((d) => d.id === tab)) {
        return [tab as DomainId];
      }
    }
    return [];
  });
  const [windowWidth, setWindowWidth] = useState(
    typeof window !== "undefined" ? window.innerWidth : 1280
  );
  const [windowHeight, setWindowHeight] = useState(
    typeof window !== "undefined" ? window.innerHeight : 800
  );
  const [isCompact, setIsCompact] = useState(false);

  const [lines, setLines] = useState<Record<DomainId, { x1: number; y1: number; x2: number; y2: number } | null>>({
    persona: null,
    models: null,
    history: null,
    memory: null,
    appearance: null,
    interaction: null,
  });

  const lastActiveDomains = useRef<DomainId[]>([]);
  useEffect(() => {
    const closed = lastActiveDomains.current.filter((d) => !activeDomains.includes(d));
    if (closed.length > 0) {
      closed.forEach((domainId) => {
        // If uncommitted restart-required changes are left behind on card close, discard them safely
        useSettingsStore.getState().discardDomainChanges(domainId);
      });
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
        setIsCompact(window.innerWidth < 1024);
      });
    };
    checkSize();
    window.addEventListener("resize", checkSize);
    return () => {
      window.removeEventListener("resize", checkSize);
      cancelAnimationFrame(rafId);
    };
  }, []);

  const radiusX = useMemo(() => Math.max(90, Math.min(120, windowWidth * 0.09 - 10)), [windowWidth]);
  const radiusY = useMemo(() => Math.max(75, Math.min(120, windowHeight * 0.14 - 8)), [windowHeight]);

  const layoutMode = useMemo<"full-max" | "full-min" | "small">( () => {
    if (isCompact) return "small";
    if (windowWidth < 1366 || activeDomains.length > 1) return "full-min";
    return "full-max";
  }, [isCompact, windowWidth, activeDomains.length]);

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

      if (!clickedInsideNodeOrCard && !clickedCenter) {
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
        history: null,
        memory: null,
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
          const cardEl = document.getElementById(`card-${domain.id}`);

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
              case "history":
                x2 = cardRect.left - containerRect.left;
                y2 = (cardRect.top + cardRect.bottom) / 2 - containerRect.top;
                break;
              case "memory":
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
    const timer = setTimeout(calculate, 320);
    return () => {
      clearTimeout(timer);
      cancelAnimationFrame(calcRafId);
    };
  }, [activeDomains, isCompact, windowWidth, windowHeight]);

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
    handleSelect,
    handleCenterClick,
  };
}
