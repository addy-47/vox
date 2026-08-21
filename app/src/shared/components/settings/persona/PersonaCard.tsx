import { useState, memo, useCallback, useMemo, useRef } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { UserCircle, Code2, Eye, Sparkles } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import ReactMarkdown from "react-markdown";
import { Card, SegmentedControl } from "@/shared/ui";
import { PERSONA_COPY } from "@/data/settingsCopy";

interface PersonaCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

const INSTRUCTION_TABS = [
  { id: "modular" as const, label: PERSONA_COPY.tabModular },
  { id: "realtime" as const, label: PERSONA_COPY.tabRealtime },
];

const VIEW_TABS = [
  { id: "edit" as const, label: PERSONA_COPY.viewEdit, icon: Code2 },
  { id: "preview" as const, label: PERSONA_COPY.viewPreview, icon: Eye },
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
  title: string;
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
        title: "Overview & Directives",
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
      title: sections.length === 0 ? "Directives" : "Additional Context",
      content: remaining,
    });
  }

  return sections;
}

const MarkdownPreviewComponents = {
  h1: ({ ...props }: any) => <h1 className="text-[13px] font-bold mt-2 mb-1 text-[rgb(var(--accent))]" {...props} />,
  h2: ({ ...props }: any) => <h2 className="text-[12.5px] font-bold mt-2 mb-1 text-[rgb(var(--accent))]" {...props} />,
  h3: ({ ...props }: any) => <h3 className="text-[12px] font-bold mt-1.5 mb-1 text-[rgb(var(--accent))]" {...props} />,
  p: ({ ...props }: any) => <p className="mb-1.5 last:mb-0 text-[12px] text-[rgb(var(--foreground))]/80 leading-relaxed" {...props} />,
  ul: ({ ...props }: any) => <ul className="list-disc list-inside mb-2 pl-1 space-y-1 text-[12px] text-[rgb(var(--foreground))]/80 leading-relaxed" {...props} />,
  ol: ({ ...props }: any) => <ol className="list-decimal list-inside mb-2 pl-1 space-y-1 text-[12px] text-[rgb(var(--foreground))]/80 leading-relaxed" {...props} />,
  li: ({ ...props }: any) => <li className="ml-1" {...props} />,
  code: ({ children, ...props }: any) => {
    const str = String(children);
    if (str === "<lang>" || str === "<script>") {
      return (
        <span className="text-amber-400 font-mono font-bold bg-amber-400/10 px-1 py-0.5 rounded border border-amber-400/20 text-[11px]">
          {str}
        </span>
      );
    }
    return (
      <code className="bg-[rgba(var(--foreground),0.06)] px-1 py-0.5 rounded font-mono text-[11px] text-[rgb(var(--accent))]" {...props}>
        {children}
      </code>
    );
  },
  pre: ({ ...props }: any) => (
    <pre className="bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--accent),0.1)] rounded-lg p-2 font-mono text-[11px] overflow-x-auto my-1.5 w-full" {...props} />
  ),
};

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
  const [viewMode, setViewMode] = useState<"edit" | "preview">("edit");

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const highlightRef = useRef<HTMLDivElement>(null);

  const activePrompt = activeTab === "modular" ? modularPrompt : realtimePrompt;

  const tagSpans = useMemo(() => getProtectedXmlTagSpans(activePrompt), [activePrompt]);

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

  const handlePromptChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const nextValue = e.target.value;
      const prevTags = tagSpans.map((s) => s.tag);
      const nextTags = getProtectedXmlTagSpans(nextValue).map((s) => s.tag);

      // Verify tag integrity — if tags were modified or stripped, reject the mutation
      if (prevTags.length > 0 && JSON.stringify(prevTags) !== JSON.stringify(nextTags)) {
        return;
      }

      const field = activeTab === "modular" ? "modular_prompt" : "realtime_prompt";
      updateDraft("persona", field, nextValue);
    },
    [activeTab, tagSpans, updateDraft]
  );

  const handleScroll = useCallback(() => {
    if (textareaRef.current && highlightRef.current) {
      highlightRef.current.scrollTop = textareaRef.current.scrollTop;
      highlightRef.current.scrollLeft = textareaRef.current.scrollLeft;
    }
  }, []);

  const parsedSections = useMemo(() => {
    return parseXmlToSections(activePrompt);
  }, [activePrompt]);

  const isSmall = layoutMode === "small";

  return (
    <Card 
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 transform-gpu flex flex-col",
        !isSmall && cn(
          "p-4 sm:p-5",
          layoutMode === "full-min" ? "lg:w-[320px] xl:w-[380px] 2xl:w-[460px]" : "lg:w-[460px]"
        )
      )}
    >
      {/* Header & Tabs (Full Desktop Mode) */}
      {!isSmall && (
        <div className="flex items-center justify-between mb-2.5 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full gap-2">
          <div className="flex items-center gap-1.5 sm:gap-2 shrink-0">
            <UserCircle className="text-[rgb(var(--accent))]" size={17} />
            <span className="font-display text-[12.5px] sm:text-[13px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
              {PERSONA_COPY.cardTitle}
            </span>
          </div>
          
          <div className="flex items-center gap-2 shrink-0">
            <SegmentedControl options={INSTRUCTION_TABS} value={activeTab} onChange={setActiveTab} size="sm" />
            <SegmentedControl options={VIEW_TABS} value={viewMode} onChange={setViewMode} size="sm" />
          </div>
        </div>
      )}

      {/* Small Layout Pills Header */}
      {isSmall && (
        <div className="flex items-center justify-end mb-2.5 shrink-0 w-full gap-2">
          <SegmentedControl options={INSTRUCTION_TABS} value={activeTab} onChange={setActiveTab} size="sm" />
          <SegmentedControl options={VIEW_TABS} value={viewMode} onChange={setViewMode} size="sm" />
        </div>
      )}

      {/* Main Body */}
      <div className={cn("flex-1 flex flex-col justify-between gap-2", isSmall ? "min-h-[220px]" : "min-h-[160px]")}>
        {viewMode === "edit" ? (
          <div className="relative flex-1 w-full rounded-xl overflow-hidden border border-[rgba(var(--accent),0.12)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)] transition-colors">
            {/* Syntax Highlight Backdrop Layer */}
            <div
              ref={highlightRef}
              aria-hidden="true"
              className={cn(
                "absolute inset-0 p-3 pointer-events-none overflow-auto font-mono text-[12px] sm:text-[12.5px] leading-relaxed whitespace-pre-wrap break-words select-none",
                layoutMode === "full-max" ? "h-[160px]" : isSmall ? "h-[200px]" : "h-[120px]"
              )}
            >
              {renderSyntaxHighlightedText(activePrompt)}
            </div>

            {/* Foreground Editable Transparent Textarea */}
            <textarea
              ref={textareaRef}
              value={activePrompt}
              onChange={handlePromptChange}
              onKeyDown={handleKeyDown}
              onBeforeInput={handleBeforeInput}
              onScroll={handleScroll}
              placeholder={activeTab === "modular" ? PERSONA_COPY.modularPlaceholder : PERSONA_COPY.realtimePlaceholder}
              spellCheck={false}
              className={cn(
                "relative z-10 w-full p-3 font-mono text-[12px] sm:text-[12.5px] leading-relaxed text-transparent caret-[rgb(var(--accent))] bg-transparent resize-none focus:outline-none overflow-auto whitespace-pre-wrap break-words selection:bg-[rgba(var(--accent),0.25)] selection:text-[rgb(var(--foreground))]",
                layoutMode === "full-max" ? "h-[160px]" : isSmall ? "h-[200px]" : "h-[120px]"
              )}
            />
          </div>
        ) : (
          /* Preview Mode: XML Stripped & Rendered as Structured Headings */
          <div 
            className={cn(
              "w-full bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.12)] rounded-xl p-3 overflow-y-auto select-text space-y-3.5 scrollbar-thin scrollbar-thumb-[rgba(var(--accent),0.2)] scrollbar-track-transparent",
              layoutMode === "full-max" ? "h-[160px]" : isSmall ? "h-[200px]" : "h-[120px]"
            )}
          >
            {parsedSections.length === 0 ? (
              <div className="flex items-center justify-center h-full text-[12px] text-[rgb(var(--foreground-muted))]/50 italic">
                {PERSONA_COPY.emptyPrompt}
              </div>
            ) : (
              parsedSections.map((sec, idx) => (
                <div key={idx} className="flex flex-col gap-1 rounded-lg bg-[rgba(var(--accent),0.03)] border border-[rgba(var(--accent),0.07)] p-2.5">
                  <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1 mb-1">
                    <span className="text-[11.5px] font-black uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-1.5">
                      <Sparkles size={11} className="text-[rgb(var(--accent))]/70" />
                      {sec.title}
                    </span>
                    {sec.tag && (
                      <span className="text-[11px] font-mono uppercase font-bold text-[rgb(var(--foreground-muted))]/50 px-1 py-0.2 rounded bg-[rgba(var(--foreground),0.04)]">
                        &lt;{sec.tag}&gt;
                      </span>
                    )}
                  </div>
                  <div className="text-[12px] text-[rgb(var(--foreground))]/80 leading-relaxed">
                    <ReactMarkdown components={MarkdownPreviewComponents}>
                      {sec.content}
                    </ReactMarkdown>
                  </div>
                </div>
              ))
            )}
          </div>
        )}

        {/* Footer Guidance Note */}
        <p className="text-[11px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/60 leading-normal font-semibold uppercase tracking-wide px-0.5">
          {activeTab === "modular" ? (
            <>
              Supports <code className="text-amber-400 font-mono font-bold bg-amber-400/10 px-1 py-0.2 rounded border border-amber-400/20">&lt;lang&gt;</code> and <code className="text-amber-400 font-mono font-bold bg-amber-400/10 px-1 py-0.2 rounded border border-amber-400/20">&lt;script&gt;</code> template variables, dynamically resolved based on user speech language.
            </>
          ) : (
            PERSONA_COPY.realtimeFooterHint
          )}
        </p>
      </div>
    </Card>
  );
});

PersonaCard.displayName = "PersonaCard";

