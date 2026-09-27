import { memo, useCallback, useId, type ReactNode } from "react";
import { cn } from "@/shared/lib/utils";

export interface TriangularLoopOption<T extends string = string> {
  id: T;
  label: string;
  sublabel?: string;
  description?: string;
  customIcon?: (color: string) => ReactNode;
}

export interface TriangularLoopSelectorProps<T extends string = string> {
  options: [TriangularLoopOption<T>, TriangularLoopOption<T>, TriangularLoopOption<T>];
  value: T;
  onChange: (value: T) => void;
  className?: string;
  showActiveLabel?: boolean;
  disabled?: boolean;
  "aria-label"?: string;
}

/**
 * Built-in crisp vector glyphs for standard delivery/output destinations.
 */
function renderDefaultGlyph(id: string, color: string, cx: number, cy: number) {
  if (id === "paste") {
    // Send / Keypress paper airplane glyph
    return (
      <g stroke={color} strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round" fill="none">
        <path d={`M ${cx - 4.5} ${cy + 3.5} L ${cx + 5.5} ${cy} L ${cx - 4.5} ${cy - 3.5} L ${cx - 2.5} ${cy} Z`} />
        <line x1={cx - 2.5} y1={cy} x2={cx + 1.5} y2={cy} />
      </g>
    );
  }
  if (id === "clipboard") {
    // Clipboard sheet with clip glyph
    return (
      <g stroke={color} strokeWidth="1.15" strokeLinecap="round" strokeLinejoin="round" fill="none">
        <rect x={cx - 4.5} y={cy - 4} width="9" height="10" rx="1.5" />
        <path d={`M ${cx - 2} ${cy - 4} V ${cy - 5.5} H ${cx + 2} V ${cy - 4}`} />
        <line x1={cx - 2} y1={cy - 0.8} x2={cx + 2} y2={cy - 0.8} strokeWidth="0.9" />
        <line x1={cx - 2} y1={cy + 2} x2={cx + 1} y2={cy + 2} strokeWidth="0.9" />
      </g>
    );
  }
  if (id === "tray") {
    // HUD / Stacked layers glyph
    return (
      <g stroke={color} strokeWidth="1.15" strokeLinecap="round" strokeLinejoin="round" fill="none">
        <path d={`M ${cx - 5} ${cy - 2.5} L ${cx} ${cy} L ${cx + 5} ${cy - 2.5} L ${cx} ${cy - 4.5} Z`} />
        <path d={`M ${cx - 5} ${cy + 1} L ${cx} ${cy + 3.5} L ${cx + 5} ${cy + 1}`} />
        <path d={`M ${cx - 5} ${cy + 4.5} L ${cx} ${cy + 7} L ${cx + 5} ${cy + 4.5}`} />
      </g>
    );
  }
  // Generic circular dot fallback
  return <circle cx={cx} cy={cy} r="3" fill={color} />;
}

