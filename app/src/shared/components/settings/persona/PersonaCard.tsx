import { useState, memo, useCallback, useMemo, useRef, useEffect } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { CircleUserRound, Pencil, Check } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, SegmentedControl, Markdown } from "@/shared/ui";
import { Tooltip } from "@/shared/ui/Tooltip";
import { PERSONA_COPY } from "@/data/settingsCopy";

interface PersonaCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

const INSTRUCTION_TABS = [
  { id: "modular" as const, label: PERSONA_COPY.tabModular, compactLabel: "M", title: PERSONA_COPY.tabModular },
  { id: "realtime" as const, label: PERSONA_COPY.tabRealtime, compactLabel: "R", title: PERSONA_COPY.tabRealtime },
];

/**
 * Standardises modular and realtime prompts: extracts user persona text from XML wrapper if present.
 */
function extractPersonaText(rawPrompt: string): string {
  if (!rawPrompt) return "";
  const match = rawPrompt.match(/<persona>([\s\S]*?)<\/persona>/i);
  if (match) {
    return match[1].trim();
  }
  return rawPrompt.trim();
}

function updatePersonaInRawPrompt(rawPrompt: string, newPersona: string): string {
  if (/<persona>[\s\S]*?<\/persona>/i.test(rawPrompt)) {
    return rawPrompt.replace(/<persona>[\s\S]*?<\/persona>/i, `<persona>\n${newPersona.trim()}\n</persona>`);
  }
  return newPersona.trim();
}

/**
 * Highlights prompt syntax (XML tags, template variables, comments/headers, bullet dashes, brackets)
 */
