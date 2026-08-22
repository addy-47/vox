import { useState, memo } from "react";
import { Eye, EyeOff, CheckCircle2, AlertCircle, RefreshCw, Key } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";

interface ApiKeyFieldProps {
  label?: string;
  value: string;
  onChange: (val: string) => void;
  placeholder?: string;
  testing?: boolean;
  status?: "success" | "error" | "idle";
  statusMessage?: string | null;
  onTestConnection?: () => void;
  className?: string;
  error?: boolean;
}

export const ApiKeyField = memo(
  ({
    label = "API Key",
    value,
    onChange,
    placeholder = "sk-...",
    testing = false,
    status = "idle",
    statusMessage,
    onTestConnection,
    className,
    error = false,
  }: ApiKeyFieldProps) => {
    const [showKey, setShowKey] = useState(false);

    return (
      <div className={cn("space-y-1 w-full", className)}>
        {label && (
          <div className="flex items-center justify-between text-[11px] font-bold tracking-wider uppercase">
            <span
              className={cn(
                "flex items-center gap-1 ml-0.5",
                error ? "text-rose-400/85" : "text-[rgb(var(--foreground-muted))]/75"
              )}
            >
              <Key size={12} className={error ? "text-rose-400/70" : "text-[rgb(var(--accent))]"} />
              {label}
            </span>
            {onTestConnection && (
              <button
                type="button"
                onClick={onTestConnection}
                disabled={testing || !value.trim()}
                className="flex items-center gap-1 text-[11px] font-medium text-[rgb(var(--accent))] hover:underline disabled:opacity-40 transition-opacity cursor-pointer"
              >
                {testing ? (
                  <RefreshCw size={11} className="animate-spin" />
                ) : (
                  "Test Connection"
                )}
              </button>
            )}
          </div>
        )}

        <div
          className={cn(
            "relative flex items-center border-b transition-all duration-300 pb-0.5",
            error
              ? "border-rose-500/50 focus-within:border-b-2 focus-within:border-rose-400"
              : "border-[rgba(var(--border),0.15)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))]"
          )}
        >
          <input
            type={showKey ? "text" : "password"}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            className="w-full bg-transparent border-none outline-none text-[12px] font-mono py-1 pr-7 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/30 transition-colors"
          />
          <Tooltip label={showKey ? "Hide key" : "Show key"} className="absolute right-0">
            <button
              type="button"
              onClick={() => setShowKey(!showKey)}
              className="text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] transition-colors p-1 cursor-pointer flex items-center justify-center"
            >
              {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
          </Tooltip>
        </div>

        {statusMessage && (
          <div
            className={cn(
              "flex items-center gap-1.5 text-[11.5px] font-medium transition-all pt-0.5",
              status === "success" && "text-emerald-400",
              status === "error" && "text-rose-400",
              status === "idle" && "text-[rgb(var(--foreground-muted))]"
            )}
          >
            {status === "success" && <CheckCircle2 size={13} />}
            {status === "error" && <AlertCircle size={13} />}
            <span>{statusMessage}</span>
          </div>
        )}
      </div>
    );
  }
);

ApiKeyField.displayName = "ApiKeyField";
