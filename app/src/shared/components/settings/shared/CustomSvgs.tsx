import { memo, type ReactNode } from "react";
import { cn } from "@/shared/lib/utils";

/* ── Preset control primitives (Compact 32px height, non-stretching) ── */

export interface PresetButtonProps {
  selected: boolean;
  onClick?: () => void;
  mono?: boolean;
  children: ReactNode;
  className?: string;
}

export const PresetButton = memo(
  ({ selected, onClick, mono = true, children, className }: PresetButtonProps) => (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "w-full h-[32px] py-1 px-1.5 rounded-lg border text-[10.5px] sm:text-[11px] font-bold transition-all duration-200 cursor-pointer flex items-center justify-center gap-1 shrink-0",
        mono && "font-mono",
        selected
          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
          : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]",
        className
      )}
    >
      {children}
    </button>
  )
);

PresetButton.displayName = "PresetButton";

export interface PresetInputProps {
  value: string;
  onChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  onBlur?: (e: React.FocusEvent<HTMLInputElement>) => void;
  onKeyDown?: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  placeholder: string;
  selected: boolean;
  inputMode?: "numeric" | "decimal" | "text";
  ariaLabel?: string;
}

export const PresetInput = memo(
  ({
    value,
    onChange,
    onBlur,
    onKeyDown,
    placeholder,
    selected,
    inputMode = "numeric",
    ariaLabel,
  }: PresetInputProps) => (
    <div
      className={cn(
        "w-full h-[32px] rounded-lg border flex items-center justify-center transition-all overflow-hidden shrink-0",
        selected
          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
          : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
      )}
    >
      <input
        type="text"
        inputMode={inputMode}
        value={value}
        onChange={onChange}
        onBlur={onBlur}
        onKeyDown={onKeyDown}
        placeholder={placeholder}
        aria-label={ariaLabel}
        className={cn(
          "w-full text-center text-[10.5px] sm:text-[11px] font-mono font-bold bg-transparent outline-none py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none",
          selected ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]",
          "placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal"
        )}
      />
    </div>
  )
);

PresetInput.displayName = "PresetInput";
