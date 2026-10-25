import { memo, useId } from "react";
import { cn } from "@/shared/lib/utils";
import { SPEECH_CARD_COPY } from "@/data/settingsCopy";

/* Geometry (SVG user units). The lane keyframes in index.css are sampled from
   these exact curves — change one, regenerate the other. */
const W = 960;
const H = 128;
const MID = H / 2;
const CX = 400;
const HALF = 66;
const INLET = CX - HALF; // 334
const MAX_WORDS = 6;
const LANE_COUNT = 3;
const LANE_SPREAD = 30;

const LANE_PATHS = [-LANE_SPREAD, 0, LANE_SPREAD].map((dy) => {
  const y = MID + dy;
  return `M 0 ${y} C ${INLET * 0.45} ${y}, ${INLET * 0.7} ${MID}, ${INLET} ${MID}`;
});

/* Static engine mark: a bare microphone drawn in line only — no frame, no
   background, no glow. The capsule, grille, cradle and stand carry the detail. */
const MIC_HALF_W = 11;
const MIC_TOP = -42;
const MIC_BOTTOM = 0;
const MIC_GRILLE = [-35, -30, -25, -20, -15];
const MIC_GRILLE_HALF = [6, 7, 7, 7, 6];
const MIC_CRADLE = "M -19 -8 L -19 4 A 19 19 0 0 0 19 4 L 19 -8";
const MIC_STAND = "M 0 23 L 0 36";
const MIC_BASE = "M -12 36 L 12 36";

/* Output waveform: starts at the core almost invisible, grows into solid bars
   over the first few steps, then tapers toward the right edge. Every bar
   animates; the delay ramps across the tail so the crest travels left to right. */
const BAR_GAP = 12;
const BAR_START = CX + 46;
const BAR_END = W - 10;
const BAR_COUNT = Math.floor((BAR_END - BAR_START) / BAR_GAP) + 1;
const BAR_FADE_STEPS = 5;
const BAR_PHASE_STEP = 0.04; // seconds; full sweep stays under one 2.2s cycle

/** Heights and opacity ramp up after the core, then taper; computed once. */
const OUT_BARS = Array.from({ length: BAR_COUNT }, (_, i) => {
  const solidify = Math.min(1, i / BAR_FADE_STEPS);
  const env = 0.45 + 0.55 * Math.exp(-i / (BAR_COUNT * 0.9));
  const jitter = 0.55 + 0.45 * Math.abs(Math.sin(i * 1.7) * Math.cos(i * 0.6));
  return {
    x: BAR_START + i * BAR_GAP,
    h: Math.round(Math.max(4, 56 * env * jitter * (0.2 + 0.8 * solidify))),
    op: Math.round((0.08 + 0.92 * solidify) * (0.3 + 0.7 * env) * 100) / 100,
    // Negative delay so the leftmost bar is furthest ahead: the crest moves right.
    delay: -((BAR_COUNT - 1 - i) * BAR_PHASE_STEP),
  };
});

export interface EngineFlowBannerProps {
  /** Engine display name, shown beside the drawing. */
  engineName: string;
  words?: readonly string[];
  className?: string;
}

/**
 * Text flows in from the left edge along curved lanes, is absorbed by the
 * bare speaker driver, and leaves as a faded waveform that solidifies and
 * tapers to the right. Decorative: the drawing is aria-hidden, the caption
 * carries the meaning. Only the six word elements animate (index.css `efb-*`);
 * the whole waveform is static, so hovering the modal causes no repaint.
 */
