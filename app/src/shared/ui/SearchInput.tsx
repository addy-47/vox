import React from "react";
import { Search, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export interface SearchInputProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, "onChange"> {
  value: string;
  onChange: (value: string) => void;
  onClear?: () => void;
  placeholder?: string;
  className?: string;
}

export const SearchInput: React.FC<SearchInputProps> = ({
  value,
  onChange,
  onClear,
  placeholder = "Search...",
  className,
  ...props
}) => {
  const handleClear = () => {
    onChange("");
    if (onClear) onClear();
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape" && value) {
      handleClear();
    }
  };

  return (
    <div className={cn("relative flex items-center w-full", className)}>
      <Search
        size={14}
        className="absolute left-3 text-[rgb(var(--foreground-muted))]/60 pointer-events-none"
      />
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.12)] rounded-xl pl-9 pr-8 py-2 text-[13px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 font-medium focus:outline-none focus:border-[rgba(var(--accent),0.35)] transition-colors"
        {...props}
      />
      {value && (
        <button
          type="button"
          onClick={handleClear}
          className="absolute right-2.5 p-0.5 rounded-full text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer"
          aria-label="Clear search"
        >
          <X size={12} />
        </button>
      )}
    </div>
  );
};
