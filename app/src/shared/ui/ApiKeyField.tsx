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
      <div className={cn("space-y-2", className)}>
        {label && (
          <div className="flex items-center justify-between text-xs font-semibold tracking-wider text-[rgb(var(--foreground-muted))] uppercase">
            <span className={cn("flex items-center gap-1.5", error && "text-rose-400 font-bold")}>
              <Key size={13} className={error ? "text-rose-400" : "text-[rgb(var(--accent))]"} />
              {label}
            </span>
            {onTestConnection && (
              <button
                type="button"
                onClick={onTestConnection}
                disabled={testing || !value.trim()}
                className="flex items-center gap-1 text-[12px] font-medium text-[rgb(var(--accent))] hover:underline disabled:opacity-40 transition-opacity"
              >
                {testing ? (
                  <RefreshCw size={12} className="animate-spin" />
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
              "w-full bg-[rgba(var(--foreground),0.03)] rounded-xl px-3.5 py-2.5 pr-10 text-xs font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 focus:outline-none transition-all",
              error
                ? "border-2 border-rose-500/80 bg-rose-500/10 focus:border-rose-400 focus:ring-2 focus:ring-rose-500/30 shadow-[0_0_12px_rgba(244,63,94,0.2)] text-rose-100"
                : "border border-[rgba(var(--border),0.12)] focus:border-[rgba(var(--accent),0.4)] focus:bg-[rgba(var(--accent),0.02)]"
            )}
          />
          <Tooltip label={showKey ? "Hide key" : "Show key"} className="absolute right-3">
            <button
              type="button"
              onClick={() => setShowKey(!showKey)}
              className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors p-1"
            >
              {showKey ? <EyeOff size={15} /> : <Eye size={15} />}
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