function renderSyntaxHighlightedText(code: string) {
  if (!code) return "";

  // Split into lines to preserve structure
  const lines = code.split("\n");

  return lines.map((line, lineIdx) => {
    // Check for comment/section marker
    if (line.trim().startsWith("#") || line.trim().startsWith("//")) {
      return (
        <div key={lineIdx} className="text-[rgb(var(--foreground-muted))]/50 font-mono italic">
          {line || "\u00A0"}
        </div>
      );
    }

    // Tokenize line for XML tags, template variables, brackets, bullet points
    const tokenRegex = /(<\/?[a-zA-Z0-9_-]+>)|(<[a-zA-Z0-9_-]+>)|(\[[^\]]+\])|(^[ \t]*[-*][ \t]+)|(`[^`]+`)/g;
    const parts = [];
    let lastIndex = 0;
    let match: RegExpExecArray | null;

    while ((match = tokenRegex.exec(line)) !== null) {
      if (match.index > lastIndex) {
        parts.push(
          <span key={`${lastIndex}-text`} className="text-[rgb(var(--foreground))]/80">
            {line.substring(lastIndex, match.index)}
          </span>
        );
      }

      const matchStr = match[0];
      if (matchStr.startsWith("</") || (matchStr.startsWith("<") && matchStr.endsWith(">") && !matchStr.startsWith("<lang") && !matchStr.startsWith("<script"))) {
        // XML tags like <persona>, </persona>, <guidelines>, <internal_rules>
        parts.push(
          <span key={`${match.index}-tag`} className="text-[rgb(var(--accent))] font-bold">
            {matchStr}
          </span>
        );
      } else if (matchStr === "<lang>" || matchStr === "<script>") {
        // Special template variables
        parts.push(
          <span key={`${match.index}-var`} className="text-amber-400 font-semibold bg-amber-400/10 px-1 py-0.2 rounded border border-amber-400/20">
            {matchStr}
          </span>
        );
      } else if (matchStr.startsWith("[") && matchStr.endsWith("]")) {
        // [Bracket Headers]
        parts.push(
          <span key={`${match.index}-bracket`} className="text-sky-400 font-semibold">
            {matchStr}
          </span>
        );
      } else if (match[4]) {
        // Bullet markers
        parts.push(
          <span key={`${match.index}-bullet`} className="text-[rgb(var(--accent))]/70 font-black">
            {matchStr}
          </span>
        );
      } else {
        parts.push(
          <span key={`${match.index}-code`} className="text-teal-300 font-mono">
            {matchStr}
          </span>
        );
      }

      lastIndex = tokenRegex.lastIndex;
    }

    if (lastIndex < line.length) {
      parts.push(
        <span key={`${lastIndex}-end`} className="text-[rgb(var(--foreground))]/80">
          {line.substring(lastIndex)}
        </span>
      );
    }

    return (
      <div key={lineIdx} className="min-h-[1.5em] leading-relaxed">
        {parts.length > 0 ? parts : "\u00A0"}
      </div>
    );
  });
}




/**
 * Identifies all protected XML tag spans in the text [start, end)
 */
interface TagSpan {
  start: number;
  end: number;
  tag: string;
}

function getProtectedXmlTagSpans(text: string): TagSpan[] {
  const spans: TagSpan[] = [];
  const tagRegex = /<\/?[a-zA-Z0-9_-]+>|<[a-zA-Z0-9_-]+>/g;
  let match: RegExpExecArray | null;
  while ((match = tagRegex.exec(text)) !== null) {
    spans.push({
      start: match.index,
      end: match.index + match[0].length,
      tag: match[0],
    });
  }
  return spans;
}

/**
 * Checks if a selection range overlaps or directly borders any protected XML tag
 */
function isTouchingProtectedTag(spans: TagSpan[], start: number, end: number, isBackspace = false, isDelete = false): boolean {
  for (const span of spans) {
    // Exact containment or overlap
    if (start < span.end && end > span.start) {
      return true;
    }
    // Caret right at the right edge and backspacing into the tag
    if (isBackspace && start === span.end && end === span.end) {
      return true;
    }
    // Caret right at the left edge and deleting into the tag
    if (isDelete && start === span.start && end === span.start) {
      return true;
    }
  }
  return false;
}

export const PersonaCard = memo(({ layoutMode = "full-max" }: PersonaCardProps) => {
  const modularPrompt = useSettingsStore((s) => s.draftSettings?.persona.modular_prompt ?? "");
  const realtimePrompt = useSettingsStore((s) => s.draftSettings?.persona.realtime_prompt ?? "");
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const [activeTab, setActiveTab] = useState<"modular" | "realtime">("modular");
  const [viewMode, setViewMode] = useState<"edit" | "preview">("preview");

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const highlightRef = useRef<HTMLDivElement>(null);

  // Standardise modular and realtime: extract user-facing persona text
  const activeRawPrompt = activeTab === "modular" ? modularPrompt : realtimePrompt;
  const activePrompt = useMemo(() => extractPersonaText(activeRawPrompt), [activeRawPrompt]);

  // Local keystroke draft
  const [promptDraft, setPromptDraft] = useState<string | null>(null);
  const promptDraftRef = useRef<string | null>(null);
  const commitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [tagViolation, setTagViolation] = useState(false);

  useEffect(() => {
    return () => {
      if (commitTimerRef.current) clearTimeout(commitTimerRef.current);
    };
  }, []);

  // Diagnostics log requested for dark mode rendering artifact inspection
  useEffect(() => {
    const isDark = document.documentElement.classList.contains("dark");
    const cardEl = document.getElementById("card-persona") || textareaRef.current?.parentElement;
    const computed = cardEl ? window.getComputedStyle(cardEl) : null;
    console.info("[PersonaCard Diagnostics]", {
      isDark,
      htmlClasses: document.documentElement.className,
      layoutMode,
      activeTab,
      viewMode,
      cardDimensions: cardEl ? { width: cardEl.offsetWidth, height: cardEl.offsetHeight } : null,
      computedBorder: computed?.border,
      computedBorderColor: computed?.borderColor,
      computedOutline: computed?.outline,
      computedBoxShadow: computed?.boxShadow,
      computedBg: computed?.backgroundColor,
      timestamp: Date.now(),
    });
  }, [layoutMode, activeTab, viewMode]);

  const shownPrompt = promptDraft ?? activePrompt;
  const tagSpans = useMemo(() => getProtectedXmlTagSpans(shownPrompt), [shownPrompt]);
  const highlightedContent = useMemo(() => renderSyntaxHighlightedText(shownPrompt), [shownPrompt]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      const target = e.currentTarget;
      const start = target.selectionStart;
      const end = target.selectionEnd;

      const isBackspace = e.key === "Backspace";
      const isDelete = e.key === "Delete";

      if (isTouchingProtectedTag(tagSpans, start, end, isBackspace, isDelete)) {
        e.preventDefault();
        return;
      }
    },
    [tagSpans]
  );

  const handleBeforeInput = useCallback(
    (e: React.FormEvent<HTMLTextAreaElement>) => {
      const nativeEvent = e.nativeEvent as InputEvent;
      const target = e.currentTarget;
      const start = target.selectionStart;
      const end = target.selectionEnd;

      if (isTouchingProtectedTag(tagSpans, start, end)) {
        nativeEvent.preventDefault?.();
        e.preventDefault();
      }
    },
    [tagSpans]
  );

  const commitDraft = useCallback(
    (nextValue: string) => {
      setTagViolation(false);
      setPromptDraft(null);
      promptDraftRef.current = null;
      if (activeTab === "modular") {
        updateDraft("persona", "modular_prompt", nextValue);
      } else {
        const nextRaw = updatePersonaInRawPrompt(realtimePrompt, nextValue);
        updateDraft("persona", "realtime_prompt", nextRaw);
      }
    },
    [activeTab, realtimePrompt, updateDraft]
  );

  const handlePromptChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const nextValue = e.target.value;
      setPromptDraft(nextValue);
      promptDraftRef.current = nextValue;
      if (commitTimerRef.current) clearTimeout(commitTimerRef.current);
      commitTimerRef.current = setTimeout(() => commitDraft(nextValue), 200);
    },
    [commitDraft]
  );

  const handlePromptBlur = useCallback(() => {
    if (commitTimerRef.current) {
      clearTimeout(commitTimerRef.current);
      commitTimerRef.current = null;
    }
    const pending = promptDraftRef.current;
    if (pending !== null) {
      commitDraft(pending);
    }
  }, [commitDraft]);

  const handleTabChange = useCallback(
    (tab: "modular" | "realtime") => {
      if (commitTimerRef.current) {
        clearTimeout(commitTimerRef.current);
        commitTimerRef.current = null;
      }
      const pending = promptDraftRef.current;
      promptDraftRef.current = null;
      setPromptDraft(null);
      setTagViolation(false);
      if (pending !== null) {
        if (activeTab === "modular") {
          updateDraft("persona", "modular_prompt", pending);
        } else {
          const nextRaw = updatePersonaInRawPrompt(realtimePrompt, pending);
          updateDraft("persona", "realtime_prompt", nextRaw);
        }
      }
      setActiveTab(tab);
    },
    [activeTab, realtimePrompt, updateDraft]
  );

  const handleScroll = useCallback(() => {
    if (textareaRef.current && highlightRef.current) {
      highlightRef.current.scrollTop = textareaRef.current.scrollTop;
      highlightRef.current.scrollLeft = textareaRef.current.scrollLeft;
    }
  }, []);

  const isSmall = layoutMode === "small";

  return (
    <Card 
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "@container text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 transform-gpu flex flex-col",
        !isSmall && cn(
          "p-4 sm:p-5",
          layoutMode === "full-min" ? "lg:w-[350px] xl:w-[370px] 2xl:w-[440px]" : "lg:w-[470px]"
        )
      )}
    >
      {/* Header & Controls: Clean title on left, Modular | Realtime pill on right */}
      <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full gap-2 flex-nowrap">
        <div className="flex items-center gap-2 shrink-0">
          <CircleUserRound className="text-[rgb(var(--accent))]" size={17} />
          <span className="font-display text-[13px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
            {PERSONA_COPY.cardTitle}
          </span>
        </div>
        
        <div className="flex items-center gap-1.5 shrink-0">
          <SegmentedControl options={INSTRUCTION_TABS} value={activeTab} onChange={handleTabChange} size="sm" />
        </div>
      </div>

      {/* Main Body: Default Preview with Floating Pencil Icon Toggle in Corner */}
      <div className="flex-1 flex flex-col w-full relative">
        {/* Floating Pencil / Done Toggle Button in Corner */}
        <div className="absolute top-2 right-2 z-20">
          <Tooltip label={viewMode === "edit" ? "Done editing" : "Edit persona"}>
            <button
              type="button"
              onClick={() => setViewMode((m) => (m === "preview" ? "edit" : "preview"))}
              className={cn(
                "p-1.5 rounded-lg transition-all duration-200 cursor-pointer flex items-center justify-center",
                viewMode === "edit"
                  ? "bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.25)] border border-[rgba(var(--accent),0.3)] shadow-[0_0_8px_rgba(var(--accent),0.15)]"
                  : "text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--foreground),0.05)] border border-transparent"
              )}
              aria-label={viewMode === "edit" ? "Done editing" : "Edit persona"}
            >
              {viewMode === "edit" ? <Check size={13} strokeWidth={2.5} /> : <Pencil size={13} />}
            </button>
          </Tooltip>
        </div>

        {viewMode === "edit" ? (
          <>
            <div className="relative w-full rounded-xl overflow-hidden border border-[rgba(var(--accent),0.12)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)] transition-colors">
              {/* Syntax Highlight Backdrop Layer */}
              <div
                ref={highlightRef}
                aria-hidden="true"
                className={cn(
                  "absolute inset-0 p-3 pr-9 pointer-events-none overflow-auto font-mono text-[12px] sm:text-[12.5px] leading-relaxed whitespace-pre-wrap break-words select-none",
                  layoutMode === "full-max" ? "h-[200px]" : isSmall ? "h-[200px]" : "h-[150px]"
                )}
              >
                {highlightedContent}
              </div>

              {/* Foreground Editable Transparent Textarea */}
              <textarea
                ref={textareaRef}
                value={promptDraft ?? activePrompt}
                onChange={handlePromptChange}
                onBlur={handlePromptBlur}
                onKeyDown={handleKeyDown}
                onBeforeInput={handleBeforeInput}
                onScroll={handleScroll}
                placeholder={activeTab === "modular" ? PERSONA_COPY.modularPlaceholder : PERSONA_COPY.realtimePlaceholder}
                spellCheck={false}
                className={cn(
                  "relative z-10 w-full p-3 pr-9 font-mono text-[12px] sm:text-[12.5px] leading-relaxed text-transparent caret-[rgb(var(--accent))] bg-transparent resize-none focus:outline-none overflow-auto whitespace-pre-wrap break-words selection:bg-[rgba(var(--accent),0.25)] selection:text-[rgb(var(--foreground))]",
                  layoutMode === "full-max" ? "h-[200px]" : isSmall ? "h-[200px]" : "h-[150px]"
                )}
              />
            </div>
            {tagViolation && (
              <p role="alert" className="text-[11px] font-mono text-[rgb(var(--warning))] mt-1.5">
                {PERSONA_COPY.tagsProtectedHint}
              </p>
            )}
          </>
        ) : (
          /* Preview Mode: Clean persona text rendered beautifully */
          <div 
            className={cn(
              "w-full bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.12)] rounded-xl p-3 pr-9 overflow-y-auto select-text scrollbar-thin scrollbar-thumb-[rgba(var(--accent),0.2)] scrollbar-track-transparent",
              layoutMode === "full-max" ? "h-[200px]" : isSmall ? "h-[200px]" : "h-[150px]"
            )}
          >
            {!shownPrompt.trim() ? (
              <div className="flex items-center justify-center h-full text-[12px] text-[rgb(var(--foreground-muted))]/50 italic">
                {PERSONA_COPY.emptyPrompt}
              </div>
            ) : (
              <Markdown
                content={shownPrompt}
                variant="preview"
                className="text-[12px] sm:text-[12.5px] text-[rgb(var(--foreground))]/85 leading-relaxed font-medium"
              />
            )}
          </div>
        )}
      </div>
    </Card>
  );
});

PersonaCard.displayName = "PersonaCard";

