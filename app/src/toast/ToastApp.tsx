import React, { useEffect, useState, useRef, useCallback } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { CheckCircle2, AlertTriangle, AlertCircle, Info, X } from "lucide-react";
import { onShowToast, type ToastPayload } from "@/services/eventsService";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const LEVEL = {
  success: { icon: CheckCircle2, color: "rgb(var(--accent))", bg: "rgba(var(--accent),0.10)", border: "rgba(var(--accent),0.16)" },
  warning: { icon: AlertTriangle, color: "rgb(var(--warning))", bg: "rgba(var(--warning),0.12)", border: "rgba(var(--warning),0.18)" },
  error: { icon: AlertCircle, color: "rgb(var(--error))", bg: "rgba(var(--error),0.10)", border: "rgba(var(--error),0.16)" },
  info: { icon: Info, color: "rgb(var(--foreground-muted))", bg: "rgba(var(--foreground),0.06)", border: "rgba(var(--border),0.08)" },
} as const;

const DEFAULT_DURATION_MS = 3400;

export const ToastApp: React.FC = () => {
  const [toast, setToast] = useState<ToastPayload | null>(null);
  const [visible, setVisible] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const destroyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const progressRef = useRef<number | null>(null);
  const [progress, setProgress] = useState(1);

  const hide = useCallback(async () => {
    setVisible(false);
    if (progressRef.current) cancelAnimationFrame(progressRef.current);
    // animate out upwards, then hide webview once animation completes
    if (destroyTimerRef.current) clearTimeout(destroyTimerRef.current);
    destroyTimerRef.current = setTimeout(async () => {
      try {
        await getCurrentWindow().hide();
      } catch {
        try { await invoke("hide_toast_window"); } catch {}
      }
      setTimeout(async () => {
        try { await invoke("destroy_toast_window_cmd"); } catch {}
        setToast(null);
        setProgress(1);
      }, 280);
    }, 320);
  }, []);

  const show = useCallback(async (payload: ToastPayload) => {
    if (timerRef.current) clearTimeout(timerRef.current);
    if (destroyTimerRef.current) clearTimeout(destroyTimerRef.current);
    if (progressRef.current) cancelAnimationFrame(progressRef.current);

    // Present the hidden transparent webview now that we have content to paint.
    // Frontend owns the first show to avoid the black flash; backend has a
    // fallback show as well.
    let shown = false;
    try { await getCurrentWindow().show(); shown = true; } catch (e) { console.warn("[ToastApp] getCurrentWindow().show failed", e); }
    if (!shown) {
      try { await invoke("show_toast_window"); shown = true; } catch (e) { console.warn("[ToastApp] show_toast_window invoke failed", e); }
    }
    await new Promise<void>((r) => requestAnimationFrame(() => r()));

    console.log("[ToastApp] showing", payload);
    setToast(payload);
    setVisible(true);
    setProgress(1);

    const duration = payload.duration_ms ?? DEFAULT_DURATION_MS;
    const start = performance.now();
    const tick = (now: number) => {
      const p = Math.max(0, 1 - (now - start) / duration);
      setProgress(p);
      if (p > 0) progressRef.current = requestAnimationFrame(tick);
    };
    progressRef.current = requestAnimationFrame(tick);
    timerRef.current = setTimeout(() => hide(), duration);
  }, [hide]);

  useEffect(() => {
    let cancelled = false;
    const unlisten = onShowToast((payload) => { if (!cancelled) show(payload); });
    const fetchPending = async () => {
      try {
        const pending = await invoke<ToastPayload | null>("get_last_toast");
        if (pending && !cancelled) show(pending);
      } catch {}
    };
    // allow backend delayed emit to land, then fall back to polling the stored payload
    const t = setTimeout(fetchPending, 700);
    return () => {
      cancelled = true;
      clearTimeout(t);
      unlisten();
      if (timerRef.current) clearTimeout(timerRef.current);
      if (destroyTimerRef.current) clearTimeout(destroyTimerRef.current);
      if (progressRef.current) cancelAnimationFrame(progressRef.current);
    };
  }, [show]);

  const cfg = toast ? (LEVEL[toast.level as keyof typeof LEVEL] ?? LEVEL.info) : LEVEL.info;
  const Icon = cfg.icon;

  return (
    <div className="w-screen h-screen flex items-start justify-center bg-transparent pointer-events-none overflow-hidden select-none p-6">
      <AnimatePresence>
        {visible && toast && (
          <motion.div
            key={`${toast.title}-${toast.message}-${toast.level}`}
            initial={{ opacity: 0, y: -12 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -12 }}
            transition={{ duration: 0.36, ease: [0.16, 1, 0.3, 1] }}
            className="pointer-events-auto relative w-[360px] overflow-hidden rounded-xl border glass-card"
            style={{
              // glass-card already sets bg/border/blur; keep margin via outer p-6
              boxShadow: "0 10px 28px -14px rgba(0,0,0,0.55), inset 0 1px 0 rgba(255,255,255,0.05)",
            }}
          >
            <div className="flex items-start gap-3 px-4 py-3.5">
              <div
                className="shrink-0 w-7 h-7 rounded-full flex items-center justify-center mt-0.5"
                style={{ background: cfg.bg, border: `1px solid ${cfg.border}` }}
              >
                <Icon size={14} style={{ color: cfg.color }} strokeWidth={1.7} />
              </div>

              <div className="flex-1 min-w-0 flex flex-col gap-0.5">
                <span className="font-sans text-[13px] font-semibold leading-none tracking-[-0.01em] text-[rgb(var(--foreground))]">
                  {toast.title}
                </span>
                <p className="font-sans text-[12px] leading-[1.5] text-[rgb(var(--foreground-muted))] line-clamp-2 break-words">
                  {toast.message}
                </p>
              </div>

              <button
                onClick={hide}
                aria-label="Dismiss"
                className="shrink-0 -mr-1 w-6 h-6 rounded-full flex items-center justify-center text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))]/80 hover:bg-[rgba(var(--foreground),0.06)] transition-colors"
              >
                <X size={12} strokeWidth={1.8} />
              </button>
            </div>

            <div className="relative h-px w-full overflow-hidden bg-[rgba(var(--border),0.06)]">
              <motion.div
                className="absolute inset-y-0 left-0 origin-left"
                style={{ background: cfg.color, opacity: 0.45, width: "100%", transformOrigin: "left center" }}
                animate={{ scaleX: progress }}
                transition={{ duration: 0.06, ease: "linear" }}
                initial={{ scaleX: 1 }}
              />
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
