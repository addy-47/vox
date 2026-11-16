import { memo, useId } from "react";
import { cn } from "@/shared/lib/utils";
import { SPEECH_CARD_COPY } from "@/data/settingsCopy";

/* Geometry (SVG user units). Matches index.css lane coordinate system. */
const W = 960;
const H = 128;
const MID = H / 2;
const CX = 400;
const HALF = 66;
const INLET = CX - HALF; // 334
const OUTLET = CX + 46;
const MAX_WORDS = 6;
const LANE_COUNT = 3;
const LANE_SPREAD = 30;

const LANE_PATHS = [-LANE_SPREAD, 0, LANE_SPREAD].map((dy) => {
  const y = MID + dy;
  return `M 0 ${y} C ${INLET * 0.45} ${y}, ${INLET * 0.7} ${MID}, ${INLET} ${MID}`;
});

const OUT_LANE_PATHS = [-LANE_SPREAD, 0, LANE_SPREAD].map((dy) => {
  const y = MID + dy;
  return `M ${OUTLET} ${MID} C ${OUTLET + 100} ${MID}, ${OUTLET + 220} ${y}, ${W - 20} ${y}`;
});

/* Microphone mark geometry (TTS) */
const MIC_HALF_W = 11;
const MIC_TOP = -42;
const MIC_BOTTOM = 0;
const MIC_GRILLE = [-35, -30, -25, -20, -15];
const MIC_GRILLE_HALF = [6, 7, 7, 7, 6];
const MIC_CRADLE = "M -19 -8 L -19 4 A 19 19 0 0 0 19 4 L 19 -8";
const MIC_STAND = "M 0 23 L 0 36";
const MIC_BASE = "M -12 36 L 12 36";

/* Output waveform for TTS */
const BAR_GAP = 12;
const BAR_START = OUTLET;
const BAR_END = W - 10;
const BAR_COUNT = Math.floor((BAR_END - BAR_START) / BAR_GAP) + 1;
const BAR_FADE_STEPS = 5;
const BAR_PHASE_STEP = 0.04;

const OUT_BARS = Array.from({ length: BAR_COUNT }, (_, i) => {
  const solidify = Math.min(1, i / BAR_FADE_STEPS);
  const env = 0.45 + 0.55 * Math.exp(-i / (BAR_COUNT * 0.9));
  const jitter = 0.55 + 0.45 * Math.abs(Math.sin(i * 1.7) * Math.cos(i * 0.6));
  return {
    x: BAR_START + i * BAR_GAP,
    h: Math.round(Math.max(4, 56 * env * jitter * (0.2 + 0.8 * solidify))),
    op: Math.round((0.08 + 0.92 * solidify) * (0.3 + 0.7 * env) * 100) / 100,
    delay: -((BAR_COUNT - 1 - i) * BAR_PHASE_STEP),
  };
});

/* Input waveform for STT & VAD (audio flows into center) */
const IN_BAR_START = 16;
const IN_BAR_END = INLET - 16;
const IN_BAR_COUNT = Math.floor((IN_BAR_END - IN_BAR_START) / BAR_GAP) + 1;

const IN_BARS = Array.from({ length: IN_BAR_COUNT }, (_, i) => {
  const solidify = Math.min(1, (IN_BAR_COUNT - 1 - i) / BAR_FADE_STEPS);
  const env = 0.45 + 0.55 * Math.exp(-(IN_BAR_COUNT - 1 - i) / (IN_BAR_COUNT * 0.9));
  const jitter = 0.55 + 0.45 * Math.abs(Math.sin(i * 1.5) * Math.cos(i * 0.7));
  return {
    x: IN_BAR_START + i * BAR_GAP,
    h: Math.round(Math.max(4, 54 * env * jitter * (0.2 + 0.8 * solidify))),
    op: Math.round((0.08 + 0.92 * solidify) * (0.3 + 0.7 * env) * 100) / 100,
    delay: -(i * BAR_PHASE_STEP),
  };
});

