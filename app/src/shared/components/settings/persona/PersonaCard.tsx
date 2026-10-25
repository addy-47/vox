import { useState, memo, useCallback, useMemo, useRef } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { CircleUserRound, Pencil, Check, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, Markdown } from "@/shared/ui";
import { Tooltip } from "@/shared/ui/Tooltip";
import { CardTitleBar } from "../shared/CardTitleBar";
import { PERSONA_COPY } from "@/data/settingsCopy";

interface PersonaCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

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
          <span key={`${match.index}-var`} className="text-[rgb(var(--warn-soft))] font-semibold bg-[rgba(var(--warn-soft),0.1)] px-1 py-0.2 rounded border border-[rgba(var(--warn-soft),0.2)]">
            {matchStr}
          </span>
        );
      } else if (matchStr.startsWith("[") && matchStr.endsWith("]")) {
        // [Bracket Headers]
        parts.push(
          <span key={`${match.index}-bracket`} className="text-[rgb(var(--info))] font-semibold">
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
          <span key={`${match.index}-code`} className="text-[rgb(var(--success))]/80 font-mono">
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
  const pipelineMode = useSettingsStore((s) => s.draftSettings?.interaction.pipeline_mode ?? "modular");
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const isModular = pipelineMode === "modular";
  const [viewMode, setViewMode] = useState<"edit" | "preview">("preview");
  /** Raw prompt captured when editing starts, so Cancel can restore it exactly. */
  const [editSnapshot, setEditSnapshot] = useState<string | null>(null);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const highlightRef = useRef<HTMLDivElement>(null);

  // Standardise modular and realtime: extract user-facing persona text
  const activeRawPrompt = isModular ? modularPrompt : realtimePrompt;
  const activePrompt = useMemo(() => extractPersonaText(activeRawPrompt), [activeRawPrompt]);

  // Local keystroke draft
  const [promptDraft, setPromptDraft] = useState<string | null>(null);
  const promptDraftRef = useRef<string | null>(null);
  const [tagViolation, setTagViolation] = useState(false);

  const shownPrompt = promptDraft ?? activePrompt;
  const tagSpans = useMemo(
    () => (viewMode === "edit" ? getProtectedXmlTagSpans(shownPrompt) : []),
    [viewMode, shownPrompt]
  );
  const highlightedContent = useMemo(
    () => (viewMode === "edit" ? renderSyntaxHighlightedText(shownPrompt) : null),
    [viewMode, shownPrompt]
  );

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
        nativeEvent.preventDefault();
        e.preventDefault();
      }
    },
    [tagSpans]
  );

  const commitDraft = useCallback(
    (nextValue: string) => {
      setTagViolation(false);
      if (isModular) {
        updateDraft("persona", "modular_prompt", nextValue);
      } else {
        const nextRaw = updatePersonaInRawPrompt(realtimePrompt, nextValue);
        updateDraft("persona", "realtime_prompt", nextRaw);
      }
    },
    [isModular, realtimePrompt, updateDraft]
  );

  const handlePromptChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const nextValue = e.target.value;
      setPromptDraft(nextValue);
      promptDraftRef.current = nextValue;
    },
    []
  );

  const handlePromptBlur = useCallback(() => {
    // Keep draft locally in state while editing; do not trim or switch to activePrompt on blur
  }, []);

  const handleScroll = useCallback(() => {
    if (textareaRef.current && highlightRef.current) {
      highlightRef.current.scrollTop = textareaRef.current.scrollTop;
      highlightRef.current.scrollLeft = textareaRef.current.scrollLeft;
    }
  }, []);

  const startEditing = useCallback(() => {
    setEditSnapshot(activeRawPrompt);
    setPromptDraft(activePrompt);
    promptDraftRef.current = activePrompt;
    setViewMode("edit");
  }, [activeRawPrompt, activePrompt]);

  const finishEditing = useCallback(() => {
    const pending = promptDraftRef.current;
    if (pending !== null) {
      commitDraft(pending);
    }
    setPromptDraft(null);
    promptDraftRef.current = null;
    setEditSnapshot(null);
    setViewMode("preview");
  }, [commitDraft]);

  const cancelEditing = useCallback(() => {
    if (editSnapshot !== null) {
      updateDraft(
        "persona",
        isModular ? "modular_prompt" : "realtime_prompt",
        editSnapshot
      );
    }
    setPromptDraft(null);
    promptDraftRef.current = null;
    setTagViolation(false);
    setEditSnapshot(null);
    setViewMode("preview");
  }, [editSnapshot, isModular, updateDraft]);

  const isSmall = layoutMode === "small";

  return (
    <Card 
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "@container text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 transform-gpu flex flex-col",
        !isSmall && cn("p-4 sm:p-5")
      )}
    >
      {/* Header */}
      <CardTitleBar
        icon={<CircleUserRound size={17} />}
        title={PERSONA_COPY.cardTitle}
      />

      {/* Main Body: Default Preview with Floating Pencil Icon Toggle in Corner */}
      <div className="flex-1 flex flex-col w-full relative">
        {/* Edit trigger sits in the top corner only while previewing. */}
        {viewMode === "preview" && (
          <div className="absolute top-2 right-2 z-20">
            <Tooltip label={PERSONA_COPY.editPersona}>
              <button
                type="button"
                onClick={startEditing}
                className="p-1.5 rounded-lg transition-all duration-200 cursor-pointer flex items-center justify-center text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--foreground),0.05)] border border-transparent"
                aria-label={PERSONA_COPY.editPersona}
              >
                <Pencil size={13} />
              </button>
            </Tooltip>
          </div>
        )}

        {/* Save / cancel pair, bottom-right, same grammar as inline edits elsewhere. */}
        {viewMode === "edit" && (
          <div className="absolute bottom-2 right-2 z-20 flex items-center gap-1">
            <Tooltip label={PERSONA_COPY.doneEditing} asChild>
              <button
                type="button"
                onClick={finishEditing}
                aria-label={PERSONA_COPY.doneEditing}
                className="p-1 rounded-md bg-[rgba(var(--accent),0.18)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] transition-colors cursor-pointer shrink-0"
              >
                <Check size={13} strokeWidth={2.5} />
              </button>
            </Tooltip>
            <Tooltip label={PERSONA_COPY.cancelEditing} asChild>
              <button
                type="button"
                onClick={cancelEditing}
                aria-label={PERSONA_COPY.cancelEditing}
                className="p-1 rounded-md text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer shrink-0"
              >
                <X size={13} strokeWidth={2.5} />
              </button>
            </Tooltip>
          </div>
        )}

        {viewMode === "edit" ? (
          <>
            <div className="relative w-full rounded-xl overflow-hidden border border-[rgba(var(--accent),0.12)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)] transition-colors">
              {/* Syntax Highlight Backdrop Layer */}
              <div
                ref={highlightRef}
                aria-hidden="true"
                className={cn(
                  "absolute inset-0 p-3 pr-9 pb-8 pointer-events-none overflow-auto font-mono text-[12px] sm:text-[12.5px] leading-relaxed whitespace-pre-wrap break-words select-none",
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
                placeholder={isModular ? PERSONA_COPY.modularPlaceholder : PERSONA_COPY.realtimePlaceholder}
                spellCheck={false}
                className={cn(
                  "relative z-10 w-full p-3 pr-9 pb-8 font-mono text-[12px] sm:text-[12.5px] leading-relaxed text-transparent caret-[rgb(var(--accent))] bg-transparent resize-none focus:outline-none overflow-auto whitespace-pre-wrap break-words selection:bg-[rgba(var(--accent),0.25)] selection:text-[rgb(var(--foreground))]",
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

