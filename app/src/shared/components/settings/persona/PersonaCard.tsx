import { useState, memo, useCallback, useMemo, useRef, useEffect } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { CircleUserRound, Code2, Eye } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, SegmentedControl, Markdown } from "@/shared/ui";
import { PERSONA_COPY } from "@/data/settingsCopy";

interface PersonaCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

const INSTRUCTION_TABS = [
  { id: "modular" as const, label: PERSONA_COPY.tabModular, compactLabel: "M", title: PERSONA_COPY.tabModular },
  { id: "realtime" as const, label: PERSONA_COPY.tabRealtime, compactLabel: "R", title: PERSONA_COPY.tabRealtime },
];

const VIEW_TABS = [
  { id: "preview" as const, icon: Eye, label: PERSONA_COPY.viewPreview, title: PERSONA_COPY.viewPreview },
  { id: "edit" as const, icon: Code2, label: PERSONA_COPY.viewEdit, title: PERSONA_COPY.viewEdit },
];

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
 * Parses XML blocks into clean structured sections for preview
 */
interface ParsedSection {
  title?: string;
  tag?: string;
  content: string;
}

function parseXmlToSections(rawPrompt: string): ParsedSection[] {
  if (!rawPrompt || !rawPrompt.trim()) return [];

  const sections: ParsedSection[] = [];
  // Match XML blocks: <tag>content</tag>
  const blockRegex = /<([a-zA-Z0-9_-]+)>([\s\S]*?)<\/\1>/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = blockRegex.exec(rawPrompt)) !== null) {
    const beforeText = rawPrompt.substring(lastIndex, match.index).trim();
    if (beforeText) {
      sections.push({
        content: beforeText,
      });
    }

    const tagName = match[1];
    const content = match[2].trim();

    // Format tag name into friendly heading: internal_rules -> Internal Rules
    const friendlyTitle = tagName
      .replace(/_/g, " ")
      .replace(/-/g, " ")
      .replace(/\b\w/g, (c) => c.toUpperCase());

    sections.push({
      title: friendlyTitle,
      tag: tagName,
      content,
    });

    lastIndex = blockRegex.lastIndex;
  }

  const remaining = rawPrompt.substring(lastIndex).trim();
  if (remaining) {
    sections.push({
      content: remaining,
    });
  }

  return sections;
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

  const activePrompt = activeTab === "modular" ? modularPrompt : realtimePrompt;

  // Local keystroke draft: the textarea commits to the store on a 200ms
  // trailing debounce instead of per keystroke. Previously every character
  // rebuilt the full syntax-highlight tree AND updated the store, re-rendering
  // every settings consumer on each keypress.
  const [promptDraft, setPromptDraft] = useState<string | null>(null);
  const promptDraftRef = useRef<string | null>(null);
  const commitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [tagViolation, setTagViolation] = useState(false);
  useEffect(() => {
    return () => {
      if (commitTimerRef.current) clearTimeout(commitTimerRef.current);
    };
  }, []);

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
      // Verify tag integrity against the COMMITTED prompt — if protected tags
      // were modified or stripped, say so visibly instead of silently
      // swallowing the keystroke (the old behaviour, with zero diagnostics).
      const prevTags = getProtectedXmlTagSpans(activePrompt).map((s) => s.tag);
      const nextTags = getProtectedXmlTagSpans(nextValue).map((s) => s.tag);
      if (prevTags.length > 0 && JSON.stringify(prevTags) !== JSON.stringify(nextTags)) {
        setTagViolation(true);
        return;
      }
      setTagViolation(false);
      setPromptDraft(null);
      promptDraftRef.current = null;
      const field = activeTab === "modular" ? "modular_prompt" : "realtime_prompt";
      updateDraft("persona", field, nextValue);
    },
    [activePrompt, activeTab, updateDraft]
  );

  const handlePromptChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const nextValue = e.target.value;
      // Instant, local, no IPC — the store commit happens on pause (below).
      setPromptDraft(nextValue);
      promptDraftRef.current = nextValue;
      if (commitTimerRef.current) clearTimeout(commitTimerRef.current);
      commitTimerRef.current = setTimeout(() => commitDraft(nextValue), 200);
    },
    [commitDraft]
  );

  const handlePromptBlur = useCallback(() => {
    // Flush any pending debounced commit so tabbing away never loses typing.
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
      // Commit (don't drop) any pending draft before switching tabs.
      if (commitTimerRef.current) {
        clearTimeout(commitTimerRef.current);
        commitTimerRef.current = null;
      }
      const pending = promptDraftRef.current;
      promptDraftRef.current = null;
      setPromptDraft(null);
      setTagViolation(false);
      if (pending !== null) {
        const prevTags = getProtectedXmlTagSpans(activePrompt).map((s) => s.tag);
        const nextTags = getProtectedXmlTagSpans(pending).map((s) => s.tag);
        if (prevTags.length === 0 || JSON.stringify(prevTags) === JSON.stringify(nextTags)) {
          const field = activeTab === "modular" ? "modular_prompt" : "realtime_prompt";
          updateDraft("persona", field, pending);
        }
      }
      setActiveTab(tab);
    },
    [activePrompt, activeTab, updateDraft]
  );

  const handleScroll = useCallback(() => {
    if (textareaRef.current && highlightRef.current) {
      highlightRef.current.scrollTop = textareaRef.current.scrollTop;
      highlightRef.current.scrollLeft = textareaRef.current.scrollLeft;
    }
  }, []);

  const parsedSections = useMemo(() => {
    return parseXmlToSections(shownPrompt);
  }, [shownPrompt]);

  const isSmall = layoutMode === "small";

  return (
    <Card 
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "@container text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 transform-gpu flex flex-col",
        !isSmall && cn(
          "p-4 sm:p-5",
          layoutMode === "full-min" ? "lg:w-[380px] xl:w-[430px] 2xl:w-[480px]" : "lg:w-[480px]"
        )
      )}
    >
      {/* Header & Controls */}
      <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full gap-2 flex-nowrap">
        <div className="flex items-center gap-2 shrink-0">
          <CircleUserRound className="text-[rgb(var(--accent))]" size={17} />
          <span className="font-display text-[13px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
            {PERSONA_COPY.cardTitle}
          </span>
        </div>
        
        <div className="flex items-center gap-1.5 shrink-0">
          <SegmentedControl options={INSTRUCTION_TABS} value={activeTab} onChange={handleTabChange} size="sm" />
          <SegmentedControl options={VIEW_TABS} value={viewMode} onChange={setViewMode} size="sm" />
        </div>
      </div>

      {/* Main Body */}
      <div className="flex-1 flex flex-col w-full">
        {viewMode === "edit" ? (
          <>
          <div className="relative w-full rounded-xl overflow-hidden border border-[rgba(var(--accent),0.12)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)] transition-colors">
            {/* Syntax Highlight Backdrop Layer */}
            <div
              ref={highlightRef}
              aria-hidden="true"
              className={cn(
                "absolute inset-0 p-3 pointer-events-none overflow-auto font-mono text-[12px] sm:text-[12.5px] leading-relaxed whitespace-pre-wrap break-words select-none",
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
                "relative z-10 w-full p-3 font-mono text-[12px] sm:text-[12.5px] leading-relaxed text-transparent caret-[rgb(var(--accent))] bg-transparent resize-none focus:outline-none overflow-auto whitespace-pre-wrap break-words selection:bg-[rgba(var(--accent),0.25)] selection:text-[rgb(var(--foreground))]",
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
          /* Preview Mode: XML Stripped & Rendered as Structured Headings */
          <div 
            className={cn(
              "w-full bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.12)] rounded-xl p-3 overflow-y-auto select-text space-y-3.5 scrollbar-thin scrollbar-thumb-[rgba(var(--accent),0.2)] scrollbar-track-transparent",
              layoutMode === "full-max" ? "h-[200px]" : isSmall ? "h-[200px]" : "h-[150px]"
            )}
          >
            {parsedSections.length === 0 ? (
              <div className="flex items-center justify-center h-full text-[12px] text-[rgb(var(--foreground-muted))]/50 italic">
                {PERSONA_COPY.emptyPrompt}
              </div>
            ) : (
              parsedSections.map((sec, idx) => (
                <div key={idx} className="flex flex-col gap-1 rounded-lg bg-[rgba(var(--accent),0.03)] border border-[rgba(var(--accent),0.07)] p-2.5">
                  {(sec.title || sec.tag) && (
                    <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1 mb-1">
                      <span className="text-[11.5px] font-black uppercase tracking-wider text-[rgb(var(--accent))]">
                        {sec.title}
                      </span>
                      {sec.tag && (
                        <span className="text-[11px] font-mono uppercase font-bold text-[rgb(var(--foreground-muted))]/50 px-1 py-0.2 rounded bg-[rgba(var(--foreground),0.04)]">
                          &lt;{sec.tag}&gt;
                        </span>
                      )}
                    </div>
                  )}
                  <Markdown content={sec.content} variant="preview" className="text-[12px] text-[rgb(var(--foreground))]/80 leading-relaxed" />
                </div>
              ))
            )}
          </div>
        )}
        
      </div>
    </Card>
  );
});

PersonaCard.displayName = "PersonaCard";