export const TriangularLoopSelector = memo(
  <T extends string = string>({
    options,
    value,
    onChange,
    className,
    showActiveLabel = false,
    disabled = false,
    "aria-label": ariaLabel = "Triangular cyclic mode selector",
  }: TriangularLoopSelectorProps<T>) => {
    const filterId = useId();
    const activeIndex = options.findIndex((o) => o.id === value);
    const resolvedIndex = activeIndex >= 0 ? activeIndex : 0;
    const activeOption = options[resolvedIndex];

    const handleAdvanceLoop = useCallback(() => {
      if (disabled) return;
      const nextIndex = (resolvedIndex + 1) % options.length;
      onChange(options[nextIndex].id);
    }, [disabled, resolvedIndex, options, onChange]);

    const handleSelectIndex = useCallback(
      (idx: number, e?: React.MouseEvent) => {
        if (e) e.stopPropagation();
        if (disabled) return;
        if (idx === resolvedIndex) {
          handleAdvanceLoop();
        } else {
          onChange(options[idx].id);
        }
      },
      [disabled, resolvedIndex, handleAdvanceLoop, onChange, options]
    );

    // Node layout positions in tightened viewBox "0 0 184 92" (~15% less wide, ~7% taller)
    // Node 0: Top-Left (Paste)
    // Node 1: Top-Right (Clipboard)
    // Node 2: Bottom-Center (Tray)
    // labelY spaced with generous 14px padding from node center
    const nodes = [
      { cx: 34, cy: 17, labelY: 46 },
      { cx: 150, cy: 17, labelY: 46 },
      { cx: 92, cy: 56, labelY: 84 },
    ];

    // Connecting curved orbital loop paths:
    // Path 0 -> 1 (Top-Left to Top-Right)
    // Path 1 -> 2 (Top-Right to Bottom-Center)
    // Path 2 -> 0 (Bottom-Center to Top-Left)
    const arcs = [
      {
        d: "M 48 14 Q 92 5 136 14",
        arrowPoints: "138,14 130,10.5 132,14.5 130,18",
        sourceIndex: 0,
        targetIndex: 1,
      },
      {
        d: "M 141 30 Q 138 54 108 53",
        arrowPoints: "106,53 114,48 112,52.5 116,56",
        sourceIndex: 1,
        targetIndex: 2,
      },
      {
        d: "M 76 53 Q 46 54 43 30",
        arrowPoints: "41,28 43,36.5 46,32 51,34",
        sourceIndex: 2,
        targetIndex: 0,
      },
    ];

    return (
      <div
        className={cn(
          "flex flex-col items-center justify-center select-none",
          disabled && "opacity-50 pointer-events-none",
          className
        )}
        role="radiogroup"
        aria-label={ariaLabel}
      >
        <svg
          viewBox="0 0 184 92"
          className="w-[184px] h-[92px] overflow-visible cursor-pointer"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
          onClick={handleAdvanceLoop}
          role="button"
          tabIndex={0}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              handleAdvanceLoop();
            }
          }}
          aria-label={`Cycle to next mode (currently ${activeOption.label})`}
        >
          <defs>
            {/* Ambient accent glow filter */}
            <filter id={`${filterId}-glow`} x="-40%" y="-40%" width="180%" height="180%">
              <feGaussianBlur in="SourceGraphic" stdDeviation="2.5" result="blur" />
              <feMerge>
                <feMergeNode in="blur" />
                <feMergeNode in="SourceGraphic" />
              </feMerge>
            </filter>
          </defs>

          {/* Background orbital guide paths (subtle wireframe) */}
          {arcs.map((arc, i) => {
            const isArcActive = resolvedIndex === arc.sourceIndex;
            return (
              <g key={`arc-${i}`} className="transition-all duration-300">
                <path
                  d={arc.d}
                  stroke={isArcActive ? "rgb(var(--accent))" : "rgba(var(--foreground), 0.12)"}
                  strokeWidth={isArcActive ? "1.6" : "1"}
                  strokeDasharray={isArcActive ? undefined : "3 3"}
                  strokeLinecap="round"
                  className={cn("transition-all duration-300", isArcActive && "opacity-90")}
                  filter={isArcActive ? `url(#${filterId}-glow)` : undefined}
                />
                <polygon
                  points={arc.arrowPoints}
                  fill={isArcActive ? "rgb(var(--accent))" : "rgba(var(--foreground), 0.22)"}
                  className="transition-all duration-300"
                />
              </g>
            );
          })}

          {/* Interactive Nodes */}
          {nodes.map((pos, idx) => {
            const opt = options[idx];
            if (!opt) return null;
            const isSelected = resolvedIndex === idx;

            const nodeStroke = isSelected ? "rgb(var(--accent))" : "rgba(var(--foreground), 0.18)";
            const nodeFill = isSelected ? "rgba(var(--accent), 0.18)" : "rgba(var(--foreground), 0.03)";
            const iconColor = isSelected ? "rgb(var(--accent))" : "rgba(var(--foreground-muted), 0.65)";
            const textColor = isSelected ? "rgb(var(--accent))" : "rgba(var(--foreground-muted), 0.55)";

            return (
              <g
                key={opt.id}
                role="radio"
                aria-checked={isSelected}
                tabIndex={-1}
                onClick={(e) => handleSelectIndex(idx, e)}
                className="group/node cursor-pointer transition-all duration-300"
              >
                {/* Outer Glow Halo for Active Node */}
                {isSelected && (
                  <circle
                    cx={pos.cx}
                    cy={pos.cy}
                    r="15"
                    fill="none"
                    stroke="rgb(var(--accent))"
                    strokeWidth="1"
                    strokeOpacity="0.3"
                    className="animate-pulse"
                  />
                )}

                {/* Node Capsule / Circle (r=12) */}
                <circle
                  cx={pos.cx}
                  cy={pos.cy}
                  r="12"
                  fill={nodeFill}
                  stroke={nodeStroke}
                  strokeWidth={isSelected ? "1.5" : "1"}
                  className="transition-all duration-300 group-hover/node:stroke-[rgb(var(--accent))]/60 group-hover/node:fill-[rgba(var(--accent),0.08)]"
                  filter={isSelected ? `url(#${filterId}-glow)` : undefined}
                />

                {/* Node Icon Glyph */}
                <g className="transition-transform duration-200 group-hover/node:scale-110" style={{ transformOrigin: `${pos.cx}px ${pos.cy}px` }}>
                  {opt.customIcon
                    ? opt.customIcon(iconColor)
                    : renderDefaultGlyph(opt.id, iconColor, pos.cx, pos.cy)}
                </g>

                {/* Node Text Label with dedicated vertical padding below icon */}
                <text
                  x={pos.cx}
                  y={pos.labelY}
                  textAnchor="middle"
                  fill={textColor}
                  className={cn(
                    "font-mono font-bold text-[8px] uppercase tracking-wider select-none transition-colors duration-300 group-hover/node:fill-[rgb(var(--foreground))]",
                    isSelected && "font-black"
                  )}
                >
                  {opt.label}
                </text>
              </g>
            );
          })}
        </svg>

        {/* Dynamic Mode Subtitle underneath loop (if requested) */}
        {showActiveLabel && (
          <div className="flex items-center gap-1.5 -mt-1 text-[10px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--accent))] transition-all duration-300">
            <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.6)]" />
            <span>{activeOption.label} Active</span>
          </div>
        )}
      </div>
    );
  }
);

TriangularLoopSelector.displayName = "TriangularLoopSelector";
