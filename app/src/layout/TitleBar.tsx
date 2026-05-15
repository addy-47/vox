import React, { useEffect, useState } from "react";
import { Minus, Square, X } from "lucide-react";
// Removed unused import

export const TitleBar: React.FC = () => {
  const [isTauri, setIsTauri] = useState(false);
  const [isCloseHovered, setIsCloseHovered] = useState(false);

  useEffect(() => {
    // Check if we are running in Tauri
    const hasTauri = !!(window as any).__TAURI__ || 
                     !!(window as any).__TAURI_INTERNALS__ || 
                     !!(window as any).__TAURI_METADATA__;
    setIsTauri(hasTauri);

    if (hasTauri) {
      let unlistenFocus: (() => void) | undefined;
      let unlistenBlur: (() => void) | undefined;

      const setupListeners = async () => {
        try {
          const { getCurrentWindow } = await import("@tauri-apps/api/window");
          const win = getCurrentWindow();
          
          unlistenFocus = await win.listen("tauri://focus", () => {
            setIsCloseHovered(false);
          });
          unlistenBlur = await win.listen("tauri://blur", () => {
            setIsCloseHovered(false);
          });
        } catch (err) {
          console.error("[TitleBar] Failed to setup listeners:", err);
        }
      };

      setupListeners();
      
      // Also use standard web focus event for redundancy
      const handleWebFocus = () => setIsCloseHovered(false);
      window.addEventListener('focus', handleWebFocus);
      
      return () => {
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
    } catch (e) {}
  };

  const handleMaximize = async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().toggleMaximize();
    } catch (e) {}
  };

  const handleClose = async () => {
    setIsCloseHovered(false);
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().close();
    } catch (e) {}
  };

  if (!isTauri) return null;

  return (
    <div className="relative flex items-center justify-between h-8 shrink-0 select-none z-50 bg-[rgb(var(--background))] border-b border-[rgba(var(--border),0.05)] transition-all duration-400 ease-in-out">
      {/* Absolute Drag Region */}
      <div className="absolute inset-0 z-0" data-tauri-drag-region />

      <div className="relative z-10 flex items-center gap-2 pl-4 pointer-events-none">
        <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.6)]" />
        <span className="text-[11px] font-bold tracking-[0.2em] text-[rgb(var(--foreground-muted))] uppercase">Vox</span>
      </div>

      <div className="relative z-10 flex items-center h-full">
        <button 
          onClick={handleMinimize}
          className="flex items-center justify-center w-10 h-full text-[rgb(var(--foreground-muted))] hover:bg-[rgb(var(--foreground))]/5 hover:text-[rgb(var(--foreground))] transition-colors"
          title="Minimize"
        >
          <Minus size={14} />
        </button>
        <button 
          onClick={handleMaximize}
          className="flex items-center justify-center w-10 h-full text-[rgb(var(--foreground-muted))] hover:bg-[rgb(var(--foreground))]/5 hover:text-[rgb(var(--foreground))] transition-colors"
          title="Maximize"
        >
          <Square size={12} />
        </button>
        <button 
          onClick={handleClose}
          onMouseEnter={() => setIsCloseHovered(true)}
          onMouseLeave={() => setIsCloseHovered(false)}
          className={`flex items-center justify-center w-10 h-full transition-colors ${
            isCloseHovered 
              ? "bg-red-500 text-white" 
              : "text-[rgb(var(--foreground-muted))]"
          }`}
          title="Close"
        >
          <X size={16} />
        </button>
      </div>
    </div>
  );
};
