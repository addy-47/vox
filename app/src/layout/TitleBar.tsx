import React, { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
// Removed unused import

export const TitleBar: React.FC = () => {
  const [isTauri, setIsTauri] = useState(false);
  const [isCloseHovered, setIsCloseHovered] = useState(false);

  useEffect(() => {
    // Check if we are running in Tauri
    setIsTauri(!!(window as any).__TAURI_INTERNALS__);

    if ((window as any).__TAURI_INTERNALS__) {
      const win = getCurrentWindow();
      const unlistenFocus = win.listen("tauri://focus", () => setIsCloseHovered(false));
      const unlistenBlur = win.listen("tauri://blur", () => setIsCloseHovered(false));
      
      return () => {
        unlistenFocus.then(u => u());
        unlistenBlur.then(u => u());
      };
    }
    return () => {};
  }, []);

  const handleMinimize = async () => {
    try {
      await getCurrentWindow().minimize();
    } catch (e) {}
  };

  const handleMaximize = async () => {
    try {
      await getCurrentWindow().toggleMaximize();
    } catch (e) {}
  };

  const handleClose = async () => {
    setIsCloseHovered(false);
    try {
      await getCurrentWindow().close();
    } catch (e) {}
  };

  if (!isTauri) return null;

  return (
    <div 
      className="flex items-center justify-between h-8 shrink-0 select-none z-50 bg-[rgb(var(--background))] border-b border-[rgba(var(--border),0.00)] transition-colors duration-300"
      data-tauri-drag-region
    >
      <div className="flex items-center gap-2 pl-4 pointer-events-none" data-tauri-drag-region>
        <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.6)]" />
        <span className="text-[10px] font-bold tracking-[0.2em] text-[rgb(var(--foreground-muted))] uppercase">Vox</span>
      </div>

      <div className="flex items-center h-full">
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
