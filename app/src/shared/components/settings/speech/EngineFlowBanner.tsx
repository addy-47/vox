import { memo, useId } from "react";
import { cn } from "@/shared/lib/utils";
import { SPEECH_CARD_COPY } from "@/data/settingsCopy";

const H = 112;
const MID = H / 2;
const CX = 270;
const INLET = CX - 66; // 204

const OUTLET_BARS = [
  { x: 348, maxH: 42, op: 0.9, delay: 0 },
  { x: 358, maxH: 48, op: 0.85, delay: 0.1 },
  { x: 368, maxH: 54, op: 0.8, delay: 0.2 },
  { x: 378, maxH: 46, op: 0.75, delay: 0.3 },
  { x: 388, maxH: 38, op: 0.7, delay: 0.4 },
  { x: 398, maxH: 44, op: 0.65, delay: 0.5 },
  { x: 408, maxH: 34, op: 0.6, delay: 0.6 },
  { x: 418, maxH: 28, op: 0.55, delay: 0.7 },
  { x: 428, maxH: 22, op: 0.5, delay: 0.8 },
  { x: 438, maxH: 18, op: 0.45, delay: 0.9 },
  { x: 448, maxH: 14, op: 0.4, delay: 1.0 },
  { x: 458, maxH: 10, op: 0.35, delay: 1.1 },
];

const INLET_LANES = [
  { d: `M 10 24 C 95 24, 145 ${MID}, ${INLET} ${MID}`, y: 24 },
  { d: `M 10 ${MID} L ${INLET} ${MID}`, y: MID },
  { d: `M 10 88 C 95 88, 145 ${MID}, ${INLET} ${MID}`, y: 88 },
];

export interface EngineFlowBannerProps {
  /** Engine display name. Shown in the banner shell, not in the drawing. */
  engineName: string;
  words?: readonly string[];
  className?: string;
}

/**
 * Text flowing through an engine and coming out as audio. A decorative
 * diagram: the shell carries the words, the drawing is `aria-hidden`.
 *
 * Uses GPU-accelerated CSS keyframe animations and standard CSS fade overlays,
 * avoiding expensive SVG SMIL timers and SVG masks that trigger CPU repaints
 * or UI jitter during hover and modal dismiss transitions.
 */
