import React from "react";
import { CheckCircle2, AlertTriangle, Sparkles, HelpCircle } from "lucide-react";
import type { AccuracyLevel } from "@/services/memoryProfilerService";

export const AccuracyBadge: React.FC<{ type: AccuracyLevel }> = ({ type }) => {
  switch (type) {
    case "Measured":
      return (
        <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-mono uppercase bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/30">
          <CheckCircle2 size={11} /> Measured
        </span>
      );
    case "Estimated":
      return (
        <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-mono uppercase bg-[rgb(var(--accent))]/10 text-[rgb(var(--foreground))] border border-[rgba(var(--border),0.3)]">
          <AlertTriangle size={11} /> Estimated
        </span>
      );
    case "Correlated":
      return (
        <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-mono uppercase bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.25)]">
          <Sparkles size={11} /> Correlated
        </span>
      );
    default:
      return (
        <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-mono uppercase bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--border),0.2)]">
          <HelpCircle size={11} /> Unattributed
        </span>
      );
  }
};
