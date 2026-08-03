import React, { memo } from "react";
import { Check, Cpu, Cloud } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export interface ProviderOption {
  id: string;
  name: string;
  description?: string;
  icon?: React.ReactNode;
  badge?: string;
  isLocal?: boolean;
}

interface ProviderSelectorProps {
  providers: ProviderOption[];
  selectedId: string;
  onSelect: (id: string) => void;
  gridCols?: string;
  className?: string;
}

export const ProviderSelector: React.FC<ProviderSelectorProps> = memo(
  ({ providers, selectedId, onSelect, gridCols = "grid-cols-2 sm:grid-cols-3", className }) => {
    return (
      <div className={cn("grid gap-2.5", gridCols, className)}>
        {providers.map((p) => {
          const isSelected = p.id === selectedId;
          return (
            <button
              key={p.id}
              type="button"
              onClick={() => onSelect(p.id)}
              className={cn(
                "relative flex flex-col justify-between p-3.5 rounded-xl border text-left transition-all duration-300 group",
                isSelected
                  ? "bg-[rgba(var(--accent),0.08)] border-[rgba(var(--accent),0.4)] text-[rgb(var(--foreground))]"
                  : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--border),0.1)] hover:bg-[rgba(var(--foreground),0.04)] hover:border-[rgba(var(--border),0.2)] text-[rgb(var(--foreground-muted))]"
              )}
            >
              <div className="flex items-start justify-between gap-2 mb-2">
                <div className="flex items-center gap-2">
                  <div
                    className={cn(
                      "w-7 h-7 rounded-lg flex items-center justify-center border transition-colors",
                      isSelected
                        ? "bg-[rgba(var(--accent),0.15)] border-[rgba(var(--accent),0.3)] text-[rgb(var(--accent))]"
                        : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.1)] text-[rgb(var(--foreground-muted))]"
                    )}
                  >
                    {p.icon || (p.isLocal ? <Cpu size={15} /> : <Cloud size={15} />)}
                  </div>
                  <div>
                    <h4 className="text-xs font-semibold text-[rgb(var(--foreground))] flex items-center gap-1.5">
                      {p.name}
                      {p.badge && (
                        <span className="px-1.5 py-0.5 rounded text-[9px] font-bold tracking-wider uppercase bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.2)]">
                          {p.badge}
                        </span>
                      )}
                    </h4>
                  </div>
                </div>
                {isSelected && (
                  <div className="w-4 h-4 rounded-full bg-[rgb(var(--accent))] text-black flex items-center justify-center text-[10px] font-bold">
                    <Check size={11} strokeWidth={3} />
                  </div>
                )}
              </div>

              {p.description && (
                <p className="text-[11px] text-[rgb(var(--foreground-muted))]/80 line-clamp-2 leading-relaxed">
                  {p.description}
                </p>
              )}
            </button>
          );
        })}
      </div>
    );
  }
);

ProviderSelector.displayName = "ProviderSelector";