export const EngineFlowBanner = memo(({
  engineName,
  words = SPEECH_CARD_COPY.engineFlow.words,
  className,
}: EngineFlowBannerProps) => {
  const uid = useId().replace(/:/g, "");
  const accent = "rgb(var(--accent))";
  const displayWords = words.slice(0, 3);

  return (
    <div
      className={cn(
        "hidden md:flex shrink-0 items-center gap-5 rounded-2xl border border-[rgba(var(--foreground),0.08)]",
        "bg-[rgba(var(--card),0.75)] px-5 py-3 shadow-lg select-none",
        className
      )}
    >
      <style>{`
        @keyframes efb-pulse-${uid} {
          0%, 100% { transform: scale(0.85); opacity: 0.7; }
          50% { transform: scale(1.15); opacity: 1; }
        }
        @keyframes efb-bar-bounce-${uid} {
          0%, 100% { transform: scaleY(0.35); }
          50% { transform: scaleY(1); }
        }
        @keyframes efb-word-flow-${uid} {
          0% { transform: translateX(-30px); opacity: 0; }
          25% { opacity: 0.75; }
          75% { opacity: 0.75; }
          100% { transform: translateX(24px); opacity: 0; }
        }
        [data-viewport-resize] .efb-anim-${uid} {
          animation-play-state: paused !important;
        }
        @media (prefers-reduced-motion: reduce) {
          .efb-anim-${uid} {
            animation: none !important;
          }
        }
      `}</style>

      <div className="relative min-w-0 flex-1 overflow-hidden" style={{ height: H }}>
        <svg
          viewBox={`0 0 540 ${H}`}
          preserveAspectRatio="xMidYMid meet"
          aria-hidden="true"
          focusable="false"
          className="block w-full h-full"
        >
          <defs>
            <radialGradient id={`efb-halo-${uid}`} cx="50%" cy="50%" r="50%">
              <stop offset="0%" stopColor={accent} stopOpacity="0.22" />
              <stop offset="100%" stopColor={accent} stopOpacity="0" />
            </radialGradient>
          </defs>

          {/* Guide lanes: the path text takes into the engine */}
          <g fill="none" stroke="currentColor" strokeOpacity="0.12" strokeDasharray="2 5">
            {INLET_LANES.map((lane, i) => (
              <path key={i} d={lane.d} />
            ))}
          </g>

          {/* Words flowing into the inlet */}
          <g fill="currentColor" fontSize="13" fontWeight="500" style={{ fontFamily: "inherit" }}>
            {displayWords.map((word, i) => {
              const lane = INLET_LANES[i % INLET_LANES.length];
              const xPos = 85 + i * 36;
              const delay = i * 0.9;
              return (
                <text
                  key={`${word}-${i}`}
                  x={xPos}
                  y={lane.y + 4}
                  textAnchor="middle"
                  className={`efb-anim-${uid}`}
                  style={{
                    animation: `efb-word-flow-${uid} 3.6s ease-in-out infinite`,
                    animationDelay: `${delay}s`,
                    transformOrigin: `${xPos}px ${lane.y}px`,
                  }}
                >
                  {word}
                </text>
              );
            })}
          </g>

          {/* The engine core: halo, sleek minimal capsule, harmonic soundbars */}
          <circle cx={CX} cy={MID} r="64" fill={`url(#efb-halo-${uid})`} />

          <g transform={`translate(${CX} ${MID})`}>
            {/* Pill capsule boundary */}
            <rect
              x="-52"
              y="-26"
              width="104"
              height="52"
              rx="26"
              fill="color-mix(in srgb, rgb(var(--accent)) 8%, transparent)"
              stroke={accent}
              strokeWidth="1.5"
            />

            {/* Subtle central activity indicator */}
            <circle
              cx="-24"
              cy="0"
              r="3.5"
              fill={accent}
              className={`efb-anim-${uid}`}
              style={{
                animation: `efb-pulse-${uid} 2s ease-in-out infinite`,
                transformOrigin: "-24px 0px",
              }}
            />

            {/* Clean minimal equalizer bars */}
            <g fill={accent}>
              {[
                { x: -8, h: 14, delay: 0 },
                { x: 3, h: 26, delay: 0.18 },
                { x: 14, h: 32, delay: 0.36 },
                { x: 25, h: 22, delay: 0.54 },
                { x: 34, h: 13, delay: 0.72 },
              ].map((bar, k) => (
                <rect
                  key={k}
                  x={bar.x}
                  y={-bar.h / 2}
                  width="3.5"
                  height={bar.h}
                  rx="1.75"
                  className={`efb-anim-${uid}`}
                  style={{
                    transformOrigin: `${bar.x + 1.75}px 0px`,
                    animation: `efb-bar-bounce-${uid} 1.2s ease-in-out infinite`,
                    animationDelay: `-${bar.delay}s`,
                  }}
                />
              ))}
            </g>
          </g>

          {/* Audio bars leaving the engine */}
          <g fill={accent}>
            {OUTLET_BARS.map((bar, i) => (
              <rect
                key={i}
                x={bar.x}
                y={MID - bar.maxH / 2}
                width="3.5"
                height={bar.maxH}
                rx="1.75"
                opacity={bar.op}
                className={`efb-anim-${uid}`}
                style={{
                  transformOrigin: `${bar.x + 1.75}px ${MID}px`,
                  animation: `efb-bar-bounce-${uid} 1.3s ease-in-out infinite`,
                  animationDelay: `${bar.delay}s`,
                }}
              />
            ))}
          </g>
        </svg>

        {/* Lightweight hardware-accelerated CSS gradient overlays replacing heavy SVG masks */}
        <div className="pointer-events-none absolute inset-y-0 left-0 w-16 bg-gradient-to-r from-[rgba(var(--card),0.9)] to-transparent" />
        <div className="pointer-events-none absolute inset-y-0 right-0 w-24 bg-gradient-to-l from-[rgba(var(--card),0.9)] to-transparent" />
      </div>

      <div className="shrink-0 max-w-[220px] text-right">
        <p className="text-[11px] font-bold uppercase tracking-[0.14em] text-[rgb(var(--accent))]">
          {SPEECH_CARD_COPY.engineFlow.title}
        </p>
        <p className="mt-1 text-[12px] leading-snug text-[rgb(var(--foreground-muted))]">
          {engineName}
        </p>
        <p className="mt-0.5 text-[11px] leading-snug text-[rgb(var(--foreground-muted))]/70">
          {SPEECH_CARD_COPY.engineFlow.caption}
        </p>
      </div>
    </div>
  );
});

EngineFlowBanner.displayName = "EngineFlowBanner";
