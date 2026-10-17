import React, { useState, useRef, useEffect, useCallback, memo } from "react";
import { Search, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export { HighlightMatch } from "@/shared/ui/HighlightMatch";

export interface SearchBarShellProps {
  id?: string;
  value: string;
  onChange: (value: string) => void;
  onClear: () => void;
  placeholder?: string;
  ariaLabel?: string;
  clearAriaLabel?: string;
  className?: string;
  dropdownPlacement?: "bottom" | "top";
  variant?: "inline" | "modal";
  autoFocus?: boolean;
  itemCount?: number;
  activeIndex?: number;
  onActiveIndexChange?: (index: number) => void;
  onSelectActive?: (index: number) => void;
  onEscape?: () => void;
  onKeyDown?: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  onFocusChange?: (focused: boolean) => void;
  inputRef?: React.RefObject<HTMLInputElement | null>;
  dropdownRef?: React.RefObject<HTMLDivElement | null>;
  isDropdownOpen?: boolean;
  dropdownHeader?: React.ReactNode;
  dropdownContent?: React.ReactNode;
  dropdownClassName?: string;
  children?: React.ReactNode;
}

export const SearchBarShell = memo<SearchBarShellProps>(({
  id = "search-input",
  value,
  onChange,
  onClear,
  placeholder = "Search...",
  ariaLabel = "Search",
  clearAriaLabel = "Clear search",
  className,
  dropdownPlacement = "bottom",
  variant = "inline",
  autoFocus = false,
  itemCount = 0,
  activeIndex = -1,
  onActiveIndexChange,
  onSelectActive,
  onEscape,
  onKeyDown,
  onFocusChange,
  inputRef,
  dropdownRef,
  isDropdownOpen,
  dropdownHeader,
  dropdownContent,
  dropdownClassName,
  children,
}) => {
  const [internalFocused, setInternalFocused] = useState(false);
  const internalInputRef = useRef<HTMLInputElement>(null);
  const effectiveInputRef = inputRef ?? internalInputRef;

  const internalDropdownRef = useRef<HTMLDivElement>(null);
  const effectiveDropdownRef = dropdownRef ?? internalDropdownRef;

  const blurTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleBlur = useCallback(() => {
    if (variant === "modal") return;
    if (blurTimerRef.current) clearTimeout(blurTimerRef.current);
    blurTimerRef.current = setTimeout(() => {
      blurTimerRef.current = null;
      setInternalFocused(false);
      onFocusChange?.(false);
    }, 200);
  }, [variant, onFocusChange]);

  const handleFocus = useCallback(() => {
    if (blurTimerRef.current) {
      clearTimeout(blurTimerRef.current);
      blurTimerRef.current = null;
    }
    setInternalFocused(true);
    onFocusChange?.(true);
  }, [onFocusChange]);

  useEffect(() => {
    return () => {
      if (blurTimerRef.current) clearTimeout(blurTimerRef.current);
    };
  }, []);

  useEffect(() => {
    if (autoFocus) {
      effectiveInputRef.current?.focus();
    }
  }, [autoFocus, effectiveInputRef]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      onKeyDown?.(e);
      if (e.defaultPrevented) return;

      if (e.key === "Escape") {
        e.preventDefault();
        setInternalFocused(false);
        onFocusChange?.(false);
        onEscape?.();
        return;
      }

      if (itemCount <= 0) return;

      if (e.key === "ArrowDown") {
        e.preventDefault();
        const next = activeIndex < 0 ? 0 : (activeIndex + 1) % itemCount;
        onActiveIndexChange?.(next);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        const prev = activeIndex <= 0 ? itemCount - 1 : activeIndex - 1;
        onActiveIndexChange?.(prev);
      } else if (e.key === "Enter") {
        e.preventDefault();
        const targetIndex = activeIndex >= 0 && activeIndex < itemCount ? activeIndex : 0;
        onSelectActive?.(targetIndex);
        setInternalFocused(false);
        onFocusChange?.(false);
        effectiveInputRef.current?.blur();
      }
    },
    [activeIndex, itemCount, onActiveIndexChange, onEscape, onFocusChange, onKeyDown, onSelectActive, effectiveInputRef]
  );

  const handleClearClick = useCallback(() => {
    onClear();
    effectiveInputRef.current?.focus();
  }, [onClear, effectiveInputRef]);

  const isTopDropdown = dropdownPlacement === "top";
  const shouldShowDropdown =
    variant !== "modal" &&
    (isDropdownOpen !== undefined
      ? isDropdownOpen
      : internalFocused && value.trim().length > 0 && (Boolean(dropdownContent) || itemCount > 0));

  return (
    <div
      className={cn(
        "relative pointer-events-auto flex flex-col w-full",
        variant === "modal" ? "h-full min-h-0" : "items-center",
        className
      )}
      data-arrow-nav
    >
      {/* ── Underline Search Input: Shared styling across History and Memory ── */}
      <div className="flex items-center gap-2 py-1.5 border-b border-[rgba(var(--foreground),0.18)] focus-within:border-[rgba(var(--accent),0.8)] transition-colors duration-200 w-full shrink-0">
        <Search size={13} className="text-[rgb(var(--accent))] shrink-0 opacity-70" />
        <input
          id={id}
          ref={effectiveInputRef}
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onFocus={handleFocus}
          onBlur={handleBlur}
          onKeyDown={handleKeyDown}
          role="combobox"
          aria-label={ariaLabel}
          aria-expanded={shouldShowDropdown || (variant === "modal" && itemCount > 0)}
          aria-controls={`${id}-listbox`}
          aria-haspopup="listbox"
          aria-autocomplete="list"
          autoComplete="off"
          placeholder={placeholder}
          className="flex-1 min-w-0 bg-transparent text-[12px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/60 focus:outline-none"
        />
        {value && (
          <button
            type="button"
            onClick={handleClearClick}
            className="p-0.5 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] cursor-pointer shrink-0 transition-colors"
            aria-label={clearAriaLabel}
          >
            <X size={12} />
          </button>
        )}
      </div>

      {/* ── Modal Variant Content ── */}
      {variant === "modal" && children}

      {/* ── Inline Quick Search Dropdown Container ── */}
      {shouldShowDropdown && (
        <div
          ref={effectiveDropdownRef}
          id={`${id}-listbox`}
          role="listbox"
          style={{ width: "min(420px, calc(100vw - 32px))" }}
          className={cn(
            "absolute left-1/2 -translate-x-1/2 rounded-2xl glass-card border border-[rgba(var(--accent),0.25)] bg-[rgba(var(--card),0.96)] shadow-lg p-2 z-50 flex flex-col gap-1 overflow-hidden",
            isTopDropdown ? "bottom-[calc(100%+8px)]" : "top-[calc(100%+8px)]",
            dropdownClassName
          )}
        >
          {dropdownHeader}
          {dropdownContent}
        </div>
      )}
    </div>
  );
});

SearchBarShell.displayName = "SearchBarShell";