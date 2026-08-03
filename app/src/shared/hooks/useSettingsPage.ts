import { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { SETTINGS_DOMAINS as DOMAINS, type SettingsDomainId as DomainId } from "@/data/settingsDomains";
import { useSettings } from "@/shared/context/SettingsContext";
import { useSettingsStore } from "@/store/settingsStore";

export const discardCardChanges = (domainId: DomainId, settings: any, updateDraft: any, draftSettings?: any) => {
  if (!settings) return;
  switch (domainId) {
    case "models": {
      const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
      if (isRealtime) {
        const provId = draftSettings.realtime?.provider || "gemini_live";
        const subkey = provId === "gemini_live" ? "gemini" :
                       provId === "openai_realtime" ? "openai" :
                       provId === "deepgram_voice_agent" ? "deepgram" : "elevenlabs";
                       
        const savedProvConfig = settings.realtime?.[subkey] || {};
        const currentDraftProvConfig = draftSettings.realtime?.[subkey] || {};
        
        const { api_key: _, ...savedClean } = savedProvConfig;
        updateDraft("realtime", subkey, {
          ...currentDraftProvConfig,
          ...savedClean
        });
      } else {
        Object.keys(settings.vad).forEach(k => updateDraft("vad", k, (settings.vad as any)[k]));
        Object.keys(settings.asr).forEach(k => updateDraft("asr", k, (settings.asr as any)[k]));
        updateDraft("llm", "model", settings.llm.model);
        updateDraft("llm", "ctx_size", settings.llm.ctx_size);
        updateDraft("llm", "threads", settings.llm.threads);
        Object.keys(settings.tts).forEach(k => updateDraft("tts", k, (settings.tts as any)[k]));
        if (settings.llm.provider && draftSettings?.llm.provider) {
          updateDraft("llm", "provider", {
            ...draftSettings.llm.provider,
            model: settings.llm.provider.model
          });
        }
      }
      break;
    }
    case "tray":
      updateDraft("ui", "tray_enabled", settings.ui.tray_enabled);
      updateDraft("ui", "tray_blur_density", settings.ui.tray_blur_density);
      updateDraft("ui", "tray_glass_tint", settings.ui.tray_glass_tint);
      updateDraft("ui", "tray_history_limit", settings.ui.tray_history_limit);
      updateDraft("interaction", "tray_mode", settings.interaction.tray_mode);
      break;
    case "persona":
      Object.keys(settings.assistant).forEach(k => updateDraft("assistant", k, (settings.assistant as any)[k]));
      break;
    case "memory":
      Object.keys(settings.persistence).forEach(k => updateDraft("persistence", k, (settings.persistence as any)[k]));
      Object.keys(settings.memory).forEach(k => updateDraft("memory", k, (settings.memory as any)[k]));
      break;
    case "appearance":
      updateDraft("ui", "theme", settings.ui.theme);
      updateDraft("ui", "accent_seed", settings.ui.accent_seed);
      break;
    case "interaction": {
      updateDraft("interaction", "main_app_mode", settings.interaction.main_app_mode);
      updateDraft("interaction", "auto_sleep_timeout", settings.interaction.auto_sleep_timeout);
      updateDraft("interaction", "pipeline_mode", settings.interaction.pipeline_mode);
      const currentDraftModel = draftSettings?.llm.provider?.model || "";
      updateDraft("llm", "provider", {
        ...settings.llm.provider,
        model: currentDraftModel
      });
      
      const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
      if (isRealtime) {
        updateDraft("realtime", "provider", settings.realtime.provider);
        const subkeys = ["gemini", "openai", "deepgram", "elevenlabs"] as const;
        subkeys.forEach(subkey => {
          if (settings.realtime?.[subkey] && draftSettings?.realtime?.[subkey]) {
            updateDraft("realtime", subkey, {
              ...draftSettings.realtime[subkey],
              api_key: settings.realtime[subkey].api_key
            });
          }
        });
      }
      break;
    }
  }
};

export function useSettingsPage() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [activeDomains, setActiveDomains] = useState<DomainId[]>([]);
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
    tray: null,
    memory: null,
    appearance: null,
    interaction: null,
  });

  const { settings } = useSettings();
  const { updateDraft, draftSettings } = useSettingsStore();

  const lastActiveDomains = useRef<DomainId[]>([]);
  useEffect(() => {
    const closed = lastActiveDomains.current.filter((d) => !activeDomains.includes(d));
    if (closed.length > 0 && settings) {
      closed.forEach((domainId) => {
        discardCardChanges(domainId, settings, updateDraft, draftSettings);
      });
    }
    lastActiveDomains.current = activeDomains;
  }, [activeDomains, settings, updateDraft, draftSettings]);

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
        tray: null,
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
        const newLines = { ...lines };
        let changed = false;

        DOMAINS.forEach((domain) => {
          if (!activeDomains.includes(domain.id)) {
            if (newLines[domain.id] !== null) {
              newLines[domain.id] = null;
              changed = true;
            }
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
              case "tray":
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
              const existing = newLines[domain.id];
              if (
                !existing ||
                Math.abs(existing.x1 - x1) > 0.5 ||
                Math.abs(existing.y1 - y1) > 0.5 ||
                Math.abs(existing.x2 - x2) > 0.5 ||
                Math.abs(existing.y2 - y2) > 0.5
              ) {
                newLines[domain.id] = { x1, y1, x2, y2 };
                changed = true;
              }
            }
          } else {
            if (newLines[domain.id] !== null) {
              newLines[domain.id] = null;
              changed = true;
            }
          }
        });

        if (changed) {
          setLines(newLines);
        }
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
