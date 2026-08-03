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

  return (
    <div className={cn("space-y-1.5 w-full", className)}>
      <div className="flex justify-between items-center">
        <div className="flex flex-col">
          <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70 flex items-center gap-1.5">
            {Icon && <Icon size={12} className="text-[rgb(var(--accent))]" />}
            {label}
          </span>
          {sublabel && (
            <span className="text-[10px] text-[rgb(var(--foreground-muted))]/55">
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
        className="w-full mt-1 cursor-pointer accent-[rgb(var(--accent))]"
      />
    </div>
  );
};
