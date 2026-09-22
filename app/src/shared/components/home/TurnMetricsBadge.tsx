import React, { memo, useMemo } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Zap } from "lucide-react";
import { useSessionStore } from "@/store/sessionStore";
import { Tooltip } from "@/shared/ui/Tooltip";
import { METRICS_COPY } from "@/data/metricsCopy";
import { cn } from "@/shared/lib/utils";

function formatMs(ms: number): string {
  if (ms <= 0) return "—";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function formatTokens(count: number): string {
  if (count <= 0) return "0";
  if (count >= 1000) {
    return `${(count / 1000).toFixed(1)}k`;
  }
  return `${count}`;
}

export const TurnMetricsBadge: React.FC<{ className?: string }> = memo(({ className }) => {
  const interactionState = useSessionStore((s) => s.interactionState);
  const latestMetrics = useSessionStore((s) => s.latestTurnMetrics);

  const isEngaged = interactionState !== "Idle";

  const stats = useMemo(() => {
    if (!isEngaged) return null;

    if (!latestMetrics) {
      return {
        turnId: 0,
        ttft: "—",
        rawTtftMs: 0,
        ttfa: "—",
        rawTtfaMs: 0,
        voice: "—",
        rawVoiceMs: 0,
        ctxUsed: "0",
        ctxMax: "—",
        rawCtxUsed: 0,
        rawCtxMax: 0,
        ctxPct: 0,
        isAwaiting: true,
      };
    }

    const ctxUsed = latestMetrics.context_tokens_used;
    const ctxMax = latestMetrics.context_window;
    const ctxPct = ctxMax > 0 ? Math.round((ctxUsed / ctxMax) * 100) : 0;

    return {
      turnId: latestMetrics.turn_id,
      ttft: formatMs(latestMetrics.ttft_ms),
      rawTtftMs: latestMetrics.ttft_ms,
      ttfa: formatMs(latestMetrics.ttfa_ms),
      rawTtfaMs: latestMetrics.ttfa_ms,
      voice: formatMs(latestMetrics.total_voice_latency_ms),
      rawVoiceMs: latestMetrics.total_voice_latency_ms,
      ctxUsed: formatTokens(ctxUsed),
      ctxMax: formatTokens(ctxMax),
      rawCtxUsed: ctxUsed,
      rawCtxMax: ctxMax,
      ctxPct,
      isAwaiting: false,
    };
  }, [isEngaged, latestMetrics]);

  if (!stats) return null;

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0, y: 4 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: 4 }}
        transition={{ duration: 0.2, ease: "easeOut" }}
        className={cn(
          "hidden lg:inline-flex items-center gap-2.5 px-2 py-1 select-none text-[14px] font-mono leading-none tabular-nums text-[rgb(var(--foreground))]",
          className
        )}
        role="status"
        aria-label={METRICS_COPY.badgeAriaLabel}
      >
        <div className="flex items-center text-[rgb(var(--accent))] shrink-0">
          <Zap size={14} strokeWidth={2.2} />
        </div>

        {/* TTFT */}
        <Tooltip
          side="top"
          label={
            <span>
              {METRICS_COPY.ttftTooltip}:{" "}
              <strong>{stats.isAwaiting ? METRICS_COPY.awaitingTurn : `${stats.rawTtftMs}ms`}</strong>
            </span>
          }
        >
          <div className="cursor-default flex items-center gap-1.5 hover:opacity-80 transition-opacity">
            <span className="text-[rgb(var(--foreground-muted))] font-normal">{METRICS_COPY.ttft}</span>
            <span className="text-[rgb(var(--foreground))] font-semibold">{stats.ttft}</span>
          </div>
        </Tooltip>

        <span className="text-[rgb(var(--foreground-muted))]/40">·</span>

        {/* TTFA */}
        <Tooltip
          side="top"
          label={
            <span>
              {METRICS_COPY.ttfaTooltip}:{" "}
              <strong>{stats.isAwaiting ? METRICS_COPY.awaitingTurn : `${stats.rawTtfaMs}ms`}</strong>
            </span>
          }
        >
          <div className="cursor-default flex items-center gap-1.5 hover:opacity-80 transition-opacity">
            <span className="text-[rgb(var(--foreground-muted))] font-normal">{METRICS_COPY.ttfa}</span>
            <span className="text-[rgb(var(--foreground))] font-semibold">{stats.ttfa}</span>
          </div>
        </Tooltip>

        <span className="text-[rgb(var(--foreground-muted))]/40">·</span>

        {/* Voice Latency */}
        <Tooltip
          side="top"
          label={
            <span>
              {METRICS_COPY.voiceTooltip}:{" "}
              <strong>{stats.isAwaiting ? METRICS_COPY.awaitingTurn : `${stats.rawVoiceMs}ms`}</strong>
            </span>
          }
        >
          <div className="cursor-default flex items-center gap-1.5 hover:opacity-80 transition-opacity">
            <span className="text-[rgb(var(--foreground-muted))] font-normal">{METRICS_COPY.voice}</span>
            <span className="text-[rgb(var(--accent))] font-semibold">{stats.voice}</span>
          </div>
        </Tooltip>

        <span className="text-[rgb(var(--foreground-muted))]/40">·</span>

        {/* CTX Window Utilization */}
        <Tooltip
          side="top"
          label={
            <span>
              {METRICS_COPY.ctxTooltip}:{" "}
              <strong>
                {stats.isAwaiting
                  ? METRICS_COPY.awaitingTurn
                  : `${stats.rawCtxUsed.toLocaleString()} / ${stats.rawCtxMax.toLocaleString()} ${METRICS_COPY.tokensUnit} (${stats.ctxPct}%)`}
              </strong>
            </span>
          }
        >
          <div className="cursor-default flex items-center gap-1.5 hover:opacity-80 transition-opacity">
            <span className="text-[rgb(var(--foreground-muted))] font-normal">{METRICS_COPY.ctx}</span>
            <span
              className={cn(
                "font-semibold",
                stats.ctxPct >= 85
                  ? "text-red-400"
                  : stats.ctxPct >= 65
                  ? "text-amber-400"
                  : "text-[rgb(var(--foreground))]"
              )}
            >
              {stats.ctxPct}%
            </span>
            {!stats.isAwaiting && (
              <span className="text-[13px] text-[rgb(var(--foreground-muted))] font-normal">
                ({stats.ctxUsed}/{stats.ctxMax})
              </span>
            )}
          </div>
        </Tooltip>
      </motion.div>
    </AnimatePresence>
  );
});

TurnMetricsBadge.displayName = "TurnMetricsBadge";
