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
      <div className={cn("space-y-1", className)}>
        {label && (
          <div className="flex items-center justify-between text-[11px] font-bold tracking-wider uppercase">
            <span className={cn("flex items-center gap-1 ml-0.5", error ? "text-rose-400/85" : "text-[rgb(var(--foreground-muted))]/75")}>
              <Key size={12} className={error ? "text-rose-400/70" : "text-[rgb(var(--accent))]"} />
              {label}
            </span>
            {onTestConnection && (
              <button
                type="button"
                onClick={onTestConnection}
                disabled={testing || !value.trim()}
                className="flex items-center gap-1 text-[11px] font-medium text-[rgb(var(--accent))] hover:underline disabled:opacity-40 transition-opacity"
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

        <div className="relative flex items-center">
          <input
            type={showKey ? "text" : "password"}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            className={cn(
              "w-full bg-[rgba(var(--foreground),0.03)] rounded-lg px-2.5 h-[32px] pr-8 text-[11px] sm:text-[12px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/30 focus:outline-none transition-all",
              error
                ? "border border-rose-500/35 bg-rose-500/[0.02] focus:border-rose-400/60 focus:bg-rose-500/[0.04]"
                : "border border-[rgba(var(--border),0.12)] focus:border-[rgba(var(--accent),0.4)] focus:bg-[rgba(var(--accent),0.02)]"
            )}
          />
          <Tooltip label={showKey ? "Hide key" : "Show key"} className="absolute right-2">
            <button
              type="button"
              onClick={() => setShowKey(!showKey)}
              className="text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] transition-colors p-1"
            >
              {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
          </Tooltip>
        </div>

        {statusMessage && (
          <div
            className={cn(
              "flex items-center gap-1.5 text-[12px] font-medium transition-all",
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
