import React, { useState, useEffect } from "react";
import {
  Upload,
  Edit3,
  Check,
  X,
  Sparkles,
  ArrowLeft,
  Cpu,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { PixelSynthesisCanvas } from "./PixelSynthesisCanvas";

export type StagingMode = "idle" | "import" | "edit" | "consolidating";

export interface PersonalMemoryStagingCardProps {
  canonicalContent: string | null;
  mode: StagingMode;
  onModeChange: (mode: StagingMode) => void;
  onSave: (content: string) => Promise<void>;
  unconsolidatedCount: number;
  isSaving: boolean;
  isCommitting: boolean;
}

export const PersonalMemoryStagingCard: React.FC<PersonalMemoryStagingCardProps> = ({
  canonicalContent,
  mode,
  onModeChange,
  onSave,
  unconsolidatedCount,
  isSaving,
  isCommitting,
}) => {
  const [draft, setDraft] = useState("");
  const [tickerIndex, setTickerIndex] = useState(0);

  // Sync draft with canonical content whenever switching to edit mode
  useEffect(() => {
    if (mode === "edit") {
      setDraft(canonicalContent || "");
    } else if (mode === "import") {
      setDraft("");
    }
  }, [mode, canonicalContent]);

  // Synthesis status ticker cycle
  useEffect(() => {
    if (mode !== "consolidating") return;
    const phrases = [
      `Distilling ${unconsolidatedCount || 1} identity ${unconsolidatedCount === 1 ? "fact" : "facts"}...`,
      "Synthesizing personal directives & habits...",
      "Resolving semantic contradictions...",
      "Structuring unified markdown dossier...",
    ];
    const interval = setInterval(() => {
      setTickerIndex((prev) => (prev + 1) % phrases.length);
    }, 2400);
    return () => clearInterval(interval);
  }, [mode, unconsolidatedCount]);

  const handleCommit = async () => {
    if (!draft.trim()) return;
    await onSave(draft);
  };

  return (
    <div
      className={cn(
        "relative w-full h-full min-h-0 rounded-2xl p-5 sm:p-6 flex flex-col transition-all duration-500 overflow-hidden",
        "glass-card border bg-[rgba(var(--card),0.45)] backdrop-blur-xl",
        mode === "consolidating"
          ? "border-[rgba(var(--accent),0.4)] shadow-[0_0_30px_rgba(var(--accent),0.12)]"
          : mode === "edit" || mode === "import"
          ? "border-[rgba(var(--accent),0.25)] shadow-lg"
          : "border-dashed border-[rgba(var(--border),0.22)] hover:border-[rgba(var(--accent),0.3)]"
      )}
    >
      {/* Dynamic Header Bar */}
      <div className="flex items-center justify-between gap-4 border-b border-[rgba(var(--border),0.12)] pb-3.5 min-h-[44px] shrink-0">
        <div className="flex items-center gap-3">
          <div
            className={cn(
              "w-8 h-8 rounded-xl border flex items-center justify-center transition-colors shadow-sm",
              mode === "consolidating"
                ? "bg-[rgba(var(--accent),0.2)] border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))] animate-pulse"
                : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.15)] text-[rgb(var(--foreground-muted))]"
            )}
          >
            {mode === "consolidating" ? (
              <Cpu size={16} className="animate-spin text-[rgb(var(--accent))]" />
            ) : mode === "edit" ? (
              <Edit3 size={16} className="text-[rgb(var(--accent))]" />
            ) : mode === "import" ? (
              <Upload size={16} className="text-[rgb(var(--accent))]" />
            ) : (
              <Sparkles size={16} />
            )}
          </div>

          <div className="flex flex-col">
            <span className="text-[13px] font-semibold tracking-wide text-[rgb(var(--foreground))]">
              {mode === "consolidating"
                ? "Computational Synthesis"
                : mode === "edit"
                ? "Direct In-Place Edit"
                : mode === "import"
                ? "Import / Paste Memory"
                : "Staging Mirror"}
            </span>
            <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
              {mode === "consolidating"
                ? "Autonomous LLM Reconstruction"
                : mode === "edit"
                ? "Changes flow directly to persistent memory"
                : mode === "import"
                ? "Paste markdown to replace current profile"
                : "Interactive draft workspace"}
            </span>
          </div>
        </div>

        {/* Action controls in header */}
        <div className="flex items-center gap-2">
          {isCommitting ? (
            <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.14)] border border-[rgba(var(--accent),0.3)] text-[rgb(var(--accent))] shadow-sm transition-all duration-300">
              <Check size={12} className="text-[rgb(var(--accent))]" />
              <span>Saved</span>
            </div>
          ) : mode === "edit" || mode === "import" ? (
            <>
              <button
                type="button"
                onClick={handleCommit}
                disabled={isSaving || !draft.trim()}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] transition-colors disabled:opacity-40 cursor-pointer shadow-sm"
              >
                <ArrowLeft size={13} className={cn(isSaving && "animate-pulse")} />
                {isSaving ? "Saving…" : "Save & Commit"}
              </button>
              <button
                type="button"
                onClick={() => onModeChange("idle")}
                disabled={isSaving}
                className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
              >
                <X size={13} /> Cancel
              </button>
            </>
          ) : mode === "idle" ? (
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => onModeChange("import")}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:border-[rgba(var(--accent),0.3)] transition-colors cursor-pointer"
              >
                <Upload size={12} /> Import
              </button>
              <button
                type="button"
                onClick={() => onModeChange("edit")}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.3)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.2)] transition-colors cursor-pointer shadow-sm"
              >
                <Edit3 size={12} /> Edit
              </button>
            </div>
          ) : (
            <span className="px-2.5 py-1 rounded-full text-[10px] font-mono font-medium tracking-wide bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.3)] text-[rgb(var(--accent))] animate-pulse">
              Synthesizing…
            </span>
          )}
        </div>
      </div>

      {/* Mode 1: Idle Skeleton Slate (Single unified card with subtle pulse) */}
      {mode === "idle" && (
        <div className="flex-1 min-h-0 flex flex-col justify-between pt-5 pb-2 overflow-y-auto custom-scrollbar transition-opacity duration-500">
          {/* Wireframe lines mirroring the left dossier */}
          <div className="space-y-6 opacity-40 select-none pointer-events-none pr-2">
            {/* Section 1 */}
            <div className="space-y-2.5">
              <div className="w-1/4 h-3.5 rounded-md bg-[rgba(var(--accent),0.3)] animate-pulse" />
              <div className="w-full h-2.5 rounded bg-[rgba(var(--foreground),0.08)]" />
              <div className="w-11/12 h-2.5 rounded bg-[rgba(var(--foreground),0.08)]" />
              <div className="w-4/5 h-2.5 rounded bg-[rgba(var(--foreground),0.08)]" />
            </div>

            {/* Section 2 */}
            <div className="space-y-2.5">
              <div className="w-1/3 h-3.5 rounded-md bg-[rgba(var(--accent),0.3)] animate-pulse" />
              <div className="w-full h-2.5 rounded bg-[rgba(var(--foreground),0.08)]" />
              <div className="w-5/6 h-2.5 rounded bg-[rgba(var(--foreground),0.08)]" />
              <div className="w-3/4 h-2.5 rounded bg-[rgba(var(--foreground),0.08)]" />
            </div>

            {/* Section 3 */}
            <div className="space-y-2.5">
              <div className="w-2/5 h-3.5 rounded-md bg-[rgba(var(--accent),0.3)] animate-pulse" />
              <div className="w-full h-2.5 rounded bg-[rgba(var(--foreground),0.08)]" />
              <div className="w-4/5 h-2.5 rounded bg-[rgba(var(--foreground),0.08)]" />
            </div>
          </div>

          <div className="shrink-0 text-[10px] font-mono text-[rgb(var(--foreground-muted))]/60 text-center select-none pt-4 border-t border-[rgba(var(--border),0.06)]">
            Click Import or Edit above to modify personal memory
          </div>
        </div>
      )}

      {/* Mode 2: Import Textarea */}
      {mode === "import" && (
        <div
          className={cn(
            "flex-1 min-h-0 flex flex-col gap-2 pt-3 transition-opacity duration-500",
            isCommitting ? "opacity-30 pointer-events-none" : "opacity-100"
          )}
        >
          <textarea
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder="Paste your markdown directives here (e.g. ## Overview, ## Preferences, etc.)..."
            className="w-full flex-1 min-h-0 bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.16)] rounded-xl p-4 text-[13px] font-mono text-[rgb(var(--foreground))] leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.5)] transition-colors custom-scrollbar"
            spellCheck={false}
            autoFocus
          />
          <div className="shrink-0 flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))] pt-1">
            <span>Supports standard markdown syntax.</span>
            <span>{draft.length} characters</span>
          </div>
        </div>
      )}

      {/* Mode 3: In-Place Live Editor */}
      {mode === "edit" && (
        <div
          className={cn(
            "flex-1 min-h-0 flex flex-col gap-2 pt-3 transition-opacity duration-500",
            isCommitting ? "opacity-30 pointer-events-none" : "opacity-100"
          )}
        >
          <textarea
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            className="w-full flex-1 min-h-0 bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.16)] rounded-xl p-4 text-[13px] font-mono text-[rgb(var(--foreground))] leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.5)] transition-colors custom-scrollbar"
            spellCheck={false}
            autoFocus
          />
          <div className="shrink-0 flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))] pt-1">
            <span>Saving streams updates directly to the canonical database.</span>
            <span>{draft.length} characters</span>
          </div>
        </div>
      )}

      {/* Mode 4: Generative Pixel Synthesis */}
      {mode === "consolidating" && (
        <div className="relative flex-1 min-h-0 rounded-xl overflow-hidden bg-[rgba(var(--card),0.9)] border border-[rgba(var(--accent),0.2)] flex flex-col justify-between p-6 mt-3">
          {/* Active Canvas Pixel Field */}
          <div className="absolute inset-0 z-0">
            <PixelSynthesisCanvas active={true} />
          </div>

          {/* Top Status HUD */}
          <div className="relative z-10 flex items-center justify-between">
            <span className="px-2 py-0.5 rounded-md text-[10px] font-mono font-semibold uppercase tracking-wider bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.3)]">
              Neural Reconstruction
            </span>
            <span className="text-[11px] font-mono text-[rgb(var(--accent))] animate-pulse font-medium">
              Active Synthesis
            </span>
          </div>

          {/* Center Dynamic Ticker */}
          <div className="relative z-10 flex flex-col items-center justify-center text-center p-4 bg-[rgba(var(--card),0.75)] backdrop-blur-md rounded-xl border border-[rgba(var(--accent),0.25)] max-w-sm mx-auto shadow-2xl">
            <Sparkles size={20} className="text-[rgb(var(--accent))] animate-pulse mb-2" />
            <span className="text-[12.5px] font-mono font-medium text-[rgb(var(--foreground))] transition-all duration-300">
              {[
                `Distilling ${unconsolidatedCount || 1} identity ${unconsolidatedCount === 1 ? "fact" : "facts"}...`,
                "Synthesizing personal directives & habits...",
                "Resolving semantic contradictions...",
                "Structuring unified markdown dossier...",
              ][tickerIndex]}
            </span>
            <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))] mt-1">
              Cross-referencing conversation history in vox.db
            </span>
          </div>

          {/* Bottom Footer Info */}
          <div className="relative z-10 flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
            <span>LLM Cognitive Pipeline</span>
            <span>Will commit to left dossier upon completion</span>
          </div>
        </div>
      )}
    </div>
  );
};
