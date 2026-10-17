import React, { useEffect, useState } from "react";
import { Minus, Square, X, ArrowUpCircle, Copy, Check, Sparkles, Cpu, ChevronRight } from "lucide-react";
import { LAYOUT_COPY } from "@/data/layoutCopy";
import { useNavigate } from "react-router-dom";
import { checkForUpdates, checkForModelUpdates, type UpdateReport, type ModelUpdateReport } from "@/services/setupService";
import { copyToClipboard } from "@/shared/lib/clipboard";
import { isRealMobileDevice } from "@/layout/breakpoints";

export const TitleBar: React.FC = () => {
  if (isRealMobileDevice()) return null;

  const navigate = useNavigate();
  const [isTauri, setIsTauri] = useState(false);
  const [isCloseHovered, setIsCloseHovered] = useState(false);
  const [appUpdate, setAppUpdate] = useState<UpdateReport | null>(null);
  const [modelUpdate, setModelUpdate] = useState<ModelUpdateReport | null>(null);
  const [copied, setCopied] = useState(false);
  const [modelCardOpen, setModelCardOpen] = useState(false);

  useEffect(() => {
    if (!isTauri) return;
    const fetchUpdates = async () => {
      try {
        const appRes = await checkForUpdates();
        if (appRes && appRes.update_available) {
          setAppUpdate(appRes);
        } else {
          setAppUpdate(null);
        }
      } catch (e) {
        console.warn("Failed to check app updates:", e);
      }

      try {
        const modelRes = await checkForModelUpdates();
        if (modelRes && modelRes.update_available) {
          setModelUpdate(modelRes);
        } else {
          setModelUpdate(null);
        }
      } catch (e) {
        console.warn("Failed to check model updates:", e);
      }
    };
    
    fetchUpdates();
  }, [isTauri]);

  const handleCopyCommand = async (cmd: string) => {
    const ok = await copyToClipboard(cmd);
    if (ok) {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  // Hover cards reveal on hover/focus. Blur-dismiss restores keyboard and
  // touch parity: Escape or an outside tap drops focus, hiding the card.
  useEffect(() => {
    const cardOf = (el: Element | null) => el?.closest?.("[data-hover-card]") ?? null;
    const dismissIfInside = (target: EventTarget | null) => {
      const active = document.activeElement as HTMLElement | null;
      const card = active ? cardOf(active) : null;
      if (card && target instanceof Node && !card.contains(target)) active?.blur();
    };
    const onPointerDown = (e: PointerEvent) => dismissIfInside(e.target);
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        const active = document.activeElement as HTMLElement | null;
        if (active && cardOf(active)) active.blur();
      }
    };
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKeyDown, true);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKeyDown, true);
    };
  }, []);

  useEffect(() => {
    // Check if we are running in Tauri
    const hasTauri = !!window.__TAURI__ ||
                     !!window.__TAURI_INTERNALS__ ||
                     !!window.__TAURI_METADATA__;
    setIsTauri(hasTauri);

    if (hasTauri) {
      let isMounted = true;
      let unlistenFocus: (() => void) | undefined;
      let unlistenBlur: (() => void) | undefined;

      const setupListeners = async () => {
        try {
          const { getCurrentWindow } = await import("@tauri-apps/api/window");
          const win = getCurrentWindow();
          
          const fnFocus = await win.listen("tauri://focus", () => {
            if (isMounted) setIsCloseHovered(false);
          });
          if (isMounted) {
            unlistenFocus = fnFocus;
          } else {
            fnFocus();
          }

          const fnBlur = await win.listen("tauri://blur", () => {
            if (isMounted) setIsCloseHovered(false);
          });
          if (isMounted) {
            unlistenBlur = fnBlur;
          } else {
            fnBlur();
          }
        } catch (err) {
          console.error("[TitleBar] Failed to setup listeners:", err);
        }
      };

      setupListeners();
      
      // Also use standard web focus event for redundancy
      const handleWebFocus = () => {
        if (isMounted) setIsCloseHovered(false);
      };
      window.addEventListener('focus', handleWebFocus);
      
      return () => {
        isMounted = false;
        if (unlistenFocus) unlistenFocus();
        if (unlistenBlur) unlistenBlur();
        window.removeEventListener('focus', handleWebFocus);
      };
    }
    return () => {};
  }, []);

  const handleMinimize = async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().minimize();
    } catch (e) {
      console.error("[TitleBar] Minimize window failed:", e);
    }
  };

  const handleMaximize = async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().toggleMaximize();
    } catch (e) {
      console.error("[TitleBar] Maximize window failed:", e);
    }
  };

  const handleClose = async () => {
    setIsCloseHovered(false);
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().close();
    } catch (e) {
      console.error("[TitleBar] Close window failed:", e);
    }
  };

  if (!isTauri || isRealMobileDevice()) return null;

  return (
    // A desktop window titlebar (drag region + minimise/maximise/close) has no
    // meaning on a handheld OS and is the single most obvious "this is a web app
    // in a WebView" tell. Android supplies its own system bar and back gesture.
    <div className={`${isRealMobileDevice() ? 'hidden' : 'relative flex'} items-center justify-between h-7 shrink-0 select-none z-50 bg-transparent border-b border-transparent`}>
      {/* Absolute Drag Region */}
      <div className="absolute inset-0 z-0" data-tauri-drag-region />

      <div className="relative z-10 flex items-center gap-2 pl-4">
        <div className="flex items-center gap-1.5 pointer-events-none mr-1">
          <div data-flip-trace="title-bar-dot" className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.6)]" />
          <span className="text-[12px] font-bold tracking-[0.2em] text-[rgb(var(--foreground-muted))] uppercase">Vox</span>
        </div>

        {/* App Update Pill */}
        {appUpdate && (
          <div className="relative group/app-pill pointer-events-auto flex items-center">
            <button className="flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[12px] font-black uppercase tracking-wider bg-gradient-to-r from-[rgb(var(--accent))]/10 to-[rgb(var(--accent))]/20 border border-[rgb(var(--accent))]/30 text-[rgb(var(--accent))] hover:from-[rgb(var(--accent))]/20 hover:to-[rgb(var(--accent))]/30 transition-all duration-300">
              <ArrowUpCircle size={10} className="shrink-0" />
              <span className="hidden sm:inline">v{appUpdate.latest_version} Available</span>
            </button>
            
            {/* Tooltip Hover Card */}
            <div
              data-hover-card
              className="absolute top-7 left-0 w-80 p-3 rounded-xl bg-[rgba(var(--card),0.96)] dark:bg-[rgba(var(--card),0.98)] backdrop-blur-md border border-[rgba(var(--foreground),0.08)] shadow-[0_4px_16px_rgba(0,0,0,0.08)] dark:shadow-[0_4px_20px_rgba(0,0,0,0.25)] opacity-0 translate-y-1 pointer-events-none group-hover/app-pill:opacity-100 group-hover/app-pill:translate-y-0 group-hover/app-pill:pointer-events-auto group-focus-within/app-pill:opacity-100 group-focus-within/app-pill:translate-y-0 group-focus-within/app-pill:pointer-events-auto transition-all duration-200 z-50 text-[12px] text-[rgb(var(--foreground))] flex flex-col gap-2.5"
            >
              <div className="flex items-center justify-between gap-2">
                <div className="font-bold text-[13px] text-[rgb(var(--foreground))]">{LAYOUT_COPY.titleBar.appUpdate}</div>
                <span className="text-[10.5px] font-mono font-semibold px-1.5 py-0.5 rounded-md bg-[rgba(var(--accent),0.1)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.2)]">
                  v{appUpdate.latest_version}
                </span>
              </div>
              <div className="text-[11.5px] text-[rgb(var(--foreground-muted))]">Upgrade from v{appUpdate.current_version} to v{appUpdate.latest_version}</div>
              
              <div>
                <div className="font-semibold text-[10.5px] uppercase tracking-wider text-[rgb(var(--foreground-muted))]/80 mb-1">{LAYOUT_COPY.titleBar.whatsNew}</div>
                <ul className="list-disc pl-3 space-y-0.5 text-[11.5px] text-[rgb(var(--foreground))]/85">
                  {appUpdate.release_notes.map((note: string, idx: number) => (
                    <li key={idx}>{note}</li>
                  ))}
                </ul>
              </div>
              
              <div className="p-2 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-between gap-2">
                <code className="text-[11px] font-mono text-[rgb(var(--foreground))]/80 select-all truncate block flex-1">{appUpdate.update_command}</code>
                <button 
                  onClick={() => handleCopyCommand(appUpdate.update_command)}
                  className="p-1 hover:bg-[rgba(var(--accent),0.15)] rounded text-[rgb(var(--accent))] transition-colors shrink-0 cursor-pointer"
                  aria-label={LAYOUT_COPY.titleBar.copyCommand}
                >
                  {copied ? <Check size={12} /> : <Copy size={12} />}
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Model Update Pill */}
        {modelUpdate && (
          <div className="relative group/model-pill pointer-events-auto flex items-center">
            <button
              onClick={() => setModelCardOpen((v) => !v)}
              aria-expanded={modelCardOpen}
              className="flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[12px] font-black uppercase tracking-wider bg-gradient-to-r from-[rgb(var(--accent))]/10 to-[rgb(var(--accent))]/20 border border-[rgb(var(--accent))]/30 text-[rgb(var(--accent))] hover:from-[rgb(var(--accent))]/20 hover:to-[rgb(var(--accent))]/30 transition-all duration-300"
            >
              <ArrowUpCircle size={10} className="shrink-0" />
              <span className="hidden sm:inline">{LAYOUT_COPY.titleBar.modelsUpdate}</span>
            </button>

            {/* Model updates popover */}
            <div
              data-hover-card
              className={`absolute top-7 left-0 w-80 max-w-[calc(100vw-2rem)] rounded-xl bg-[rgba(var(--card),0.96)] dark:bg-[rgba(var(--card),0.98)] backdrop-blur-md border border-[rgba(var(--foreground),0.08)] shadow-[0_4px_16px_rgba(0,0,0,0.08)] dark:shadow-[0_4px_20px_rgba(0,0,0,0.25)] z-50 overflow-hidden transition-all duration-200 p-3 flex flex-col gap-2.5 ${
                modelCardOpen
                  ? "opacity-100 translate-y-0 pointer-events-auto"
                  : "opacity-0 translate-y-1 pointer-events-none group-hover/model-pill:opacity-100 group-hover/model-pill:translate-y-0 group-hover/model-pill:pointer-events-auto group-focus-within/model-pill:opacity-100 group-focus-within/model-pill:translate-y-0 group-focus-within/model-pill:pointer-events-auto"
              }`}
            >
              <div className="flex items-center justify-between gap-2.5">
                <div className="flex items-center gap-2 min-w-0">
                  <div className="w-7 h-7 rounded-md bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                    <Sparkles size={14} />
                  </div>
                  <div className="flex flex-col min-w-0">
                    <span className="text-[13px] font-bold text-[rgb(var(--foreground))] tracking-tight truncate leading-tight">
                      {LAYOUT_COPY.titleBar.modelUpdates}
                    </span>
                    <span className="text-[11px] text-[rgb(var(--foreground-muted))]/80 truncate mt-0.5 leading-tight">
                      {LAYOUT_COPY.titleBar.modelsOutdatedCount(modelUpdate.outdated_models.length)}
                    </span>
                  </div>
                </div>
                <span className="text-[10.5px] font-mono font-semibold px-1.5 py-0.5 rounded-md bg-[rgba(var(--accent),0.1)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.2)] shrink-0">
                  v{modelUpdate.remote_version}
                </span>
              </div>

              <ul className="max-h-44 overflow-y-auto overscroll-contain custom-scrollbar flex flex-col gap-1 pr-0.5">
                {modelUpdate.outdated_models.map((m: string) => (
                  <li
                    key={m}
                    className="flex items-center justify-between gap-2 px-2.5 py-1.5 rounded-md bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--foreground),0.05)] hover:border-[rgba(var(--accent),0.25)] transition-all min-w-0"
                  >
                    <div className="flex items-center gap-2 min-w-0 flex-1">
                      <Cpu size={12} className="text-[rgb(var(--accent))] shrink-0 opacity-80" />
                      <span className="text-[11.5px] font-mono font-medium text-[rgb(var(--foreground))] truncate min-w-0">
                        {m}
                      </span>
                    </div>
                    <span className="text-[9.5px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))] bg-[rgba(var(--accent),0.1)] px-1.5 py-0.5 rounded-sm border border-[rgba(var(--accent),0.2)] shrink-0">
                      {LAYOUT_COPY.titleBar.updateAvailable}
                    </span>
                  </li>
                ))}
              </ul>

              <button
                type="button"
                onClick={() => {
                  setModelCardOpen(false);
                  navigate("/settings");
                  window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: "models" }));
                }}
                className="w-full py-1.5 px-3 rounded-lg bg-[rgba(var(--accent),0.1)] hover:bg-[rgb(var(--accent))] text-[rgb(var(--accent))] hover:text-[rgb(var(--accent-foreground))] font-bold text-[11px] tracking-wide uppercase transition-all flex items-center justify-center gap-1.5 cursor-pointer border border-[rgba(var(--accent),0.2)] hover:border-[rgb(var(--accent))]"
              >
                <span>Manage in Model Hub</span>
                <ChevronRight size={13} />
              </button>
            </div>
          </div>
        )}
      </div>

      <div className="relative z-10 flex items-center h-full" data-titlebar-window-controls>
        <button 
          onClick={handleMinimize}
          className="flex items-center justify-center w-10 h-full text-[rgb(var(--foreground-muted))] hover:bg-[rgb(var(--foreground))]/5 hover:text-[rgb(var(--foreground))] transition-colors"
          aria-label={LAYOUT_COPY.titleBar.minimize}
        >
          <Minus size={14} />
        </button>
        <button 
          onClick={handleMaximize}
          className="flex items-center justify-center w-10 h-full text-[rgb(var(--foreground-muted))] hover:bg-[rgb(var(--foreground))]/5 hover:text-[rgb(var(--foreground))] transition-colors"
          aria-label={LAYOUT_COPY.titleBar.maximize}
        >
          <Square size={12} />
        </button>
        <button 
          onClick={handleClose}
          onMouseEnter={() => setIsCloseHovered(true)}
          onMouseLeave={() => setIsCloseHovered(false)}
          className={`flex items-center justify-center w-10 h-full transition-colors ${
            isCloseHovered 
              ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]" 
              : "text-[rgb(var(--foreground-muted))]"
          }`}
          aria-label={LAYOUT_COPY.titleBar.close}
        >
          <X size={16} />
        </button>
      </div>
    </div>
  );
};
