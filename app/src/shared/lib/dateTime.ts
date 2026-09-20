import { SESSION_COPY } from "@/data/sessionCopy";

/**
 * Pure date/time and recency formatting utilities.
 * Completely free of backend IPC or service dependencies to protect chunk splitting.
 */

export function formatSessionRecency(timestampMs: number, nowMs: number = Date.now()): string {
  const diffMs = Math.max(0, nowMs - timestampMs);
  const minutes = Math.floor(diffMs / 60000);
  if (minutes < 1) return SESSION_COPY.recency.justNow;
  if (minutes < 60) return SESSION_COPY.recency.minutesAgo.replace("{n}", String(minutes));
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return SESSION_COPY.recency.hoursAgo.replace("{n}", String(hours));
  if (diffMs < 48 * 3600000) return SESSION_COPY.recency.yesterday;
  return new Date(timestampMs).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

export function formatDateShort(ms: number): string {
  return new Date(ms).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

export function formatDateTime(ms: number): string {
  return new Date(ms).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}
