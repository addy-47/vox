import { forwardRef, InputHTMLAttributes, ReactNode, useId } from "react";
import { cn } from "@/shared/lib/utils";

export interface UnderlineInputProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  label?: string;
  error?: boolean;
  errorMessage?: string;
  prefixIcon?: ReactNode;
  suffixAction?: ReactNode;
  containerClassName?: string;
  mono?: boolean;
}

export const UnderlineInput = forwardRef<HTMLInputElement, UnderlineInputProps>(
  (
    {
      label,
      error = false,
      errorMessage,
      prefixIcon,
      suffixAction,
      containerClassName,
      className,
      mono = true,
      disabled,
      ...props
    },
    ref
  ) => {
    // Stable per-instance id so <label htmlFor> actually targets the input.
    // React 19's useId supplies one without a module-level counter.
    const generatedId = useId();
    const inputId = props.id ?? `underline-input-${generatedId}`;
    const errorId = `${inputId}-error`;

    return (
      <div className={cn("space-y-1 w-full", containerClassName)}>
        {label && (
          <label
            htmlFor={inputId}
            className="flex items-center justify-between text-[11px] font-bold tracking-wider uppercase"
          >
            <span
              className={cn(
                "flex items-center gap-1 ml-0.5",
                error
                  ? "text-rose-400/85"
                  : "text-[rgb(var(--foreground-muted))]/75"
              )}
            >
              {prefixIcon}
              {label}
            </span>
          </label>
        )}

        <div
          className={cn(
            "relative flex items-center border-b transition-all duration-300 pb-0.5",
            error
              ? "border-rose-500/50 focus-within:border-b-2 focus-within:border-rose-400"
              : "border-[rgba(var(--border),0.15)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))]",
            disabled && "opacity-50 pointer-events-none"
          )}
        >
          <input
            ref={ref}
            id={inputId}
            disabled={disabled}
            aria-invalid={error || undefined}
            aria-describedby={error && errorMessage ? errorId : undefined}
            aria-label={label ? undefined : props["aria-label"]}
            className={cn(
              "w-full bg-transparent border-none outline-none text-[12px] py-1 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/30 transition-colors",
              mono ? "font-mono" : "font-sans font-medium",
              suffixAction && "pr-7",
              className
            )}
            {...props}
          />
          {suffixAction && (
            <div className="absolute right-0 flex items-center justify-center">
              {suffixAction}
            </div>
          )}
        </div>

        {errorMessage && (
          <span id={errorId} className="text-[11px] text-rose-400/90 font-medium flex items-center gap-1 ml-0.5">
            {errorMessage}
          </span>
        )}
      </div>
    );
  }
);

UnderlineInput.displayName = "UnderlineInput";
