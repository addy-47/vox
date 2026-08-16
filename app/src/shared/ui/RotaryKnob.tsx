import React, { useState, useRef, useCallback, useEffect, useMemo, memo } from "react";
import { cn } from "@/shared/lib/utils";
import { Minus, Plus } from "lucide-react";
import { Tooltip } from "@/shared/ui/Tooltip";

export interface RotaryKnobProps {
  label?: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  defaultValue?: number;
  formatValue?: (val: number) => string;
  formatPreset?: (val: number) => string;
  onChange: (val: number) => void;
  presetSteps?: number[];
  className?: string;
}

export const RotaryKnob = memo(({
  label = "Speed",
  value,
  min,
  max,
  step = 0.05,
  defaultValue = 1.0,
  formatValue = (v: number) => `${v.toFixed(2)}x`,
  formatPreset,
  onChange,
  presetSteps = [0.8, 1.0, 1.25, 1.5, 2.0],
  className,
}: RotaryKnobProps) => {
  const [isDragging, setIsDragging] = useState(false);
  const startYRef = useRef<number>(0);
  const startValRef = useRef<number>(value);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  // Clamp & normalize helper (prevents IEEE-754 precision drift)
  const clamp = useCallback(
    (v: number) => {
      const clamped = Math.min(max, Math.max(min, v));
      return Number(clamped.toFixed(4));
    },
    [min, max]
  );

  // Memoize SVG arc geometry
  const radius = 32;
  const strokeWidth = 4;
  const center = 40;
  const circumference = 2 * Math.PI * radius;
  const arcLength = (270 / 360) * circumference;

  const strokeDashoffset = useMemo(() => {
    const pct = Math.max(0, Math.min(1, (value - min) / (max - min)));
    return arcLength * (1 - pct);
  }, [value, min, max, arcLength]);

  // Stable event listeners using refs to prevent memory/listener leaks
  const handleMouseMove = useCallback(
    (e: MouseEvent) => {
      const deltaY = startYRef.current - e.clientY;
      const range = max - min;
      const deltaVal = (deltaY / 100) * range;
      const rawVal = startValRef.current + deltaVal;
      const newVal = Math.round(rawVal / step) * step;
      const clampedVal = clamp(newVal);
      onChangeRef.current(clampedVal);
    },
    [min, max, step, clamp]
  );

  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
    window.removeEventListener("mousemove", handleMouseMove);
    window.removeEventListener("mouseup", handleMouseUp);
  }, [handleMouseMove]);

  // Cleanup on unmount if unmounted while dragging
  useEffect(() => {
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [handleMouseMove, handleMouseUp]);

  const handleMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    setIsDragging(true);
    startYRef.current = e.clientY;
    startValRef.current = value;
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  };

  // Wheel interaction handler
  const handleWheel = (e: React.WheelEvent) => {
    e.preventDefault();
    const dir = e.deltaY < 0 ? 1 : -1;
    const newVal = Math.round((value + dir * step) / step) * step;
    onChange(clamp(newVal));
  };

  // Step adjusters
  const stepDown = () => onChange(clamp(Math.round((value - step) / step) * step));
  const stepUp = () => onChange(clamp(Math.round((value + step) / step) * step));
  const resetDefault = () => onChange(clamp(defaultValue));

  return (
    <div className={cn("flex flex-col items-center justify-center select-none will-change-transform transform-gpu", className)}>
      {label && (
        <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/75 mb-1.5 flex items-center gap-1">
          {label}
          {value !== defaultValue && (
            <Tooltip label={`Reset to ${formatValue(defaultValue)}`}>
            <button
              type="button"
              onClick={resetDefault}
              className="text-[11px] text-[rgb(var(--accent))] hover:underline cursor-pointer"
            >
              (reset)
            </button>
          </Tooltip>
          )}
        </span>
      )}

      {/* Main Knob Control Row with Quick Micro Steppers */}
      <div className="flex items-center gap-3">
        {/* Step Down (-) Micro Button */}
        <Tooltip label="Decrease Value">
        <button
          type="button"
          onClick={stepDown}
          disabled={value <= min}
          className="w-7 h-7 rounded-lg bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)] hover:border-[rgb(var(--accent))] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] disabled:opacity-30 disabled:pointer-events-none flex items-center justify-center transition-all cursor-pointer"
        >
          <Minus size={13} />
        </button>
        </Tooltip>

        {/* Interactive Rotary Dial */}
        <Tooltip label="Drag up/down, scroll mouse wheel, or double-click to reset">
        <div
          onMouseDown={handleMouseDown}
          onWheel={handleWheel}
          onDoubleClick={resetDefault}
          className={cn(
            "relative w-20 h-20 rounded-full flex items-center justify-center transition-transform duration-150 ease-out group cursor-grab active:cursor-grabbing transform-gpu",
            isDragging
              ? "scale-105 shadow-[0_0_16px_rgba(var(--accent),0.35)]"
              : "hover:scale-[1.03] hover:shadow-[0_0_12px_rgba(var(--accent),0.15)]"
          )}
        >
          {/* SVG Progress Arc Ring */}
          <svg className="w-full h-full -rotate-[135deg] pointer-events-none" viewBox="0 0 80 80">
            {/* Background Arc Track */}
            <circle
              cx={center}
              cy={center}
              r={radius}
              fill="none"
              stroke="rgba(var(--foreground), 0.08)"
              strokeWidth={strokeWidth}
              strokeDasharray={`${arcLength} ${circumference}`}
              strokeLinecap="round"
            />
            {/* Active Accent Arc with fluid smooth transition */}
            <circle
              cx={center}
              cy={center}
              r={radius}
              fill="none"
              stroke="rgb(var(--accent))"
              strokeWidth={strokeWidth}
              strokeDasharray={`${arcLength} ${circumference}`}
              strokeDashoffset={strokeDashoffset}
              strokeLinecap="round"
              className={cn(
                "transition-[stroke-dashoffset] ease-out",
                isDragging ? "duration-75" : "duration-300"
              )}
            />
          </svg>

          {/* Center Knob Hub with Value Display */}
          <div className="absolute inset-2.5 rounded-full bg-[rgba(var(--surface-bg),0.9)] border border-[rgba(var(--accent),0.25)] group-hover:border-[rgb(var(--accent))] flex items-center justify-center shadow-inner transition-colors">
            <span className="text-[14px] font-mono font-black text-[rgb(var(--foreground))]">
              {formatValue(value)}
            </span>
          </div>
        </div>
        </Tooltip>

        {/* Step Up (+) Micro Button */}
        <Tooltip label="Increase Value">
        <button
          type="button"
          onClick={stepUp}
          disabled={value >= max}
          className="w-7 h-7 rounded-lg bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)] hover:border-[rgb(var(--accent))] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] disabled:opacity-30 disabled:pointer-events-none flex items-center justify-center transition-all cursor-pointer"
        >
          <Plus size={13} />
        </button>
        </Tooltip>
      </div>

      {/* Preset Step Buttons Bar */}
      {presetSteps.length > 0 && (
        <div className="flex items-center justify-center gap-1.5 mt-2">
          {presetSteps.map((preset) => (
            <button
              key={preset}
              type="button"
              onClick={() => onChange(clamp(preset))}
              className={cn(
                "px-2 py-0.5 rounded-md text-[11px] font-mono font-bold transition-all duration-150 cursor-pointer",
                Math.abs(value - preset) < 0.02
                  ? "bg-[rgb(var(--accent))]/20 border border-[rgb(var(--accent))] text-[rgb(var(--accent))] font-black shadow-[0_0_8px_rgba(var(--accent),0.35)]"
                  : "bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.08)] text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)]"
              )}
            >
              {formatPreset ? formatPreset(preset) : preset}
            </button>
          ))}
        </div>
      )}
    </div>
  );
});

RotaryKnob.displayName = "RotaryKnob";
