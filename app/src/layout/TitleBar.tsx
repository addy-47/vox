import React, { useEffect, useState } from "react";
import { Minus, Square, X, ArrowUpCircle, Copy, Check } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { checkForUpdates, checkForModelUpdates } from "@/services/modelService";

export const TitleBar: React.FC = () => {
  const navigate = useNavigate();
  const [isTauri, setIsTauri] = useState(false);
  const [isCloseHovered, setIsCloseHovered] = useState(false);
  const [appUpdate, setAppUpdate] = useState<any>(null);
  const [modelUpdate, setModelUpdate] = useState<any>(null);
  const [copied, setCopied] = useState(false);

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

  const handleCopyCommand = (cmd: string) => {
    navigator.clipboard.writeText(cmd);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  useEffect(() => {
    // Check if we are running in Tauri
    const hasTauri = !!(window as any).__TAURI__ || 
                     !!(window as any).__TAURI_INTERNALS__ || 
                     !!(window as any).__TAURI_METADATA__;
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

  if (!isTauri) return null;

  return (
    <div className="relative flex items-center justify-between h-7 shrink-0 select-none z-50 bg-transparent border-b border-transparent">
      {/* Absolute Drag Region */}
      <div className="absolute inset-0 z-0" data-tauri-drag-region />

      <div className="relative z-10 flex items-center gap-2 pl-4">
        <div className="flex items-center gap-1.5 pointer-events-none mr-1">
          <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.6)]" />
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
            <div className="absolute top-6 left-0 w-64 p-4 rounded-xl backdrop-blur-xl bg-[rgb(var(--background))]/95 border border-[rgb(var(--accent))]/20 shadow-2xl opacity-0 translate-y-2 pointer-events-none group-hover/app-pill:opacity-100 group-hover/app-pill:translate-y-0 group-hover/app-pill:pointer-events-auto transition-all duration-300 z-50 text-[13px] text-[rgb(var(--foreground))]">
              <div className="font-bold text-[rgb(var(--accent))] mb-1">App Update Available</div>
              <div className="text-[12px] text-[rgb(var(--foreground-muted))] mb-2">Upgrade from v{appUpdate.current_version} to v{appUpdate.latest_version}</div>
              
              <div className="mb-3">
                <div className="font-black text-[12px] uppercase tracking-wider text-[rgb(var(--foreground-muted))] mb-1">What's New:</div>
                <ul className="list-disc pl-3 space-y-1 text-[12px] text-[rgb(var(--foreground))]/80">
                  {appUpdate.release_notes.map((note: string, idx: number) => (
                    <li key={idx}>{note}</li>
                  ))}
                </ul>
              </div>
              
              <div className="p-2 rounded bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.1)] flex items-center justify-between gap-2">
                <code className="text-[12px] font-mono text-[rgb(var(--foreground))]/80 select-all truncate block flex-1">{appUpdate.update_command}</code>
                <button 
                  onClick={() => handleCopyCommand(appUpdate.update_command)}
                  className="p-1 hover:bg-[rgb(var(--foreground))]/10 rounded text-[rgb(var(--accent))] transition-colors shrink-0"
                  aria-label="Copy command"
                >
                  {copied ? <Check size={11} /> : <Copy size={11} />}
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Model Update Pill */}
        {modelUpdate && (
          <div className="relative group/model-pill pointer-events-auto flex items-center">
            <button className="flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[12px] font-black uppercase tracking-wider bg-gradient-to-r from-[rgb(var(--accent))]/10 to-[rgb(var(--accent))]/20 border border-[rgb(var(--accent))]/30 text-[rgb(var(--accent))] hover:from-[rgb(var(--accent))]/20 hover:to-[rgb(var(--accent))]/30 transition-all duration-300">
              <ArrowUpCircle size={10} className="shrink-0" />
              <span className="hidden sm:inline">Models Update</span>
            </button>
            
            {/* Tooltip Hover Card — compact redesign */}
            <div className="absolute top-6 left-0 w-56 p-3 rounded-xl backdrop-blur-xl bg-[rgb(var(--background))]/95 border border-[rgb(var(--accent))]/20 shadow-2xl opacity-0 translate-y-2 pointer-events-none group-hover/model-pill:opacity-100 group-hover/model-pill:translate-y-0 group-hover/model-pill:pointer-events-auto transition-all duration-300 z-50">
              <div className="flex items-center gap-2 mb-2">
                <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-pulse shrink-0" />
                <span className="text-[12px] font-bold text-[rgb(var(--foreground))]">Model Updates</span>
              </div>

              <div className="text-[12px] text-[rgb(var(--foreground-muted))] mb-2 leading-relaxed">
                v{modelUpdate.remote_version} available · {modelUpdate.outdated_models.length} model{modelUpdate.outdated_models.length > 1 ? 's' : ''} outdated
              </div>

              <div className="mb-2">
                {modelUpdate.outdated_models.map((m: string) => (
                  <span key={m} className="inline-block px-1.5 py-0.5 rounded text-[12px] font-mono text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/8 border border-[rgb(var(--accent))]/15 mr-1 mb-0.5">
                    {m}
                  </span>
                ))}
              </div>

              <button
                onClick={() => navigate('/settings?tab=models')}
                className="text-[12px] font-bold text-[rgb(var(--accent))] hover:text-[rgb(var(--accent))]/80 transition-colors tracking-wide"
              >
                Manage Models →
              </button>
            </div>
          </div>
        )}
      </div>

      <div className="relative z-10 flex items-center h-full">
        <button 
          onClick={handleMinimize}
          className="flex items-center justify-center w-10 h-full text-[rgb(var(--foreground-muted))] hover:bg-[rgb(var(--foreground))]/5 hover:text-[rgb(var(--foreground))] transition-colors"
          aria-label="Minimize"
        >
          <Minus size={14} />
        </button>
        <button 
          onClick={handleMaximize}
          className="flex items-center justify-center w-10 h-full text-[rgb(var(--foreground-muted))] hover:bg-[rgb(var(--foreground))]/5 hover:text-[rgb(var(--foreground))] transition-colors"
          aria-label="Maximize"
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
          aria-label="Close"
        >
          <X size={16} />
        </button>
      </div>
    </div>
  );
};