export const EngineFlowBanner = memo(
  ({ engineName, words = SPEECH_CARD_COPY.engineFlow.words, className }: EngineFlowBannerProps) => {
    const uid = useId().replace(/:/g, "");
    const shown = words.slice(0, MAX_WORDS);
    const accent = "rgb(var(--accent))";

    return (
      <div
        className={cn(
          "hidden md:flex shrink-0 items-center gap-6 rounded-2xl border border-[rgba(var(--foreground),0.08)]",
          "bg-[rgba(var(--card),0.75)] px-5 py-3 shadow-lg select-none overflow-hidden",
          className
        )}
      >
        <div className="min-w-0 flex-1 flex justify-center [contain:layout_paint]">
          <svg
            viewBox={`0 0 ${W} ${H}`}
            aria-hidden="true"
            focusable="false"
            className="block w-full max-w-[960px] aspect-[960/128]"
          >
            <defs>
              <linearGradient id={`l${uid}`} gradientUnits="userSpaceOnUse" x1="0" x2={INLET} y1="0" y2="0">
                <stop offset="0" stopColor="currentColor" stopOpacity="0" />
                <stop offset="0.3" stopColor="currentColor" stopOpacity="0.18" />
                <stop offset="1" stopColor="currentColor" stopOpacity="0.22" />
              </linearGradient>
            </defs>

            {/* guide lanes */}
            <g fill="none" stroke={`url(#l${uid})`} strokeDasharray="2 6">
              {LANE_PATHS.map((d, i) => (
                <path key={i} d={d} />
              ))}
            </g>

            {/* words in, starting at the left edge; every third word is accented */}
            <g fontSize="14" fontWeight="500" textAnchor="start" style={{ fontFamily: "inherit" }}>
              {shown.map((word, i) => (
                <text
                  key={`${word}-${i}`}
                  x="0"
                  y="0"
                  className={`efb-a efb-w efb-lane-${i % LANE_COUNT}`}
                  fill={i % 3 === 1 ? accent : "currentColor"}
                  style={{ animationDelay: `${(-i * 7) / shown.length}s` }}
                >
                  {word}
                </text>
              ))}
            </g>

            {/* engine — bare microphone, line only */}
            <g
              transform={`translate(${CX} ${MID})`}
              fill="none"
              stroke={accent}
              strokeLinecap="round"
            >
              {/* capsule */}
              <rect
                x={-MIC_HALF_W}
                y={MIC_TOP}
                width={MIC_HALF_W * 2}
                height={MIC_BOTTOM - MIC_TOP}
                rx={MIC_HALF_W}
                strokeWidth="1.5"
              />
              {/* grille bars */}
              <g strokeWidth="1" strokeOpacity="0.55">
                {MIC_GRILLE.map((y, k) => (
                  <line key={y} x1={-MIC_GRILLE_HALF[k]} y1={y} x2={MIC_GRILLE_HALF[k]} y2={y} />
                ))}
              </g>
              {/* capsule band */}
              <line x1={-MIC_HALF_W} y1="-8" x2={MIC_HALF_W} y2="-8" strokeWidth="1" strokeOpacity="0.5" />
              {/* cradle and stand */}
              <path d={MIC_CRADLE} strokeWidth="1.5" />
              <path d={MIC_STAND} strokeWidth="1.5" />
              <path d={MIC_BASE} strokeWidth="1.5" />
              {/* capsule indicator */}
              <circle cy="-4" r="1.6" fill={accent} stroke="none" />
            </g>

            {/* Static tapered output waveform — matches JSDoc contract, zero per-bar SVG repaint */}
            <g fill={accent}>
              {OUT_BARS.map((b, i) => (
                <rect
                  key={i}
                  x={b.x}
                  y={MID - b.h / 2}
                  width="3.5"
                  height={b.h}
                  rx="1.75"
                  opacity={b.op}
                />
              ))}
            </g>
          </svg>
        </div>

        <div className="shrink-0 max-w-[200px] text-right">
          <p className="text-[11px] font-bold uppercase tracking-[0.14em] text-[rgb(var(--accent))]">
            {SPEECH_CARD_COPY.engineFlow.title}
          </p>
          <p className="mt-1 text-[12px] leading-snug text-[rgb(var(--foreground))]">{engineName}</p>
          <p className="mt-0.5 text-[11px] leading-snug text-[rgb(var(--foreground-muted))]/70">
            {SPEECH_CARD_COPY.engineFlow.caption}
          </p>
        </div>
      </div>
    );
  }
);

EngineFlowBanner.displayName = "EngineFlowBanner";
