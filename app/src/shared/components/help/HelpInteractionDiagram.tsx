import { memo, useState } from "react";
import { Mic, Radio, Keyboard, VolumeX, Sparkles, ArrowRight, Check } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export const HelpInteractionDiagram = memo(() => {
  const [activeTab, setActiveTab] = useState<"continuous" | "ptt" | "dictation">("continuous");

  return (
    <div className="rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--card),0.7)] p-4 flex flex-col gap-3 backdrop-blur-md">
      <div className="flex items-center justify-between">
        <span className="text-[12px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-2">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Speaking Modes Compared
        </span>
        <span className="text-[11px] text-[rgb(var(--foreground-muted))]">
          Click a mode to preview
        </span>
      </div>

      {/* Mode Selector Tabs */}
      <div className="grid grid-cols-3 gap-1 p-1 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)]">
        <button
          onClick={() => setActiveTab("continuous")}
          className={cn(
            "flex items-center justify-center gap-1.5 py-1.5 px-2 rounded-lg text-[11px] font-medium transition-all cursor-pointer",
            activeTab === "continuous"
              ? "bg-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground))] border border-[rgba(var(--accent),0.3)]"
              : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
          )}
        >
          <Radio size={13} className={activeTab === "continuous" ? "text-[rgb(var(--accent))]" : ""} />
          <span>Hands-Free</span>
        </button>

        <button
          onClick={() => setActiveTab("ptt")}
          className={cn(
            "flex items-center justify-center gap-1.5 py-1.5 px-2 rounded-lg text-[11px] font-medium transition-all cursor-pointer",
            activeTab === "ptt"
              ? "bg-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground))] border border-[rgba(var(--accent),0.3)]"
              : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
          )}
        >
          <Mic size={13} className={activeTab === "ptt" ? "text-[rgb(var(--accent))]" : ""} />
          <span>Push-to-Talk</span>
        </button>

        <button
          onClick={() => setActiveTab("dictation")}
          className={cn(
            "flex items-center justify-center gap-1.5 py-1.5 px-2 rounded-lg text-[11px] font-medium transition-all cursor-pointer",
            activeTab === "dictation"
              ? "bg-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground))] border border-[rgba(var(--accent),0.3)]"
              : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
          )}
        >
          <Keyboard size={13} className={activeTab === "dictation" ? "text-[rgb(var(--accent))]" : ""} />
          <span>Dictation</span>
        </button>
      </div>

      {/* Interactive Flow Box */}
      <div className="relative rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)] p-3 flex flex-col gap-2.5">
        {activeTab === "continuous" && (
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-1 text-center py-1 px-1">
              <div className="flex flex-col items-center gap-1 w-20">
                <div className="w-8 h-8 rounded-full border border-sky-400/30 bg-sky-400/10 flex items-center justify-center text-sky-400">
                  <Radio size={14} className="animate-pulse" />
                </div>
                <span className="text-[10.5px] font-bold text-[rgb(var(--foreground))]">Always Ready</span>
                <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">Listens for voice</span>
              </div>
              <ArrowRight size={13} className="text-[rgb(var(--foreground-muted))]/40 shrink-0" />
              <div className="flex flex-col items-center gap-1 w-20">
                <div className="w-8 h-8 rounded-full border border-cyan-400/30 bg-cyan-400/10 flex items-center justify-center text-cyan-400">
                  <Sparkles size={14} />
                </div>
                <span className="text-[10.5px] font-bold text-[rgb(var(--foreground))]">You Speak</span>
                <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">Auto transcribes</span>
              </div>
              <ArrowRight size={13} className="text-[rgb(var(--foreground-muted))]/40 shrink-0" />
              <div className="flex flex-col items-center gap-1 w-20">
                <div className="w-8 h-8 rounded-full border border-amber-400/30 bg-amber-400/10 flex items-center justify-center text-amber-400">
                  <VolumeX size={14} />
                </div>
                <span className="text-[10.5px] font-bold text-[rgb(var(--foreground))]">You Pause</span>
                <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">Sends reply</span>
              </div>
            </div>
            <p className="text-[11.5px] leading-relaxed text-[rgb(var(--foreground-muted))]">
              Speak freely without holding any buttons. Vox automatically replies when you finish speaking.
            </p>
          </div>
        )}

        {activeTab === "ptt" && (
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-1 text-center py-1 px-1">
              <div className="flex flex-col items-center gap-1 w-20">
                <div className="w-8 h-8 rounded-full border border-amber-400/30 bg-amber-400/10 flex items-center justify-center text-amber-400">
                  <Keyboard size={14} />
                </div>
                <span className="text-[10.5px] font-bold text-[rgb(var(--foreground))]">Hold Space</span>
                <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">Mic opens</span>
              </div>
              <ArrowRight size={13} className="text-[rgb(var(--foreground-muted))]/40 shrink-0" />
              <div className="flex flex-col items-center gap-1 w-20">
                <div className="w-8 h-8 rounded-full border border-cyan-400/30 bg-cyan-400/10 flex items-center justify-center text-cyan-400">
                  <Mic size={14} />
                </div>
                <span className="text-[10.5px] font-bold text-[rgb(var(--foreground))]">Speak</span>
                <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">Record voice</span>
              </div>
              <ArrowRight size={13} className="text-[rgb(var(--foreground-muted))]/40 shrink-0" />
              <div className="flex flex-col items-center gap-1 w-20">
                <div className="w-8 h-8 rounded-full border border-emerald-400/30 bg-emerald-400/10 flex items-center justify-center text-emerald-400">
                  <Check size={14} />
                </div>
                <span className="text-[10.5px] font-bold text-[rgb(var(--foreground))]">Release Key</span>
                <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">Sends reply</span>
              </div>
            </div>
            <p className="text-[11.5px] leading-relaxed text-[rgb(var(--foreground-muted))]">
              Complete control. Only records when Space is held, preventing background noise from triggering unwanted answers.
            </p>
          </div>
        )}

        {activeTab === "dictation" && (
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-1 text-center py-1 px-1">
              <div className="flex flex-col items-center gap-1 w-20">
                <div className="w-8 h-8 rounded-full border border-purple-400/30 bg-purple-400/10 flex items-center justify-center text-purple-400">
                  <Keyboard size={14} />
                </div>
                <span className="text-[10.5px] font-bold text-[rgb(var(--foreground))]">Shortcut</span>
                <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">In any app</span>
              </div>
              <ArrowRight size={13} className="text-[rgb(var(--foreground-muted))]/40 shrink-0" />
              <div className="flex flex-col items-center gap-1 w-20">
                <div className="w-8 h-8 rounded-full border border-cyan-400/30 bg-cyan-400/10 flex items-center justify-center text-cyan-400">
                  <Mic size={14} />
                </div>
                <span className="text-[10.5px] font-bold text-[rgb(var(--foreground))]">Speak</span>
                <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">Speech to text</span>
              </div>
              <ArrowRight size={13} className="text-[rgb(var(--foreground-muted))]/40 shrink-0" />
              <div className="flex flex-col items-center gap-1 w-20">
                <div className="w-8 h-8 rounded-full border border-emerald-400/30 bg-emerald-400/10 flex items-center justify-center text-emerald-400">
                  <Sparkles size={14} />
                </div>
                <span className="text-[10.5px] font-bold text-[rgb(var(--foreground))]">Auto Types</span>
                <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">At active cursor</span>
              </div>
            </div>
            <p className="text-[11.5px] leading-relaxed text-[rgb(var(--foreground-muted))]">
              Dictate text into your code editor, browser, or documents using Vox's voice recognition.
            </p>
          </div>
        )}
      </div>
    </div>
  );
});

HelpInteractionDiagram.displayName = "HelpInteractionDiagram";
