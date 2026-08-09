import React from "react";
import { cn } from "@/shared/lib/utils";
import { Badge } from "./Badge";

export interface SliderFieldProps {
  label: string;
  sublabel?: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (value: number) => void;
  formatValue?: (value: number) => string;
  unit?: string;
  icon?: React.ElementType;
  className?: string;
}

export const SliderField: React.FC<SliderFieldProps> = ({
  label,
  sublabel,
  value,
  min,
  max,
  step = 1,
  onChange,
  formatValue,
  unit,
  icon: Icon,
  className,
}) => {
  const displayValue = formatValue
    ? formatValue(value)
    : unit
    ? `${value} ${unit}`
    : `${value}`;

  const pct = Math.min(100, Math.max(0, ((value - min) / (max - min || 1)) * 100));

  return (
    <div className={cn("space-y-1.5 w-full", className)}>
      <div className="flex justify-between items-center">
        <div className="flex flex-col">
          <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70 flex items-center gap-1.5">
            {Icon && <Icon size={12} className="text-[rgb(var(--accent))]" />}
            {label}
          </span>
          {sublabel && (
            <span className="text-[11px] text-[rgb(var(--foreground-muted))]/55">
              {sublabel}
            </span>
          )}
        </div>
        <Badge variant="accent" size="sm" className="font-mono">
          {displayValue}
        </Badge>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{
          background: `linear-gradient(to right, rgba(var(--accent), 0.85) ${pct}%, rgba(var(--foreground), 0.12) ${pct}%)`,
          height: "4px",
          borderRadius: "9999px",
        }}
        className="w-full mt-1 cursor-pointer"
      />
    </div>
  );
};