/* VAD Speech bursts & pulse boundaries on right */
const VAD_PULSES = Array.from({ length: BAR_COUNT }, (_, i) => {
  const isSpeech = (i >= 3 && i <= 13) || (i >= 18 && i <= 30);
  const h = isSpeech ? Math.round(24 + 32 * Math.abs(Math.sin(i * 0.85))) : 4;
  const op = isSpeech ? 0.88 : 0.16;
  return {
    x: BAR_START + i * BAR_GAP,
    h,
    op,
    isSpeech,
    delay: -(i * 0.06),
  };
});

export interface SpeechFlowBannerProps {
  /** Pipeline domain: TTS (synthesis), STT (transcription), or VAD (detection). */
  kind?: "tts" | "stt" | "vad";
  /** Active engine/model display name. */
  engineName: string;
  words?: readonly string[];
  className?: string;
}

export const SpeechFlowBanner = memo(
  ({ kind = "tts", engineName, words, className }: SpeechFlowBannerProps) => {
    const uid = useId().replace(/:/g, "");
    const accent = "rgb(var(--accent))";

    // Domain-specific configuration
    const resolvedWords =
      words ??
      (kind === "stt"
        ? SPEECH_CARD_COPY.sttFlow.words
        : kind === "vad"
          ? SPEECH_CARD_COPY.vadFlow.words
          : SPEECH_CARD_COPY.engineFlow.words);

    const shown = resolvedWords.slice(0, MAX_WORDS);

    const meta =
      kind === "stt"
        ? {
            title: SPEECH_CARD_COPY.sttFlow.title,
            caption: SPEECH_CARD_COPY.sttFlow.caption,
          }
        : kind === "vad"
          ? {
              title: SPEECH_CARD_COPY.vadFlow.title,
              caption: SPEECH_CARD_COPY.vadFlow.caption,
            }
          : {
              title: SPEECH_CARD_COPY.engineFlow.title,
              caption: SPEECH_CARD_COPY.engineFlow.caption,
            };

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
              <linearGradient id={`lo${uid}`} gradientUnits="userSpaceOnUse" x1={OUTLET} x2={W} y1="0" y2="0">
                <stop offset="0" stopColor="currentColor" stopOpacity="0.25" />
                <stop offset="0.7" stopColor="currentColor" stopOpacity="0.18" />
                <stop offset="1" stopColor="currentColor" stopOpacity="0" />
              </linearGradient>
            </defs>

            {/* ── 1. TTS MODE: Words flowing in left -> Mic -> Waveform right ── */}
            {kind === "tts" && (
              <>
                {/* Guide lanes */}
                <g fill="none" stroke={`url(#l${uid})`} strokeDasharray="2 6">
                  {LANE_PATHS.map((d, i) => (
                    <path key={i} d={d} />
                  ))}
                </g>

                {/* Animated words in */}
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

                {/* Engine — bare microphone */}
                <g transform={`translate(${CX} ${MID})`} fill="none" stroke={accent} strokeLinecap="round">
                  <rect
                    x={-MIC_HALF_W}
                    y={MIC_TOP}
                    width={MIC_HALF_W * 2}
                    height={MIC_BOTTOM - MIC_TOP}
                    rx={MIC_HALF_W}
                    strokeWidth="1.5"
                  />
                  <g strokeWidth="1" strokeOpacity="0.55">
                    {MIC_GRILLE.map((y, k) => (
                      <line key={y} x1={-MIC_GRILLE_HALF[k]} y1={y} x2={MIC_GRILLE_HALF[k]} y2={y} />
                    ))}
                  </g>
                  <line x1={-MIC_HALF_W} y1="-8" x2={MIC_HALF_W} y2="-8" strokeWidth="1" strokeOpacity="0.5" />
                  <path d={MIC_CRADLE} strokeWidth="1.5" />
                  <path d={MIC_STAND} strokeWidth="1.5" />
                  <path d={MIC_BASE} strokeWidth="1.5" />
                  <circle cy="-4" r="1.6" fill={accent} stroke="none" />
                </g>

                {/* Output waveform with kinetic bar breathing */}
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
                      className="efb-a efb-bar"
                      style={{
                        transformOrigin: `${b.x + 1.75}px ${MID}px`,
                        animationDelay: `${b.delay}s`,
                      }}
                    />
                  ))}
                </g>
              </>
            )}

            {/* ── 2. STT MODE: Live audio waveform left -> Ear Decoder -> Kinetic streaming text right ── */}
            {kind === "stt" && (
              <>
                {/* Input live audio waveform on left with kinetic breathing */}
                <g fill={accent}>
                  {IN_BARS.map((b, i) => (
                    <rect
                      key={i}
                      x={b.x}
                      y={MID - b.h / 2}
                      width="3.5"
                      height={b.h}
                      rx="1.75"
                      opacity={b.op}
                      className="efb-a efb-bar"
                      style={{
                        transformOrigin: `${b.x + 1.75}px ${MID}px`,
                        animationDelay: `${b.delay}s`,
                      }}
                    />
                  ))}
                </g>

                {/* Acoustic Neural Ear Mark in center with expanding radar pulse ring */}
                <g transform={`translate(${CX} ${MID})`} fill="none" stroke={accent} strokeLinecap="round">
                  {/* Kinetic radar pulse ring */}
                  <circle cx="3" cy="-1" className="efb-a efb-pulse-ring" fill="none" stroke={accent} />
                  
                  {/* Outer ear helix */}
                  <path
                    d="M -6 -32 C 18 -32, 28 -14, 24 6 C 21 22, 10 32, -4 34 C -12 35, -16 29, -15 22 C -14 16, -9 13, -4 13 C 6 13, 11 6, 12 -4 C 13 -14, 7 -22, -6 -22"
                    strokeWidth="1.5"
                  />
                  {/* Inner antihelix trace */}
                  <path
                    d="M -2 -14 C 7 -14, 10 -6, 9 3 C 8 10, 2 16, -6 17"
                    strokeWidth="1.1"
                    strokeOpacity="0.6"
                  />
                  {/* Sound reception radar pulses */}
                  <path d="M -22 -14 A 20 20 0 0 0 -22 14" strokeWidth="1.25" strokeOpacity="0.45" />
                  <path d="M -30 -22 A 32 32 0 0 0 -30 22" strokeWidth="1" strokeOpacity="0.25" />
                  <circle cx="3" cy="-1" r="2.2" fill={accent} stroke="none" />
                </g>

                {/* Right outlet: guide lanes */}
                <g fill="none" stroke={`url(#lo${uid})`} strokeDasharray="2 6">
                  {OUT_LANE_PATHS.map((d, i) => (
                    <path key={i} d={d} />
                  ))}
                </g>

                {/* Kinetic streaming decoded words exiting ear along lanes */}
                <g fontSize="14" fontWeight="600" textAnchor="start" style={{ fontFamily: "inherit" }}>
                  {shown.map((word, i) => (
                    <text
                      key={`${word}-${i}`}
                      x="0"
                      y="0"
                      className={`efb-a efb-w efb-stt-lane-${i % LANE_COUNT}`}
                      fill={i % 3 === 1 ? accent : "currentColor"}
                      style={{ animationDelay: `${(-i * 7) / shown.length}s` }}
                    >
                      {word}
                    </text>
                  ))}
                </g>
              </>
            )}

            {/* ── 3. VAD MODE: Audio wave left -> Scanning Energy Gate center -> Kinetic Speech pulses right ── */}
            {kind === "vad" && (
              <>
                {/* Continuous noisy audio signal on left with breathing animation */}
                <g fill="currentColor" opacity="0.35">
                  {IN_BARS.map((b, i) => (
                    <rect
                      key={i}
                      x={b.x}
                      y={MID - b.h / 2}
                      width="3.5"
                      height={b.h}
                      rx="1.75"
                      className="efb-a efb-bar"
                      style={{
                        transformOrigin: `${b.x + 1.75}px ${MID}px`,
                        animationDelay: `${b.delay * 1.2}s`,
                      }}
                    />
                  ))}
                </g>

                {/* Probabilistic Gate in center (Geometric Diamond & Boundary Diaphragm) */}
                <g transform={`translate(${CX} ${MID})`} fill="none" stroke={accent} strokeLinecap="round">
                  {/* Concentric diamond boundary */}
                  <path d="M 0 -36 L 36 0 L 0 36 L -36 0 Z" strokeWidth="1.3" />
                  <path d="M 0 -22 L 22 0 L 0 22 L -22 0 Z" strokeWidth="1" strokeOpacity="0.45" />
                  {/* Active scanning horizontal threshold level */}
                  <line
                    x1="-48"
                    y1="0"
                    x2="48"
                    y2="0"
                    strokeWidth="1.5"
                    strokeDasharray="3 3"
                    className="efb-a efb-vad-scan"
                  />
                  {/* Active detection indicator */}
                  <circle cx="0" cy="0" r="3.5" fill={accent} stroke="none" />
                  {/* Energy level ticks */}
                  <line x1="-12" y1="-8" x2="12" y2="-8" strokeWidth="1" strokeOpacity="0.5" />
                  <line x1="-8" y1="-16" x2="8" y2="-16" strokeWidth="1" strokeOpacity="0.3" />
                </g>

                {/* Active speech segments on right with kinetic burst pulsation */}
                <g fill={accent}>
                  {VAD_PULSES.map((b, i) => (
                    <rect
                      key={i}
                      x={b.x}
                      y={MID - b.h / 2}
                      width="3.5"
                      height={b.h}
                      rx="1.75"
                      opacity={b.op}
                      className={b.isSpeech ? "efb-a efb-vad-burst" : undefined}
                      style={{
                        transformOrigin: `${b.x + 1.75}px ${MID}px`,
                        animationDelay: `${b.delay}s`,
                      }}
                    />
                  ))}
                </g>

                {/* Speech boundary bracket markers above active sections */}
                <g stroke={accent} strokeWidth="1.2" strokeOpacity="0.85" fill="none">
                  <path d={`M ${BAR_START + 3 * BAR_GAP - 2} ${MID - 34} L ${BAR_START + 3 * BAR_GAP - 2} ${MID - 38} L ${BAR_START + 13 * BAR_GAP + 5} ${MID - 38} L ${BAR_START + 13 * BAR_GAP + 5} ${MID - 34}`} />
                  <path d={`M ${BAR_START + 18 * BAR_GAP - 2} ${MID - 34} L ${BAR_START + 18 * BAR_GAP - 2} ${MID - 38} L ${BAR_START + 30 * BAR_GAP + 5} ${MID - 38} L ${BAR_START + 30 * BAR_GAP + 5} ${MID - 34}`} />
                </g>
              </>
            )}
          </svg>
        </div>

        {/* Text readout on right */}
        <div className="shrink-0 max-w-[210px] text-right">
          <p className="text-[11px] font-bold uppercase tracking-[0.14em] text-[rgb(var(--accent))]">
            {meta.title}
          </p>
          <p className="mt-1 text-[12.5px] font-medium leading-snug text-[rgb(var(--foreground))] truncate">
            {engineName}
          </p>
          <p className="mt-0.5 text-[11px] leading-snug text-[rgb(var(--foreground-muted))]/70">
            {meta.caption}
          </p>
        </div>
      </div>
    );
  }
);

SpeechFlowBanner.displayName = "SpeechFlowBanner";
