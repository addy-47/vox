import { memo, useMemo } from "react";
import ReactMarkdown from "react-markdown";
import { cn } from "@/shared/lib/utils";

export type MarkdownVariant = "document" | "bubble" | "preview";

export interface MarkdownProps {
  content: string;
  variant?: MarkdownVariant;
  className?: string;
  autoHeadings?: boolean;
}

/**
 * Normalizes plain text that uses standalone title lines (e.g. "Overview\n\n...")
 * into Markdown headings ("## Overview\n\n...") so unhashed LLM summaries format properly.
 */
function normalizeMarkdownContent(raw: string, autoHeadings: boolean): string {
  if (!raw) return "";
  let text = raw;

  if (autoHeadings) {
    const lines = text.split("\n");
    const formattedLines: string[] = [];

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      const trimmed = line.trim();

      const isHeadingCandidate =
        trimmed.length > 2 &&
        trimmed.length < 50 &&
        /^[A-Z][A-Za-z0-9, &/'"()-]+$/.test(trimmed) &&
        !trimmed.startsWith("#") &&
        !trimmed.startsWith("-") &&
        !trimmed.startsWith("*") &&
        !trimmed.startsWith(">") &&
        !/^\d+\./.test(trimmed);

      const prevEmpty = i === 0 || lines[i - 1].trim() === "";
      const nextNonHeading = i + 1 < lines.length && lines[i + 1].trim().length > 0;

      if (isHeadingCandidate && (prevEmpty || nextNonHeading)) {
        formattedLines.push(`## ${trimmed}`);
      } else {
        formattedLines.push(line);
      }
    }
    text = formattedLines.join("\n");
  }

  return text;
}

