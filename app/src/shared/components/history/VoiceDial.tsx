import { memo } from "react";

export interface DialDot {
  key: string;
  angle: number;
  size: number;
  highlighted: boolean;
}

export interface VoiceDialProps {
  /** Dot ring radius in px (measured from the dial center). */
  radius: number;
  dots: DialDot[];
  /** Faint radial ticks grounding the dial (24 for the day view). */
  tickCount?: number;
}

const TICK_OPACITY = 0.1;
const DIM_OPACITY = 0.22;
const PAD = 7;

/**
 * Static voice-print ring: one dot per session (day) or active day (month),
 * positioned on a clock face around the central disc. Dot size encodes turn
 * count; the current window's dots are accent-lit, the rest dimmed.
 */
export const VoiceDial = memo(
  ({ radius, dots, tickCount = 0 }: VoiceDialProps) => {
    const size = radius * 2 + PAD * 2;
    const ticks = Array.from({ length: tickCount }, (_, i) => i);

    return (
      <svg
        className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 pointer-events-none"
        width={size}
        height={size}
        viewBox={`${-radius - PAD} ${-radius - PAD} ${size} ${size}`}
        aria-hidden
      >
        {/* Faint radial clock ticks grounding the dial */}
        {ticks.map((i) => (
          <line
            key={`tick-${i}`}
            x1={0}
            y1={-radius}
            x2={0}
            y2={-radius + 4}
            stroke="rgb(var(--accent))"
            strokeWidth={1}
            opacity={TICK_OPACITY}
            transform={`rotate(${(i * 360) / Math.max(tickCount, 1)})`}
          />
        ))}

        {/* Session / day dots */}
        {dots.map((dot) => (
          <circle
            key={dot.key}
            cx={0}
            cy={-radius}
            r={dot.size}
            fill={
              dot.highlighted
                ? "rgb(var(--accent))"
                : "rgb(var(--foreground-muted))"
            }
            opacity={dot.highlighted ? 0.95 : DIM_OPACITY}
            transform={`rotate(${(dot.angle * 180) / Math.PI})`}
          />
        ))}
      </svg>
    );
  }
);

VoiceDial.displayName = "VoiceDial";