export const Markdown = memo(
  ({ content, variant = "document", className, autoHeadings = false }: MarkdownProps) => {
    const normalized = useMemo(
      () => normalizeMarkdownContent(content, autoHeadings),
      [content, autoHeadings]
    );

    const components = useMemo(() => {
      if (variant === "bubble") {
        return {
          h1: ({ children, ...props }: any) => (
            <h1 className="font-display text-[13.5px] font-bold text-[rgb(var(--accent))] mt-2 mb-1" {...props}>
              {children}
            </h1>
          ),
          h2: ({ children, ...props }: any) => (
            <h2 className="font-display text-[13px] font-bold text-[rgb(var(--accent))] mt-2 mb-1" {...props}>
              {children}
            </h2>
          ),
          h3: ({ children, ...props }: any) => (
            <h3 className="font-display text-[12.5px] font-semibold text-[rgb(var(--foreground))] mt-1.5 mb-1" {...props}>
              {children}
            </h3>
          ),
          p: ({ children, ...props }: any) => (
            <p className="mb-1.5 last:mb-0 leading-relaxed break-words" {...props}>
              {children}
            </p>
          ),
          ul: ({ children, ...props }: any) => (
            <ul className="list-disc pl-4 my-1.5 space-y-0.5 marker:text-[rgb(var(--accent))]" {...props}>
              {children}
            </ul>
          ),
          ol: ({ children, ...props }: any) => (
            <ol className="list-decimal pl-4 my-1.5 space-y-0.5 marker:text-[rgb(var(--accent))]" {...props}>
              {children}
            </ol>
          ),
          li: ({ children, ...props }: any) => (
            <li className="leading-relaxed" {...props}>
              {children}
            </li>
          ),
          code: ({ children, ...props }: any) => (
            <code
              className="px-1 py-0.5 rounded bg-[rgba(var(--foreground),0.08)] border border-[rgba(var(--border),0.12)] font-mono text-[11px] text-[rgb(var(--accent))]"
              {...props}
            >
              {children}
            </code>
          ),
          pre: ({ children, ...props }: any) => (
            <pre
              className="p-2.5 my-2 rounded-xl bg-[rgba(var(--card),0.9)] border border-[rgba(var(--accent),0.2)] overflow-x-auto font-mono text-[11.5px]"
              {...props}
            >
              {children}
            </pre>
          ),
          strong: ({ children, ...props }: any) => (
            <strong className="font-bold text-[rgb(var(--accent))]" {...props}>
              {children}
            </strong>
          ),
          blockquote: ({ children, ...props }: any) => (
            <blockquote
              className="border-l-2 border-[rgb(var(--accent))] pl-2.5 py-0.5 my-1.5 bg-[rgba(var(--accent),0.04)] rounded-r italic text-[rgb(var(--foreground-muted))]"
              {...props}
            >
              {children}
            </blockquote>
          ),
        };
      }

      if (variant === "preview") {
        return {
          h1: ({ children, ...props }: any) => (
            <h1 className="text-[13px] font-bold mt-2 mb-1 text-[rgb(var(--accent))]" {...props}>
              {children}
            </h1>
          ),
          h2: ({ children, ...props }: any) => (
            <h2 className="text-[12.5px] font-bold mt-2 mb-1 text-[rgb(var(--accent))]" {...props}>
              {children}
            </h2>
          ),
          h3: ({ children, ...props }: any) => (
            <h3 className="text-[12px] font-bold mt-1.5 mb-1 text-[rgb(var(--accent))]" {...props}>
              {children}
            </h3>
          ),
          p: ({ children, ...props }: any) => (
            <p className="mb-1.5 last:mb-0 text-[12px] text-[rgb(var(--foreground))]/80 leading-relaxed" {...props}>
              {children}
            </p>
          ),
          ul: ({ children, ...props }: any) => (
            <ul className="list-disc list-inside mb-2 pl-1 space-y-1 text-[12px] text-[rgb(var(--foreground))]/80 leading-relaxed marker:text-[rgb(var(--accent))]" {...props}>
              {children}
            </ul>
          ),
          ol: ({ children, ...props }: any) => (
            <ol className="list-decimal list-inside mb-2 pl-1 space-y-1 text-[12px] text-[rgb(var(--foreground))]/80 leading-relaxed marker:text-[rgb(var(--accent))]" {...props}>
              {children}
            </ol>
          ),
          li: ({ children, ...props }: any) => <li className="ml-1" {...props}>{children}</li>,
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
          pre: ({ children, ...props }: any) => (
            <pre className="bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--accent),0.1)] rounded-lg p-2 font-mono text-[11px] overflow-x-auto my-1.5 w-full" {...props}>
              {children}
            </pre>
          ),
          strong: ({ children, ...props }: any) => (
            <strong className="font-bold text-[rgb(var(--accent))]" {...props}>
              {children}
            </strong>
          ),
        };
      }

      // Default: "document"
      return {
        h1: ({ children, ...props }: any) => (
          <h1
            className="font-display text-[14.5px] font-bold text-[rgb(var(--foreground))] border-b border-[rgba(var(--accent),0.18)] pb-1.5 mb-2.5 mt-5 first:mt-0 tracking-wider uppercase"
            {...props}
          >
            {children}
          </h1>
        ),
        h2: ({ children, ...props }: any) => (
          <h2
            className="font-display text-[13px] font-bold text-[rgb(var(--accent))] border-b border-[rgba(var(--border),0.08)] pb-1 mb-2 mt-4 first:mt-0 tracking-wider uppercase"
            {...props}
          >
            {children}
          </h2>
        ),
        h3: ({ children, ...props }: any) => (
          <h3
            className="font-display text-[12px] font-semibold text-[rgb(var(--foreground))] mb-1.5 mt-3 tracking-wide"
            {...props}
          >
            {children}
          </h3>
        ),
        p: ({ children, ...props }: any) => (
          <p
            className="text-[13px] text-[rgb(var(--foreground))]/85 leading-[1.7] mb-3 last:mb-0"
            {...props}
          >
            {children}
          </p>
        ),
        ul: ({ children, ...props }: any) => (
          <ul
            className="space-y-1.5 my-2.5 list-disc pl-5 marker:text-[rgb(var(--accent))]/70"
            {...props}
          >
            {children}
          </ul>
        ),
        ol: ({ children, ...props }: any) => (
          <ol
            className="space-y-1.5 my-2.5 list-decimal pl-5 marker:text-[rgb(var(--accent))]/70"
            {...props}
          >
            {children}
          </ol>
        ),
        li: ({ children, ...props }: any) => (
          <li className="text-[13px] text-[rgb(var(--foreground))]/85 leading-[1.65]" {...props}>
            {children}
          </li>
        ),
        strong: ({ children, ...props }: any) => (
          <strong className="font-bold text-[rgb(var(--accent))]" {...props}>
            {children}
          </strong>
        ),
        em: ({ children, ...props }: any) => (
          <em className="italic text-[rgb(var(--foreground))]/80" {...props}>
            {children}
          </em>
        ),
        blockquote: ({ children, ...props }: any) => (
          <blockquote
            className="border-l-2 border-[rgb(var(--accent))] pl-3 py-1 my-2.5 bg-[rgba(var(--accent),0.04)] rounded-r text-[13px] italic text-[rgb(var(--foreground-muted))]"
            {...props}
          >
            {children}
          </blockquote>
        ),
        code: ({ children, ...props }: any) => (
          <code
            className="px-1.5 py-0.5 rounded-md bg-[rgba(var(--foreground),0.08)] border border-[rgba(var(--border),0.15)] font-mono text-[11.5px] text-[rgb(var(--accent))]"
            {...props}
          >
            {children}
          </code>
        ),
        pre: ({ children, ...props }: any) => (
          <pre
            className="p-3 my-2.5 rounded-xl bg-[rgba(var(--card),0.95)] border border-[rgba(var(--accent),0.25)] overflow-x-auto font-mono text-[12px] text-[rgb(var(--foreground))]"
            {...props}
          >
            {children}
          </pre>
        ),
        a: ({ children, ...props }: any) => (
          <a
            className="text-[rgb(var(--accent))] underline hover:opacity-80 transition-opacity"
            target="_blank"
            rel="noopener noreferrer"
            {...props}
          >
            {children}
          </a>
        ),
        hr: ({ ...props }: any) => (
          <hr className="my-4 border-[rgba(var(--border),0.15)]" {...props} />
        ),
      };
    }, [variant]);

    return (
      <div className={cn("select-text", className)}>
        <ReactMarkdown components={components}>{normalized}</ReactMarkdown>
      </div>
    );
  }
);

Markdown.displayName = "Markdown";
